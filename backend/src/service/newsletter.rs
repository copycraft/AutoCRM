//! Newsletters, one letter per reader (#10) and scheduled sends (#11).
//!
//! The old blast (`service::email::send_newsletter`) is a single row with everyone in
//! BCC. A tracked send is one `newsletter_sends` row plus one `email_messages` row per
//! reader, each with its own unsubscribe link, open pixel and tracked links. Each
//! letter goes through the normal `send_email` job, so the send window, the
//! per-recipient cap and the suppression list all apply to it as usual.

use chrono::{DateTime, Utc};
use regex::Regex;
use serde_json::json;
use sqlx::PgConnection;

use crate::AppState;
use crate::domain::template::{self, TemplateValues};
use crate::error::{AppError, AppResult};
use crate::repo::emails::{self, NewEmail};
use crate::repo::{documents, jobs, newsletter, newsletter_tags};
use crate::service::auth::AuthUser;
use crate::service::email::{self, triggers};

/// What the office composed: stored on the send, fanned out at send time.
pub struct Compose {
    pub subject: String,
    pub body: String,
    pub body_markdown: bool,
    pub hero: Option<String>,
    pub tag_ids: Vec<i64>,
    pub attachment_document_ids: Vec<i64>,
    pub embed_document_ids: Vec<i64>,
    pub send_at: DateTime<Utc>,
}

/// Creates the send and queues its dispatch job. A future `send_at` is honoured by the
/// job's `run_at`; the dispatch itself also refuses to run early.
pub async fn schedule(
    state: &AppState,
    user: &AuthUser,
    c: &Compose,
) -> AppResult<(i64, usize)> {
    let subject = c.subject.trim();
    let body = c.body.trim();
    if subject.is_empty() {
        return Err(AppError::validation("subject is required"));
    }
    if body.is_empty() {
        return Err(AppError::validation("body is required"));
    }
    let mut tag_ids = c.tag_ids.clone();
    tag_ids.sort_unstable();
    tag_ids.dedup();
    if !tag_ids.is_empty()
        && newsletter_tags::count_live(&state.db, &tag_ids).await? != tag_ids.len() as i64
    {
        return Err(AppError::validation("a tag does not exist or is archived"));
    }
    // Bodies are literal, as with the BCC blast: there is no single recipient to
    // resolve `{{variables}}` against, so any braces refuse the send loudly.
    let empty = TemplateValues::new();
    let mut unresolved = template::render(subject, &empty).unresolved;
    unresolved.extend(template::render(body, &empty).unresolved);
    unresolved.sort();
    unresolved.dedup();
    if !unresolved.is_empty() {
        return Err(AppError::validation(format!(
            "a newsletter has no recipient to resolve {} against: remove it",
            unresolved.join(", ")
        )));
    }

    let mut tx = state.db.begin().await?;
    let id = newsletter::create_send(
        &mut *tx,
        subject,
        body,
        c.body_markdown,
        c.hero.as_deref(),
        &tag_ids,
        &c.attachment_document_ids,
        &c.embed_document_ids,
        c.send_at,
        user.user_id,
    )
    .await?;
    let audience = newsletter::audience_subscribers(&mut *tx, &tag_ids).await?;
    let recipients = audience.len() as i32;
    // The job waits until send_at; the dispatch re-checks it anyway.
    jobs::enqueue(
        &mut *tx,
        crate::jobs::kinds::NEWSLETTER_DISPATCH,
        json!({ "send_id": id }),
        Some(c.send_at),
        Some(&format!("newsletter_dispatch:{id}")),
    )
    .await?;
    tx.commit().await?;
    Ok((id, recipients as usize))
}

/// Fans a send out into one letter per reader. Idempotent: a second run writes nothing.
pub async fn dispatch(state: &AppState, send_id: i64) -> anyhow::Result<usize> {
    let Some(send) = newsletter::find_send(&state.db, send_id).await? else {
        return Ok(0);
    };
    if send.cancelled_at.is_some() {
        return Ok(0);
    }
    if send.send_at > Utc::now() {
        return Ok(0);
    }
    let already: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM email_messages WHERE newsletter_send_id = $1",
    )
    .bind(send_id)
    .fetch_one(&state.db)
    .await?;
    if already > 0 {
        return Ok(0);
    }

    let mut tx = state.db.begin().await?;
    let audience = newsletter::audience_subscribers(&mut *tx, &send.tag_ids).await?;
    if audience.is_empty() {
        newsletter::set_send_recipients(&mut *tx, send_id, 0).await?;
        tx.commit().await?;
        return Ok(0);
    }

    let empty = TemplateValues::new();
    let subject = template::single_line(&template::render(&send.subject, &empty).output);
    let rendered_body = template::render(&send.body, &empty).output;
    let rendered_html = match (send.hero.as_deref().map(str::trim), send.body_markdown) {
        (Some(hero), true) if !hero.is_empty() => {
            template::markdown_to_html_hero(hero, &rendered_body)
        }
        (Some(hero), false) if !hero.is_empty() => {
            template::email_html_hero(hero, &rendered_body)
        }
        (_, true) => template::markdown_to_html(&rendered_body),
        (_, false) => template::text_to_html(&rendered_body),
    };

    // Attachments and inline images are company documents; the endpoint is office-only,
    // so ownership is not checked — the same rule as the BCC blast.
    let mut attachments = Vec::new();
    if !send.attachment_document_ids.is_empty() {
        let ids = distinct_ids(&send.attachment_document_ids);
        let docs = documents::find_many(&mut *tx, &ids).await?;
        if docs.len() != ids.len() {
            anyhow::bail!("attachments must be existing documents");
        }
        attachments = docs
            .into_iter()
            .map(|d| email::AttachmentRef {
                document_id: d.id,
                filename: Some(d.filename),
                byte_size: Some(d.byte_size),
                mode: None,
                content_id: None,
            })
            .collect();
    }
    let base_html = rendered_html;
    let base_text = rendered_body.clone();
    let (embedded_html, embedded_text, mut embedded) = email::apply_embeds_for_newsletter(
        &mut tx,
        &send.embed_document_ids,
        base_html,
        base_text,
    )
    .await
    .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    attachments.append(&mut embedded);

    let from = email::format_from(
        &state.config.email.from_name,
        &state.config.email.from_automatic,
    )
    .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let base = state.config.public_base_url.trim_end_matches('/');

    let mut written = 0;
    for sub in &audience {
        let token: String = sqlx::query_scalar("SELECT encode(gen_random_bytes(24), 'hex')")
            .fetch_one(&mut *tx)
            .await?;
        let unsub = format!(
            "{base}/hu/newsletter/unsubscribe?token={}",
            sub.unsubscribe_token
        );
        let footer_text = format!("\n\n---\nLeiratkozás: {unsub}");
        let footer_html = format!(
            "<p style=\"margin:16px 0 0 0;font-size:11px;line-height:1.5;color:#8a847a;\">\
             <a href=\"{unsub}\" style=\"color:#8a847a;\">Leiratkozás a hírlevélről</a></p>"
        );
        let body_text = format!("{}{}", embedded_text, footer_text);
        let with_links = rewrite_links(&embedded_html, base, &token);
        let pixel = format!(
            "<img src=\"{base}/api/newsletter/track/open?token={token}\" width=\"1\" height=\"1\" alt=\"\" />"
        );
        let mut body_html = with_links.replace("</body></html>", &format!("{footer_html}{pixel}</body></html>"));
        if !body_html.contains(&pixel) {
            body_html.push_str(&format!("{footer_html}{pixel}"));
        }

        let id = emails::insert(
            &mut *tx,
            &NewEmail {
                order_id: None,
                lead_id: None,
                partner_id: None,
                blocker_id: None,
                template_key: None,
                trigger: triggers::NEWSLETTER_SEND,
                sent_by: send.created_by,
                idempotency_key: Some(&format!("newsletter_send:{send_id}:{}", sub.id)),
                to_address: &sub.email,
                cc: &[],
                bcc: &[],
                from_address: &from,
                reply_to: Some(&state.config.email.reply_to_default),
                subject: &subject,
                body_html: &body_html,
                body_text: &body_text,
                attachments: json!(attachments),
                send_after: None,
            },
        )
        .await?;
        let Some(id) = id else { continue };
        attach_tracking(&mut tx, id, send_id, &token).await?;
        jobs::enqueue(
            &mut *tx,
            "send_email",
            json!({ "email_id": id }),
            None,
            Some(&format!("send_email:{id}")),
        )
        .await?;
        written += 1;
    }
    newsletter::set_send_recipients(&mut *tx, send_id, written as i32).await?;
    tx.commit().await?;
    if written > 0 {
        tracing::info!(send_id, written, "newsletter dispatched");
    }
    Ok(written as usize)
}

async fn attach_tracking(
    tx: &mut PgConnection,
    email_id: i64,
    send_id: i64,
    token: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE email_messages SET newsletter_send_id = $2, tracking_token = $3 WHERE id = $1",
    )
    .bind(email_id)
    .bind(send_id)
    .bind(token)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

fn distinct_ids(ids: &[i64]) -> Vec<i64> {
    let mut out: Vec<i64> = Vec::with_capacity(ids.len());
    for id in ids {
        if !out.contains(id) {
            out.push(*id);
        }
    }
    out
}

/// Rewrites absolute links so a click is recorded before the reader continues.
/// The unsubscribe link and the pixel are added afterwards and stay direct.
fn rewrite_links(html: &str, base: &str, token: &str) -> String {
    let re = Regex::new(r#"href="(https?://[^"]+)""#).expect("valid link pattern");
    re.replace_all(html, |caps: &regex::Captures| {
        let url = &caps[1];
        format!(
            "href=\"{base}/api/newsletter/track/click?token={token}&url={}\"",
            percent_encode(url)
        )
    })
    .into_owned()
}

/// Percent-encodes everything outside the unreserved set, so the whole target URL
/// travels as one query value.
fn percent_encode(s: &str) -> String {
    const UNRESERVED: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_.~";
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if UNRESERVED.contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Only http(s) targets are followed; anything else is dropped to the home page.
pub fn safe_redirect(base: &str, url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("{}/", base.trim_end_matches('/'))
    }
}

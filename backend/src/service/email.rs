//! Email: composing, queueing and delivering. Manual and automatic mail share one path:
//! every message is a row in email_messages first, then a `send_email` job delivers it.

use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use lettre::Address;
use lettre::message::Mailbox;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgConnection, PgPool};

use crate::AppState;
use crate::config::{
    Config, EmailConfig, EmailTransportConfig, SmtpConfig, SmtpSecurity, domain_of,
};
use crate::domain::email::{
    AutoSendContext, AutoSendDecision, EmailStatus, SendWindow, decide_automatic, normalize_address,
};
use crate::domain::stage::{StageEntity, find as find_stage};
use crate::domain::template::{self, TemplateValues};
use crate::error::{AppError, AppResult};
use crate::integrations::email::{Mailer, OutgoingAttachment, OutgoingEmail, SendError};
use crate::media::storage::content_disposition;
use crate::repo::emails::{self, NewEmail};
use crate::repo::{
    blockers, config, contacts, documents, jobs, leads, orders, partners, stages, templates,
};
use crate::service::auth::AuthUser;

pub mod triggers {
    pub const MANUAL: &str = "manual";
    pub const NUDGE_BLOCKER: &str = "nudge_blocker";
    pub const STAGE_CHANGED: &str = "stage_changed";
    pub const STALLED_ORDER: &str = "stalled_order";
}

const MAX_ATTACHMENT_BYTES: i64 = 10 * 1024 * 1024;
const LINK_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
const MAX_SEND_ATTEMPTS: i32 = 5;

/// Where the effective email transport came from. Shown in the admin UI so
/// it is always clear whether the form or the process environment is live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TransportSource {
    Database,
    Environment,
}

/// Pure merge of stored transport fields over the environment config.
/// Returns `None` when nothing is configured here (inherit everything).
/// Unit-tested below.
// Nine scalar fields is the whole transport config; bundling them would just
// rename the struct that MergedTransport already is on the API side.
#[allow(clippy::too_many_arguments)]
pub(crate) fn transport_from_parts(
    mode: Option<&str>,
    host: Option<String>,
    port: Option<i32>,
    security: Option<&str>,
    username: Option<String>,
    password: Option<String>,
    helo_name: Option<String>,
    force_ipv4: Option<bool>,
    message_id_domain: &str,
) -> Option<EmailTransportConfig> {
    match mode {
        None => None,
        Some("dry_run") => Some(EmailTransportConfig::DryRun),
        Some("smtp") => {
            let host = host.filter(|h| !h.trim().is_empty())?;
            let security: SmtpSecurity = match security {
                None => SmtpSecurity::StartTls,
                Some(v) => v.parse().ok()?,
            };
            Some(EmailTransportConfig::Smtp(SmtpConfig {
                host,
                port: port.unwrap_or(587) as u16,
                username,
                password,
                security,
                helo_name: helo_name
                    .filter(|h| !h.trim().is_empty())
                    .unwrap_or_else(|| message_id_domain.to_string()),
                force_ipv4: force_ipv4.unwrap_or(false),
            }))
        }
        Some(other) => {
            tracing::error!(mode = %other, "stored email_mode is invalid; using environment");
            None
        }
    }
}

fn resolve_transport(
    s: &config::Settings,
    password: Option<String>,
    env: &EmailConfig,
) -> Option<EmailTransportConfig> {
    transport_from_parts(
        s.email_mode.as_deref(),
        s.smtp_host.clone(),
        s.smtp_port,
        s.smtp_security.as_deref(),
        s.smtp_username.clone(),
        password,
        s.smtp_helo_name.clone(),
        s.smtp_force_ipv4,
        &env.message_id_domain,
    )
}

/// The transport to actually send with: the admin settings row when it
/// configures one, else the process environment. Never fails — a broken row
/// or an unreadable table falls back to the environment with an error log.
pub async fn effective_email_config(
    db: &PgPool,
    env: &EmailConfig,
) -> (EmailConfig, TransportSource) {
    let row = match config::settings(db).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "email transport settings unreadable; using environment");
            return (env.clone(), TransportSource::Environment);
        }
    };
    // Same executor borrow discipline as everywhere: one query at a time.
    let password = config::email_secret(db).await.unwrap_or(None);
    match resolve_transport(&row, password, env) {
        Some(transport) => (
            EmailConfig {
                transport,
                redirect_to: row.redirect_to.clone(),
                ..env.clone()
            },
            TransportSource::Database,
        ),
        None => (env.clone(), TransportSource::Environment),
    }
}

/// What an email is about. Drives both the log's foreign keys and template variables.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct About {
    pub order_id: Option<i64>,
    pub lead_id: Option<i64>,
    pub partner_id: Option<i64>,
    pub blocker_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AttachmentRef {
    pub document_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_size: Option<i64>,
    /// "attached" or "link", filled in when the email is sent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

fn hu_date(d: NaiveDate) -> String {
    d.format("%Y.%m.%d.").to_string()
}

fn has_unresolved_markers(s: &str) -> bool {
    s.contains("{{MISSING:") || s.contains("{{UNKNOWN:")
}

pub fn format_from(display_name: &str, address: &str) -> AppResult<String> {
    let addr: Address = address
        .parse()
        .map_err(|e| AppError::internal(format!("invalid sender address {address}: {e}")))?;
    Ok(Mailbox::new(Some(display_name.to_string()), addr).to_string())
}

/// (From, Reply-To) for a human-written email. When the relay may send as the person's own
/// domain, they send as themselves and replies land straight in their Gmail. Otherwise it
/// goes out from the automatic mailbox under their name, with replies routed back to them.
pub fn manual_sender(cfg: &EmailConfig, user: &AuthUser) -> AppResult<(String, Option<String>)> {
    let own_domain = domain_of(&user.email).is_some_and(|d| cfg.sender_domains.contains(&d));
    if own_domain {
        Ok((format_from(&user.display_name, &user.email)?, None))
    } else {
        Ok((
            format_from(
                &format!("{} – {}", user.display_name, cfg.from_name),
                &cfg.from_automatic,
            )?,
            Some(user.email.clone()),
        ))
    }
}

/// Resolves every whitelisted variable reachable from `about`.
pub async fn template_values(
    conn: &mut PgConnection,
    about: &About,
    sender_name: Option<&str>,
    tz: Tz,
) -> AppResult<TemplateValues> {
    let mut v = TemplateValues::new();
    let today = crate::service::business_today(tz);
    if let Some(name) = sender_name {
        v.insert("user.name", name.to_string());
    }

    let mut order_id = about.order_id;
    if let Some(blocker_id) = about.blocker_id {
        let b = blockers::find(&mut *conn, blocker_id, today)
            .await?
            .ok_or(AppError::NotFound("blocker"))?;
        order_id = order_id.or(Some(b.order_id));
        v.insert("blocker.what", b.what);
        if let Some(due) = b.due_date {
            v.insert("blocker.due_date", hu_date(due));
            let overdue = (today - due).num_days();
            if overdue > 0 {
                v.insert("blocker.days_overdue", overdue.to_string());
            }
        }
    }

    let mut partner_id = about.partner_id;
    if let Some(oid) = order_id {
        let o = orders::find(&mut *conn, oid)
            .await?
            .ok_or(AppError::NotFound("order"))?;
        partner_id = partner_id.or(Some(o.partner_id));
        v.insert("order.number", o.number.clone());
        v.insert("order.title", o.title.clone());
        if let Some(plate) = &o.vehicle_plate {
            v.insert("order.plate", plate.clone());
        }
        let vehicle = [
            o.vehicle_make.as_deref(),
            o.vehicle_model.as_deref(),
            o.vehicle_plate.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
        if !vehicle.is_empty() {
            v.insert("order.vehicle", vehicle);
        }
        if let Some(due) = o.due_date {
            v.insert("order.due_date", hu_date(due));
        }
        if let Some(current) = stages::current_order_stage(&mut *conn, oid).await? {
            let definitions = config::stage_definitions(&mut *conn, StageEntity::Order).await?;
            let label = find_stage(&definitions, &current.stage_key)
                .map_or_else(|| current.stage_key.clone(), |d| d.label_hu.clone());
            v.insert("order.stage", label);
            v.insert(
                "order.days_in_stage",
                (Utc::now() - current.entered_at).num_days().to_string(),
            );
        }
        let open = blockers::list_for_order(&mut *conn, oid, today)
            .await?
            .iter()
            .filter(|b| b.resolved_at.is_none())
            .count();
        v.insert("order.open_blockers", open.to_string());
        if let Some(contact_id) = o.contact_id {
            if let Some(c) = contacts::find(&mut *conn, contact_id).await? {
                v.insert("contact.name", c.name);
            }
        }
    }

    if let Some(lead_id) = about.lead_id {
        let l = leads::find(&mut *conn, lead_id)
            .await?
            .ok_or(AppError::NotFound("lead"))?;
        partner_id = partner_id.or(l.partner_id);
        v.insert("lead.title", l.title);
        if let Some(name) = l.contact_name {
            v.entry("contact.name").or_insert(name);
        }
    }

    if let Some(pid) = partner_id {
        if let Some(p) = partners::find(&mut *conn, pid).await? {
            v.insert("partner.name", p.name);
        }
    }
    Ok(v)
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ComposeRequest {
    pub order_id: Option<i64>,
    pub lead_id: Option<i64>,
    pub partner_id: Option<i64>,
    pub to: String,
    #[serde(default)]
    pub cc: Vec<String>,
    pub template_key: Option<String>,
    pub subject: Option<String>,
    pub body: Option<String>,
    #[serde(default)]
    pub attachment_document_ids: Vec<i64>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct Preview {
    pub to: String,
    pub cc: Vec<String>,
    pub subject: String,
    pub body_text: String,
    pub body_html: String,
    pub unresolved: Vec<String>,
    /// Manual mail may still go to a suppressed address; the UI should warn.
    pub recipient_suppressed: bool,
    pub attachments: Vec<AttachmentRef>,
}

async fn prepare(
    conn: &mut PgConnection,
    cfg: &Config,
    user: &AuthUser,
    req: &ComposeRequest,
) -> AppResult<Preview> {
    let about = About {
        order_id: req.order_id,
        lead_id: req.lead_id,
        partner_id: req.partner_id,
        blocker_id: None,
    };
    if [about.order_id, about.lead_id, about.partner_id]
        .iter()
        .filter(|x| x.is_some())
        .count()
        > 1
    {
        return Err(AppError::validation(
            "an email is about at most one of order, lead or partner",
        ));
    }
    let to = normalize_address(&req.to)
        .ok_or_else(|| AppError::validation("recipient address is invalid"))?;
    let cc = req
        .cc
        .iter()
        .map(|a| {
            normalize_address(a)
                .ok_or_else(|| AppError::validation(format!("cc address '{a}' is invalid")))
        })
        .collect::<AppResult<Vec<_>>>()?;

    let tpl = match &req.template_key {
        Some(key) => Some(
            templates::find_by_key(&mut *conn, key)
                .await?
                .ok_or(AppError::NotFound("email template"))?,
        ),
        None => None,
    };
    let subject_src = req
        .subject
        .clone()
        .or_else(|| tpl.as_ref().map(|t| t.subject.clone()))
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::validation("subject is required"))?;
    let body_src = req
        .body
        .clone()
        .or_else(|| tpl.as_ref().map(|t| t.body.clone()))
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::validation("body is required"))?;

    let values = template_values(conn, &about, Some(&user.display_name), cfg.business_tz).await?;
    let subject = template::render(&subject_src, &values);
    let body = template::render(&body_src, &values);
    let mut unresolved = subject.unresolved.clone();
    unresolved.extend(body.unresolved.iter().cloned());
    unresolved.sort();
    unresolved.dedup();

    let mut attachments = Vec::new();
    if !req.attachment_document_ids.is_empty() {
        let order_id = about
            .order_id
            .ok_or_else(|| AppError::validation("attachments require an order"))?;
        let docs = documents::find_many(&mut *conn, &req.attachment_document_ids).await?;
        if docs.len() != req.attachment_document_ids.len()
            || docs.iter().any(|d| d.order_id != order_id)
        {
            return Err(AppError::validation(
                "attachments must be documents of this order",
            ));
        }
        attachments = docs
            .into_iter()
            .map(|d| AttachmentRef {
                document_id: d.id,
                filename: Some(d.filename),
                byte_size: Some(d.byte_size),
                mode: None,
            })
            .collect();
    }

    Ok(Preview {
        recipient_suppressed: emails::is_suppressed(&mut *conn, &to).await?,
        to,
        cc,
        subject: template::single_line(&subject.output),
        body_html: template::text_to_html(&body.output),
        body_text: body.output,
        unresolved,
        attachments,
    })
}

pub async fn preview(
    state: &AppState,
    user: &AuthUser,
    req: &ComposeRequest,
) -> AppResult<Preview> {
    let mut conn = state.db.acquire().await?;
    prepare(&mut conn, &state.config, user, req).await
}

/// Queues a human-written email. Refuses to queue anything with unresolved variables:
/// a person is right there to fix it.
pub async fn send_manual(
    state: &AppState,
    user: &AuthUser,
    req: &ComposeRequest,
) -> AppResult<i64> {
    let mut tx = state.db.begin().await?;
    let p = prepare(&mut tx, &state.config, user, req).await?;
    if !p.unresolved.is_empty() {
        return Err(AppError::validation(format!(
            "unresolved template variables: {}",
            p.unresolved.join(", ")
        )));
    }
    let (from, reply_to) = manual_sender(&state.config.email, user)?;
    let id = emails::insert(
        &mut *tx,
        &NewEmail {
            order_id: req.order_id,
            lead_id: req.lead_id,
            partner_id: req.partner_id,
            blocker_id: None,
            template_key: req.template_key.as_deref(),
            trigger: triggers::MANUAL,
            sent_by: Some(user.user_id),
            idempotency_key: None,
            to_address: &p.to,
            cc: &p.cc,
            from_address: &from,
            // Replies go to the person who wrote it, never to an unmonitored box.
            reply_to: reply_to.as_deref(),
            subject: &p.subject,
            body_html: &p.body_html,
            body_text: &p.body_text,
            attachments: json!(p.attachments),
            send_after: None,
        },
    )
    .await?
    .ok_or_else(|| AppError::internal("email insert returned no id"))?;
    jobs::enqueue(
        &mut *tx,
        "send_email",
        json!({ "email_id": id }),
        None,
        Some(&format!("send_email:{id}")),
    )
    .await?;
    tx.commit().await?;
    Ok(id)
}

pub struct AutomaticEmail<'a> {
    pub template_key: &'a str,
    pub about: About,
    pub to: &'a str,
    pub trigger: &'a str,
    /// Makes the queueing itself idempotent (e.g. "nudge:17:3").
    pub idempotency_key: String,
}

/// Renders and queues automatic mail inside the caller's transaction. Returns None if the
/// idempotency key was already used. Unresolved variables are kept visible in the row; the
/// send step then fails it instead of mailing a broken message.
pub async fn queue_automatic(
    conn: &mut PgConnection,
    cfg: &Config,
    e: AutomaticEmail<'_>,
) -> AppResult<Option<i64>> {
    let tpl = templates::find_by_key(&mut *conn, e.template_key)
        .await?
        .ok_or_else(|| {
            AppError::internal(format!("email template '{}' is missing", e.template_key))
        })?;
    let to = normalize_address(e.to)
        .ok_or_else(|| AppError::validation(format!("recipient '{}' is invalid", e.to)))?;
    let values = template_values(conn, &e.about, None, cfg.business_tz).await?;
    let subject = template::single_line(&template::render(&tpl.subject, &values).output);
    let body = template::render(&tpl.body, &values).output;
    let html = template::text_to_html(&body);
    let from = format_from(&cfg.email.from_name, &cfg.email.from_automatic)?;

    // Satisfy the log's "about at most one thing" rule: an order wins over lead/partner.
    let (lead_id, partner_id) = if e.about.order_id.is_some() {
        (None, None)
    } else {
        (e.about.lead_id, e.about.partner_id)
    };
    let id = emails::insert(
        &mut *conn,
        &NewEmail {
            order_id: e.about.order_id,
            lead_id,
            partner_id: if lead_id.is_some() { None } else { partner_id },
            blocker_id: e.about.blocker_id,
            template_key: Some(e.template_key),
            trigger: e.trigger,
            sent_by: None,
            idempotency_key: Some(&e.idempotency_key),
            to_address: &to,
            cc: &[],
            from_address: &from,
            reply_to: Some(&cfg.email.reply_to_default),
            subject: &subject,
            body_html: &html,
            body_text: &body,
            attachments: json!([]),
            send_after: None,
        },
    )
    .await?;
    if let Some(id) = id {
        jobs::enqueue(
            &mut *conn,
            "send_email",
            json!({ "email_id": id }),
            None,
            Some(&format!("send_email:{id}")),
        )
        .await?;
    }
    Ok(id)
}

pub enum Delivery {
    Done,
    RunAt(DateTime<Utc>),
}

/// Delivers one queued email. The row is committed as `sending` before the SMTP server is
/// contacted; finding a row already in `sending` means a previous attempt died mid-flight,
/// and it goes to `needs_review` rather than risking a duplicate.
pub async fn deliver(state: &AppState, mailer: &Mailer, email_id: i64) -> anyhow::Result<Delivery> {
    let now = Utc::now();
    let mut tx = state.db.begin().await?;
    let Some(email) = emails::lock(&mut *tx, email_id).await? else {
        return Ok(Delivery::Done);
    };
    match email.status {
        EmailStatus::Queued => {}
        EmailStatus::Sending => {
            emails::set_status(
                &mut *tx,
                email_id,
                EmailStatus::NeedsReview,
                Some("a previous send attempt was interrupted; it may or may not have been delivered"),
            )
            .await?;
            tx.commit().await?;
            tracing::warn!(
                email_id,
                "email left in 'sending' by an interrupted attempt; marked needs_review"
            );
            return Ok(Delivery::Done);
        }
        _ => return Ok(Delivery::Done),
    }
    if email.send_after > now {
        return Ok(Delivery::RunAt(email.send_after));
    }

    if email.is_automatic {
        let settings = config::settings(&mut *tx).await?;
        let window = SendWindow {
            start: settings.send_window_start,
            end: settings.send_window_end,
            weekdays_only: settings.send_window_weekdays_only,
            tz: state.config.business_tz,
        };
        let ctx = AutoSendContext {
            automatic_enabled: settings.automatic_email_enabled,
            recipient_suppressed: emails::is_suppressed(&mut *tx, &email.to_address).await?,
            sent_to_recipient_last_24h: emails::automatic_sent_last_24h(
                &mut *tx,
                &email.to_address,
            )
            .await?,
            max_per_recipient_day: settings.max_auto_emails_per_recipient_day,
            has_unresolved_variables: has_unresolved_markers(&email.subject)
                || has_unresolved_markers(&email.body_text),
        };
        match decide_automatic(ctx, &window, now) {
            AutoSendDecision::Send => {}
            AutoSendDecision::Defer(at) => {
                emails::requeue(&mut *tx, email_id, at, None).await?;
                tx.commit().await?;
                return Ok(Delivery::RunAt(at));
            }
            AutoSendDecision::Cancel(reason) => {
                emails::cancel(&mut *tx, email_id, reason, None).await?;
                tx.commit().await?;
                tracing::info!(email_id, reason, "automatic email cancelled");
                return Ok(Delivery::Done);
            }
            AutoSendDecision::Fail(reason) => {
                emails::set_status(&mut *tx, email_id, EmailStatus::Failed, Some(reason)).await?;
                tx.commit().await?;
                tracing::warn!(email_id, reason, "automatic email failed its safety checks");
                return Ok(Delivery::Done);
            }
        }
    }

    // Resolve attachments before committing to `sending`, so storage hiccups retry cleanly.
    let requested: Vec<AttachmentRef> =
        serde_json::from_value(email.attachments.clone()).unwrap_or_default();
    let ids: Vec<i64> = requested.iter().map(|a| a.document_id).collect();
    let docs = if ids.is_empty() {
        Vec::new()
    } else {
        documents::find_many(&mut *tx, &ids).await?
    };
    if docs.len() != ids.len() {
        emails::set_status(
            &mut *tx,
            email_id,
            EmailStatus::Failed,
            Some("an attached document was deleted before sending"),
        )
        .await?;
        tx.commit().await?;
        return Ok(Delivery::Done);
    }
    let mut body_text = email.body_text.clone();
    let mut body_html = email.body_html.clone();
    let mut outgoing = Vec::new();
    let mut sent_refs = Vec::new();
    let total: i64 = docs.iter().map(|d| d.byte_size).sum();
    if total <= MAX_ATTACHMENT_BYTES {
        for d in &docs {
            outgoing.push(OutgoingAttachment {
                filename: d.filename.clone(),
                content_type: d.content_type.clone(),
                bytes: state.storage.get_bytes(&d.storage_key).await?,
            });
            sent_refs.push(AttachmentRef {
                document_id: d.id,
                filename: Some(d.filename.clone()),
                byte_size: Some(d.byte_size),
                mode: Some("attached".into()),
            });
        }
    } else {
        let mut text = String::from("\n\nLetölthető fájlok (7 napig érvényes):\n");
        let mut html = String::from("<p>Letölthető fájlok (7 napig érvényes):<br>\n");
        for d in &docs {
            let url = state
                .storage
                .presign_get(
                    &d.storage_key,
                    LINK_TTL,
                    Some(content_disposition("attachment", &d.filename)),
                )
                .await?;
            text.push_str(&format!("- {}: {}\n", d.filename, url));
            html.push_str(&format!(
                "<a href=\"{}\">{}</a><br>\n",
                template::escape_html(&url),
                template::escape_html(&d.filename)
            ));
            sent_refs.push(AttachmentRef {
                document_id: d.id,
                filename: Some(d.filename.clone()),
                byte_size: Some(d.byte_size),
                mode: Some("link".into()),
            });
        }
        html.push_str("</p>");
        body_text.push_str(&text);
        body_html = body_html.replace("</body></html>", &format!("{html}</body></html>"));
        emails::set_bodies(&mut *tx, email_id, &body_text, &body_html).await?;
    }

    emails::mark_sending(&mut *tx, email_id).await?;
    tx.commit().await?;

    let random: [u8; 6] = rand::random();
    let message_id = format!(
        "<email-{email_id}.{}@{}>",
        hex::encode(random),
        state.config.email.message_id_domain
    );
    let result = mailer
        .send(OutgoingEmail {
            message_id: message_id.clone(),
            from: email.from_address.clone(),
            reply_to: email.reply_to.clone(),
            to: email.to_address.clone(),
            cc: email.cc.clone(),
            subject: email.subject.clone(),
            body_text,
            body_html,
            attachments: outgoing,
            automatic: email.is_automatic,
        })
        .await;

    match result {
        Ok(()) => {
            emails::mark_sent(&state.db, email_id, &message_id, json!(sent_refs)).await?;
            tracing::info!(email_id, to = %email.to_address, dry_run = mailer.is_dry_run(), "email sent");
            Ok(Delivery::Done)
        }
        Err(SendError::Permanent(e)) => {
            emails::set_status(&state.db, email_id, EmailStatus::Failed, Some(&e)).await?;
            tracing::warn!(email_id, error = %e, "email permanently rejected");
            Ok(Delivery::Done)
        }
        Err(SendError::Transient(e)) => {
            let attempts = email.attempts + 1;
            if attempts >= MAX_SEND_ATTEMPTS {
                emails::set_status(
                    &state.db,
                    email_id,
                    EmailStatus::Failed,
                    Some(&format!("gave up after {attempts} attempts: {e}")),
                )
                .await?;
                return Ok(Delivery::Done);
            }
            let at = Utc::now() + jobs::backoff(attempts);
            emails::requeue(&state.db, email_id, at, Some(&e)).await?;
            tracing::warn!(email_id, error = %e, retry_at = %at, "transient send failure; will retry");
            Ok(Delivery::RunAt(at))
        }
        Err(SendError::Unknown(e)) => {
            emails::set_status(&state.db, email_id, EmailStatus::NeedsReview, Some(&e)).await?;
            tracing::warn!(email_id, error = %e, "email delivery outcome unknown; needs review");
            Ok(Delivery::Done)
        }
    }
}

/// Admin action: put a failed or needs-review email back in the queue.
pub async fn retry(state: &AppState, email_id: i64) -> AppResult<()> {
    let mut tx = state.db.begin().await?;
    let email = emails::lock(&mut *tx, email_id)
        .await?
        .ok_or(AppError::NotFound("email"))?;
    if !matches!(email.status, EmailStatus::Failed | EmailStatus::NeedsReview) {
        return Err(AppError::conflict(
            "not_retryable",
            "only failed or needs-review emails can be retried",
        ));
    }
    emails::requeue(&mut *tx, email_id, Utc::now(), None).await?;
    jobs::enqueue(
        &mut *tx,
        "send_email",
        json!({ "email_id": email_id }),
        None,
        Some(&format!("send_email:{email_id}")),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_are_detected() {
        assert!(has_unresolved_markers(
            "waiting for {{MISSING:blocker.what}}"
        ));
        assert!(has_unresolved_markers("{{UNKNOWN:x}}"));
        assert!(!has_unresolved_markers("all good {{ braces }}"));
    }

    #[test]
    fn from_header_quotes_display_names() {
        let from = format_from("Kovács János – Autotherm", "noreply@autotherm.hu").unwrap();
        assert!(from.ends_with("<noreply@autotherm.hu>"), "{from}");
    }

    fn email_config(sender_domains: &[&str]) -> EmailConfig {
        EmailConfig {
            transport: crate::config::EmailTransportConfig::DryRun,
            from_automatic: "beszerzes@autotherm.hu".into(),
            from_name: "Autotherm".into(),
            reply_to_default: "iroda@autotherm.hu".into(),
            message_id_domain: "autotherm.hu".into(),
            sender_domains: sender_domains.iter().map(|d| d.to_string()).collect(),
            redirect_to: None,
        }
    }

    fn staff(email: &str) -> AuthUser {
        AuthUser {
            user_id: 1,
            session_id: 1,
            session_kind: crate::repo::sessions::SessionKind::Web,
            email: email.into(),
            display_name: "Kovács János".into(),
            role: crate::domain::role::Role::Office,
            must_change_password: false,
        }
    }

    #[test]
    fn staff_in_a_relay_domain_send_as_themselves() {
        let (from, reply_to) = manual_sender(
            &email_config(&["autotherm.hu"]),
            &staff("kovacs@autotherm.hu"),
        )
        .unwrap();
        assert!(from.ends_with("<kovacs@autotherm.hu>"), "{from}");
        assert_eq!(reply_to, None);
    }

    #[test]
    fn other_staff_send_via_the_shared_mailbox_with_reply_to() {
        let (from, reply_to) =
            manual_sender(&email_config(&["autotherm.hu"]), &staff("janos@gmail.com")).unwrap();
        assert!(from.ends_with("<beszerzes@autotherm.hu>"), "{from}");
        assert_eq!(reply_to.as_deref(), Some("janos@gmail.com"));
        let (from, _) = manual_sender(&email_config(&[]), &staff("kovacs@autotherm.hu")).unwrap();
        assert!(from.ends_with("<beszerzes@autotherm.hu>"), "{from}");
    }

    fn stored_transport(mode: Option<&str>) -> config::Settings {
        config::Settings {
            automatic_email_enabled: false,
            max_auto_emails_per_recipient_day: 3,
            send_window_start: chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            send_window_end: chrono::NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
            send_window_weekdays_only: true,
            nudge_interval_days: 3,
            nudge_escalate_after: 2,
            stage_change_notifications: false,
            stalled_alert_recipients: vec![],
            updated_at: chrono::Utc::now(),
            updated_by: None,
            email_mode: mode.map(str::to_string),
            smtp_host: Some("smtp-relay.gmail.com".into()),
            smtp_port: Some(587),
            smtp_security: Some("starttls".into()),
            smtp_username: None,
            has_password: false,
            smtp_helo_name: None,
            smtp_force_ipv4: None,
            redirect_to: None,
        }
    }

    #[test]
    fn unset_mode_inherits_the_environment() {
        let env = email_config(&[]);
        assert!(resolve_transport(&stored_transport(None), None, &env).is_none());
    }

    #[test]
    fn database_dry_run_wins_over_smtp_environment() {
        let env = email_config(&[]);
        let transport = resolve_transport(&stored_transport(Some("dry_run")), None, &env).unwrap();
        assert!(matches!(
            transport,
            crate::config::EmailTransportConfig::DryRun
        ));
    }

    #[test]
    fn smtp_row_builds_with_env_defaults() {
        let env = email_config(&[]);
        let transport = resolve_transport(&stored_transport(Some("smtp")), None, &env).unwrap();
        let crate::config::EmailTransportConfig::Smtp(smtp) = transport else {
            panic!("expected smtp");
        };
        assert_eq!(smtp.host, "smtp-relay.gmail.com");
        assert_eq!(smtp.port, 587);
        // helo falls back to the environment identity, not a hardcoded name.
        assert_eq!(smtp.helo_name, "autotherm.hu");
    }

    #[test]
    fn broken_rows_fall_back_to_nothing() {
        let env = email_config(&[]);
        let mut row = stored_transport(Some("smtp"));
        row.smtp_host = Some("   ".into());
        assert!(resolve_transport(&row, None, &env).is_none());
        let mut row = stored_transport(Some("smtp"));
        row.smtp_security = Some("pigeon".into());
        assert!(resolve_transport(&row, None, &env).is_none());
    }
}

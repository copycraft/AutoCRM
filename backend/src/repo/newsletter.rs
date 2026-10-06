//! Newsletter subscriptions: the list the office sees under settings, and the audience
//! of a BCC blast.
//!
//! Website signups arrive through the public endpoint (source `website`) as a pending
//! request; the address joins the audience only when its owner clicks the confirmation
//! link (double opt-in). Hand-added rows say `office` and are confirmed on insert.
//! Unsubscribing sets `unsubscribed_at` rather than deleting: a deleted address would
//! silently resubscribe on the next import. Matching is case-insensitive — `Janos@X.HU`
//! and `janos@x.hu` are the same subscriber.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

use crate::domain::email::normalize_address;

/// How long a confirmation link stays valid.
pub const CONFIRM_TTL_DAYS: i32 = 7;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Subscription {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub source: String,
    pub subscribed_at: DateTime<Utc>,
    /// Null while a website signup waits for its confirmation click.
    pub confirmed_at: Option<DateTime<Utc>>,
    pub unsubscribed_at: Option<DateTime<Utc>>,
}

pub async fn list(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<Subscription>> {
    sqlx::query_as!(
        Subscription,
        r#"SELECT id, email, name, source, subscribed_at, confirmed_at, unsubscribed_at
           FROM newsletter_subscriptions ORDER BY subscribed_at DESC"#,
    )
    .fetch_all(db)
    .await
}

pub async fn count_active(db: impl PgExecutor<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        "SELECT count(*) FROM newsletter_subscriptions
          WHERE confirmed_at IS NOT NULL AND unsubscribed_at IS NULL"
    )
    .fetch_one(db)
    .await
    .map(|c: Option<i64>| c.unwrap_or(0))
}

/// Subscribe an address, confirmed. For the office hand-add, where the office holds the
/// consent. Idempotent: subscribing twice is not an error, and a returning address is
/// welcomed back — unsubscribed_at is cleared. The caller decides whether it may be.
pub async fn subscribe(
    db: impl PgExecutor<'_>,
    email: &str,
    name: &str,
    source: &str,
) -> sqlx::Result<Subscription> {
    let Some(normalized) = normalize_address(email) else {
        return Err(sqlx::Error::RowNotFound);
    };
    let name = name.trim();
    sqlx::query_as!(
        Subscription,
        r#"INSERT INTO newsletter_subscriptions (email, name, source, confirmed_at)
           VALUES ($1, $2, $3, now())
           ON CONFLICT (email) DO UPDATE
              SET name = excluded.name, unsubscribed_at = NULL,
                  confirmed_at = coalesce(newsletter_subscriptions.confirmed_at, now()),
                  confirm_token = NULL, confirm_requested_at = NULL
           RETURNING id, email, name, source, subscribed_at, confirmed_at, unsubscribed_at"#,
        normalized,
        name,
        source,
    )
    .fetch_one(db)
    .await
}

/// A website signup waiting for its confirmation click.
#[derive(Debug)]
pub struct PendingConfirmation {
    pub id: i64,
    pub email: String,
    pub token: String,
}

/// Records a website signup and returns the confirmation to mail, or None when the
/// address is already an active subscriber (nothing to confirm, nothing to send).
///
/// Nothing here makes the address active or clears an earlier opt-out: only
/// [`confirm`] does, when the owner of the address clicks the link. A still-valid
/// token is reused, so repeated signups do not invalidate a link already sent; the
/// name of an active subscriber is never overwritten by an unauthenticated form.
pub async fn request_confirmation(
    db: impl PgExecutor<'_>,
    email: &str,
    name: &str,
) -> sqlx::Result<Option<PendingConfirmation>> {
    let Some(normalized) = normalize_address(email) else {
        return Err(sqlx::Error::RowNotFound);
    };
    let row = sqlx::query!(
        r#"INSERT INTO newsletter_subscriptions
                  (email, name, source, confirm_token, confirm_requested_at)
           VALUES ($1, $2, 'website', encode(gen_random_bytes(24), 'hex'), now())
           ON CONFLICT (email) DO UPDATE SET
               name = CASE WHEN newsletter_subscriptions.confirmed_at IS NOT NULL
                                AND newsletter_subscriptions.unsubscribed_at IS NULL
                           THEN newsletter_subscriptions.name ELSE excluded.name END,
               confirm_token = CASE
                   WHEN newsletter_subscriptions.confirmed_at IS NOT NULL
                        AND newsletter_subscriptions.unsubscribed_at IS NULL THEN NULL
                   WHEN newsletter_subscriptions.confirm_token IS NOT NULL
                        AND newsletter_subscriptions.confirm_requested_at
                            > now() - make_interval(days => $3)
                        THEN newsletter_subscriptions.confirm_token
                   ELSE excluded.confirm_token END,
               confirm_requested_at = CASE
                   WHEN newsletter_subscriptions.confirmed_at IS NOT NULL
                        AND newsletter_subscriptions.unsubscribed_at IS NULL THEN NULL
                   ELSE now() END
           RETURNING id, email, confirm_token"#,
        normalized,
        name.trim(),
        CONFIRM_TTL_DAYS,
    )
    .fetch_one(db)
    .await?;
    Ok(row.confirm_token.map(|token| PendingConfirmation {
        id: row.id,
        email: row.email,
        token,
    }))
}

/// The confirmation click: the address joins the audience, and an earlier opt-out is
/// lifted because its owner asked. None for an unknown, used or expired token.
pub async fn confirm(db: impl PgExecutor<'_>, token: &str) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar!(
        "UPDATE newsletter_subscriptions
            SET confirmed_at = now(), unsubscribed_at = NULL,
                confirm_token = NULL, confirm_requested_at = NULL
          WHERE confirm_token = $1
            AND confirm_requested_at > now() - make_interval(days => $2)
          RETURNING id",
        token,
        CONFIRM_TTL_DAYS,
    )
    .fetch_optional(db)
    .await
}

pub async fn unsubscribe_by_token(
    db: impl PgExecutor<'_>,
    token: &str,
) -> sqlx::Result<Option<Subscription>> {
    sqlx::query_as!(
        Subscription,
        r#"UPDATE newsletter_subscriptions SET unsubscribed_at = now()
           WHERE unsubscribe_token = $1 AND unsubscribed_at IS NULL
           RETURNING id, email, name, source, subscribed_at, confirmed_at, unsubscribed_at"#,
        token,
    )
    .fetch_optional(db)
    .await
}

pub async fn remove(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        "DELETE FROM newsletter_subscriptions WHERE id = $1 RETURNING id",
        id
    )
    .fetch_optional(db)
    .await
    .map(|r: Option<i64>| r.is_some())
}

// ── Per-recipient sends (#10/#11) and signup-form tags (#12) ────────────────
// All runtime-checked queries: the `newsletter_sends` / tracking tables are new in
// migration 0046 and have no `.sqlx` cache entries yet.

/// One confirmed reader a send goes to, with the token their unsubscribe link uses.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct AudienceSubscriber {
    pub id: i64,
    pub email: String,
    pub name: String,
    pub unsubscribe_token: String,
}

/// Active addresses a send reaches: confirmed, not unsubscribed, with any of the tags
/// (all active when `tag_ids` is empty), minus the global suppression list.
pub async fn audience_subscribers(
    db: impl PgExecutor<'_>,
    tag_ids: &[i64],
) -> sqlx::Result<Vec<AudienceSubscriber>> {
    sqlx::query_as(
        "SELECT DISTINCT s.id, s.email, s.name, s.unsubscribe_token
           FROM newsletter_subscriptions s
          WHERE s.confirmed_at IS NOT NULL AND s.unsubscribed_at IS NULL
            AND (cardinality($1::bigint[]) = 0 OR EXISTS (
                 SELECT 1 FROM newsletter_subscription_tags k
                 WHERE k.subscription_id = s.id AND k.tag_id = ANY($1)))
            AND NOT EXISTS (SELECT 1 FROM email_suppressions x WHERE x.email = lower(s.email))
          ORDER BY lower(s.email)",
    )
    .bind(tag_ids)
    .fetch_all(db)
    .await
}

/// A website signup's lists, applied when the reader confirms.
pub async fn set_pending_tags(
    db: impl PgExecutor<'_>,
    subscription_id: i64,
    tag_ids: &[i64],
) -> sqlx::Result<()> {
    sqlx::query("UPDATE newsletter_subscriptions SET pending_tag_ids = $2 WHERE id = $1")
        .bind(subscription_id)
        .bind(tag_ids)
        .execute(db)
        .await?;
    Ok(())
}

/// Moves a confirmed subscriber's pending tags onto the subscription and clears them.
/// Called after [`confirm`]; returns how many pairs were new.
pub async fn apply_pending_tags(
    db: impl PgExecutor<'_>,
    subscription_id: i64,
) -> sqlx::Result<u64> {
    let tags: Option<Vec<i64>> = sqlx::query_scalar(
        "SELECT pending_tag_ids FROM newsletter_subscriptions WHERE id = $1",
    )
    .bind(subscription_id)
    .fetch_optional(db)
    .await?
    .flatten();
    let tags = tags.unwrap_or_default();
    if tags.is_empty() {
        return Ok(0);
    }
    let done = sqlx::query(
        "INSERT INTO newsletter_subscription_tags (subscription_id, tag_id)
         SELECT $1, t FROM unnest($2::bigint[]) t
         WHERE EXISTS (SELECT 1 FROM newsletter_tags WHERE id = t AND archived_at IS NULL)
         ON CONFLICT DO NOTHING",
    )
    .bind(subscription_id)
    .bind(&tags)
    .execute(db)
    .await?;
    sqlx::query("UPDATE newsletter_subscriptions SET pending_tag_ids = '{}' WHERE id = $1")
        .bind(subscription_id)
        .execute(db)
        .await?;
    Ok(done.rows_affected())
}

/// One newsletter send: the letter plus when it goes out.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct NewsletterSend {
    pub id: i64,
    pub subject: String,
    pub body: String,
    pub body_markdown: bool,
    pub hero: Option<String>,
    pub tag_ids: Vec<i64>,
    pub attachment_document_ids: Vec<i64>,
    pub embed_document_ids: Vec<i64>,
    pub send_at: chrono::DateTime<chrono::Utc>,
    pub recipients: i32,
    pub created_by: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub cancelled_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[allow(clippy::too_many_arguments)]
pub async fn create_send(
    db: impl PgExecutor<'_>,
    subject: &str,
    body: &str,
    body_markdown: bool,
    hero: Option<&str>,
    tag_ids: &[i64],
    attachment_ids: &[i64],
    embed_ids: &[i64],
    send_at: chrono::DateTime<chrono::Utc>,
    created_by: i64,
) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO newsletter_sends
            (subject, body, body_markdown, hero, tag_ids,
             attachment_document_ids, embed_document_ids, send_at, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING id",
    )
    .bind(subject)
    .bind(body)
    .bind(body_markdown)
    .bind(hero)
    .bind(tag_ids)
    .bind(attachment_ids)
    .bind(embed_ids)
    .bind(send_at)
    .bind(created_by)
    .fetch_one(db)
    .await
}

pub async fn find_send(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<NewsletterSend>> {
    sqlx::query_as(
        "SELECT id, subject, body, body_markdown, hero, tag_ids,
                attachment_document_ids, embed_document_ids,
                send_at, recipients, created_by, created_at, cancelled_at
           FROM newsletter_sends WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await
}

pub async fn list_sends(db: impl PgExecutor<'_>, limit: i64) -> sqlx::Result<Vec<NewsletterSend>> {
    sqlx::query_as(
        "SELECT id, subject, body, body_markdown, hero, tag_ids,
                attachment_document_ids, embed_document_ids,
                send_at, recipients, created_by, created_at, cancelled_at
           FROM newsletter_sends ORDER BY created_at DESC, id DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(db)
    .await
}

pub async fn cancel_send(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE newsletter_sends SET cancelled_at = coalesce(cancelled_at, now())
          WHERE id = $1 AND cancelled_at IS NULL",
    )
    .bind(id)
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn set_send_recipients(
    db: impl PgExecutor<'_>,
    id: i64,
    recipients: i32,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE newsletter_sends SET recipients = $2 WHERE id = $1")
        .bind(id)
        .bind(recipients)
        .execute(db)
        .await?;
    Ok(())
}

/// Per-send results: how many letters, how many opened, how many clicks.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct SendStats {
    pub send_id: i64,
    pub recipients: i64,
    pub queued: i64,
    pub sent: i64,
    pub opened: i64,
    pub opens: i64,
    pub clicks: i64,
    pub clicked: i64,
    pub unsubscribed: i64,
}

pub async fn send_stats(db: impl PgExecutor<'_>, send_id: i64) -> sqlx::Result<SendStats> {
    sqlx::query_as(
        "SELECT $1 AS send_id,
           (SELECT recipients FROM newsletter_sends WHERE id = $1) AS recipients,
           (SELECT count(*) FROM email_messages WHERE newsletter_send_id = $1) AS queued,
           (SELECT count(*) FROM email_messages
             WHERE newsletter_send_id = $1 AND status = 'sent') AS sent,
           (SELECT count(*) FROM email_messages
             WHERE newsletter_send_id = $1 AND opened_at IS NOT NULL) AS opened,
           (SELECT coalesce(sum(open_count), 0) FROM email_messages
             WHERE newsletter_send_id = $1) AS opens,
           (SELECT count(*) FROM email_clicks c
             JOIN email_messages m ON m.id = c.email_id
             WHERE m.newsletter_send_id = $1) AS clicks,
           (SELECT count(DISTINCT c.email_id) FROM email_clicks c
             JOIN email_messages m ON m.id = c.email_id
             WHERE m.newsletter_send_id = $1) AS clicked,
           (SELECT count(*) FROM newsletter_subscriptions s
             WHERE s.unsubscribed_at IS NOT NULL
               AND s.unsubscribed_at >= (SELECT created_at FROM newsletter_sends WHERE id = $1)) AS unsubscribed",
    )
    .bind(send_id)
    .fetch_one(db)
    .await
}

/// An open pixel hit: first open stamps `opened_at`, every hit bumps `open_count`.
/// Returns false for an unknown token.
pub async fn record_open(db: impl PgExecutor<'_>, token: &str) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE email_messages
            SET opened_at = coalesce(opened_at, now()), open_count = open_count + 1
          WHERE tracking_token = $1",
    )
    .bind(token)
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// A tracked-link hit: stores the click and returns where to send the reader.
/// Returns None for an unknown token.
pub async fn record_click(
    db: impl PgExecutor<'_>,
    token: &str,
    url: &str,
) -> sqlx::Result<Option<String>> {
    let id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM email_messages WHERE tracking_token = $1")
            .bind(token)
            .fetch_optional(db)
            .await?;
    let Some(id) = id else { return Ok(None) };
    sqlx::query("INSERT INTO email_clicks (email_id, url) VALUES ($1, $2)")
        .bind(id)
        .bind(url)
        .execute(db)
        .await?;
    Ok(Some(url.to_string()))
}

//! The permanent email log. Rows are inserted before sending and never deleted.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::PgExecutor;

use crate::domain::email::EmailStatus;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct EmailMessage {
    pub id: i64,
    pub order_id: Option<i64>,
    pub lead_id: Option<i64>,
    pub partner_id: Option<i64>,
    pub blocker_id: Option<i64>,
    pub template_key: Option<String>,
    pub trigger: String,
    pub is_automatic: bool,
    pub sent_by: Option<i64>,
    pub to_address: String,
    pub cc: Vec<String>,
    /// A newsletter blast's recipients. Everyone else's rows leave this empty.
    pub bcc: Vec<String>,
    pub from_address: String,
    pub reply_to: Option<String>,
    pub subject: String,
    pub body_html: String,
    pub body_text: String,
    #[schema(value_type = Vec<crate::service::email::AttachmentRef>)]
    pub attachments: Value,
    pub status: EmailStatus,
    pub provider_id: Option<String>,
    pub error: Option<String>,
    pub attempts: i32,
    pub queued_at: DateTime<Utc>,
    pub send_after: DateTime<Utc>,
    pub sending_started_at: Option<DateTime<Utc>>,
    pub sent_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub cancelled_by: Option<i64>,
    /// The received message this answers (0049).
    pub in_reply_to: Option<String>,
    pub reference_ids: Option<String>,
}

/// List view: everything except the bodies.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct EmailSummary {
    pub id: i64,
    pub order_id: Option<i64>,
    pub lead_id: Option<i64>,
    pub partner_id: Option<i64>,
    pub blocker_id: Option<i64>,
    pub template_key: Option<String>,
    pub trigger: String,
    pub is_automatic: bool,
    pub sent_by: Option<i64>,
    pub sent_by_name: Option<String>,
    pub to_address: String,
    pub subject: String,
    pub status: EmailStatus,
    pub error: Option<String>,
    pub queued_at: DateTime<Utc>,
    pub send_after: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
}

pub struct NewEmail<'a> {
    pub order_id: Option<i64>,
    pub lead_id: Option<i64>,
    pub partner_id: Option<i64>,
    pub blocker_id: Option<i64>,
    pub template_key: Option<&'a str>,
    pub trigger: &'a str,
    pub sent_by: Option<i64>,
    pub idempotency_key: Option<&'a str>,
    pub to_address: &'a str,
    pub cc: &'a [String],
    pub bcc: &'a [String],
    pub from_address: &'a str,
    pub reply_to: Option<&'a str>,
    pub subject: &'a str,
    pub body_html: &'a str,
    pub body_text: &'a str,
    pub attachments: Value,
    pub send_after: Option<DateTime<Utc>>,
}

/// Returns None when the idempotency key was already used (nothing inserted).
pub async fn insert(db: impl PgExecutor<'_>, e: &NewEmail<'_>) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar!(
        "INSERT INTO email_messages (order_id, lead_id, partner_id, blocker_id, template_key, trigger, is_automatic, sent_by,
                                     idempotency_key, to_address, cc, bcc, from_address, reply_to, subject, body_html, body_text,
                                     attachments, send_after)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, coalesce($19, now()))
         ON CONFLICT (idempotency_key) DO NOTHING
         RETURNING id",
        e.order_id,
        e.lead_id,
        e.partner_id,
        e.blocker_id,
        e.template_key,
        e.trigger,
        e.sent_by.is_none(),
        e.sent_by,
        e.idempotency_key,
        e.to_address,
        e.cc,
        e.bcc,
        e.from_address,
        e.reply_to,
        e.subject,
        e.body_html,
        e.body_text,
        e.attachments,
        e.send_after
    )
    .fetch_optional(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<EmailMessage>> {
    sqlx::query_as!(
        EmailMessage,
        r#"SELECT id, order_id, lead_id, partner_id, blocker_id, template_key, trigger, is_automatic, sent_by, to_address, cc, bcc,
                  from_address, reply_to, subject, body_html, body_text, attachments, status AS "status: EmailStatus",
                  provider_id, error, attempts, queued_at, send_after, sending_started_at, sent_at, cancelled_at, cancelled_by,
                  in_reply_to, reference_ids
           FROM email_messages WHERE id = $1"#,
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn lock(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<EmailMessage>> {
    sqlx::query_as!(
        EmailMessage,
        r#"SELECT id, order_id, lead_id, partner_id, blocker_id, template_key, trigger, is_automatic, sent_by, to_address, cc, bcc,
                  from_address, reply_to, subject, body_html, body_text, attachments, status AS "status: EmailStatus",
                  provider_id, error, attempts, queued_at, send_after, sending_started_at, sent_at, cancelled_at, cancelled_by,
                  in_reply_to, reference_ids
           FROM email_messages WHERE id = $1 FOR UPDATE"#,
        id
    )
    .fetch_optional(db)
    .await
}

#[derive(Debug, Default, Clone)]
pub struct EmailFilter {
    pub order_id: Option<i64>,
    pub lead_id: Option<i64>,
    /// Mail about the partner directly, or about any of its orders or leads.
    pub partner_id: Option<i64>,
    pub status: Option<EmailStatus>,
    pub needs_attention: bool,
    /// Free text over subject and recipient. Served server-side so the inbox searches
    /// the whole log, not just the loaded page.
    pub q: Option<String>,
}

pub async fn list(
    db: impl PgExecutor<'_>,
    f: &EmailFilter,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<EmailSummary>> {
    sqlx::query_as!(
        EmailSummary,
        r#"SELECT m.id, m.order_id, m.lead_id, m.partner_id, m.blocker_id, m.template_key, m.trigger, m.is_automatic,
                  m.sent_by, u.display_name AS "sent_by_name?", m.to_address, m.subject, m.status AS "status: EmailStatus",
                  m.error, m.queued_at, m.send_after, m.sent_at
           FROM email_messages m
           LEFT JOIN users u ON u.id = m.sent_by
           WHERE ($1::bigint IS NULL OR m.order_id = $1)
             AND ($2::bigint IS NULL OR m.lead_id = $2)
             AND ($3::bigint IS NULL OR m.partner_id = $3
                  OR m.order_id IN (SELECT id FROM orders WHERE partner_id = $3)
                  OR m.lead_id IN (SELECT id FROM leads WHERE partner_id = $3))
              AND ($4::email_status IS NULL OR m.status = $4)
              AND (NOT $5 OR m.status IN ('failed', 'needs_review'))
              AND ($8::text IS NULL OR m.subject ILIKE $8 OR m.to_address ILIKE $8)
            ORDER BY m.queued_at DESC, m.id DESC
            LIMIT $6 OFFSET $7"#,
        f.order_id,
        f.lead_id,
        f.partner_id,
        f.status as Option<EmailStatus>,
        f.needs_attention,
        limit,
        offset,
        f.q.as_deref().and_then(crate::repo::like_pattern)
    )
    .fetch_all(db)
    .await
}

pub async fn mark_sending(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE email_messages SET status = 'sending', attempts = attempts + 1, sending_started_at = now() WHERE id = $1",
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn mark_sent(
    db: impl PgExecutor<'_>,
    id: i64,
    provider_id: &str,
    attachments: Value,
) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE email_messages SET status = 'sent', sent_at = now(), provider_id = $2, attachments = $3, error = NULL WHERE id = $1",
        id,
        provider_id,
        attachments
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Used when download links replace oversized attachments, so the log shows what was sent.
pub async fn set_bodies(
    db: impl PgExecutor<'_>,
    id: i64,
    body_text: &str,
    body_html: &str,
) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE email_messages SET body_text = $2, body_html = $3 WHERE id = $1",
        id,
        body_text,
        body_html
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_status(
    db: impl PgExecutor<'_>,
    id: i64,
    status: EmailStatus,
    error: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE email_messages SET status = $2, error = $3 WHERE id = $1",
        id,
        status as EmailStatus,
        error
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn requeue(
    db: impl PgExecutor<'_>,
    id: i64,
    send_after: DateTime<Utc>,
    error: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE email_messages SET status = 'queued', send_after = $2, error = $3 WHERE id = $1",
        id,
        send_after,
        error
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn cancel(
    db: impl PgExecutor<'_>,
    id: i64,
    reason: &str,
    user_id: Option<i64>,
) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "UPDATE email_messages SET status = 'cancelled', error = $2, cancelled_at = now(), cancelled_by = $3
         WHERE id = $1 AND status IN ('queued', 'failed', 'needs_review')",
        id,
        reason,
        user_id
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

pub async fn count_needing_attention(db: impl PgExecutor<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM email_messages WHERE status IN ('failed', 'needs_review')"#
    )
    .fetch_one(db)
    .await
}

pub async fn automatic_sent_last_24h(db: impl PgExecutor<'_>, address: &str) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM email_messages
           WHERE is_automatic AND lower(to_address) = lower($1) AND status IN ('sent', 'sending')
             AND coalesce(sent_at, sending_started_at) > now() - interval '24 hours'"#,
        address
    )
    .fetch_one(db)
    .await
}

pub async fn is_suppressed(db: impl PgExecutor<'_>, address: &str) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM email_suppressions WHERE email = lower($1)) AS "e!""#,
        address
    )
    .fetch_one(db)
    .await
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Suppression {
    pub email: String,
    pub reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<i64>,
}

pub async fn list_suppressions(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<Suppression>> {
    sqlx::query_as!(
        Suppression,
        "SELECT email, reason, created_at, created_by FROM email_suppressions ORDER BY email"
    )
    .fetch_all(db)
    .await
}

pub async fn add_suppression(
    db: impl PgExecutor<'_>,
    email: &str,
    reason: Option<&str>,
    user_id: i64,
) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO email_suppressions (email, reason, created_by) VALUES (lower($1), $2, $3)
         ON CONFLICT (email) DO UPDATE SET reason = EXCLUDED.reason",
        email,
        reason,
        user_id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn remove_suppression(db: impl PgExecutor<'_>, email: &str) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "DELETE FROM email_suppressions WHERE email = lower($1)",
        email
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// Threading headers of a reply (0049); set right after the insert, before the send job.
pub async fn set_threading(
    db: impl PgExecutor<'_>,
    id: i64,
    in_reply_to: Option<&str>,
    reference_ids: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE email_messages SET in_reply_to = $2, reference_ids = $3 WHERE id = $1",
        id,
        in_reply_to,
        reference_ids
    )
    .execute(db)
    .await?;
    Ok(())
}

/// One letter in a conversation: ours (out) or the customer's (in), oldest first.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema, sqlx::FromRow)]
pub struct ConversationItem {
    /// out or in.
    pub direction: String,
    /// email_messages.id (out) or inbound_emails.id (in).
    pub id: i64,
    pub at: DateTime<Utc>,
    pub from_address: String,
    pub from_name: Option<String>,
    pub to_address: Option<String>,
    pub subject: String,
    pub body_text: String,
    /// Out only: queued, sent, failed, …
    pub status: Option<String>,
    /// Out only: who wrote it; None for automatic mail.
    pub sent_by_name: Option<String>,
    /// In only: the letter of ours it answers.
    pub reply_to_email_id: Option<i64>,
}

/// Everything written with a lead's customer: our letters about the lead (and its order,
/// once won) and what came back. Newsletters are left out.
pub async fn lead_conversation(
    db: impl PgExecutor<'_>,
    lead_id: i64,
) -> sqlx::Result<Vec<ConversationItem>> {
    sqlx::query_as(
        "SELECT * FROM (
             SELECT 'out' AS direction, m.id, coalesce(m.sent_at, m.queued_at) AS at,
                    m.from_address, NULL::text AS from_name, m.to_address, m.subject, m.body_text,
                    m.status::text AS status, u.display_name AS sent_by_name,
                    NULL::bigint AS reply_to_email_id
               FROM email_messages m
               LEFT JOIN users u ON u.id = m.sent_by
              WHERE (m.lead_id = $1 OR m.order_id = (SELECT o.id FROM orders o WHERE o.lead_id = $1))
                AND m.trigger NOT IN ('newsletter', 'newsletter_send', 'website_lead_alert')
                AND m.status::text <> 'cancelled'
             UNION ALL
             SELECT 'in', i.id, i.received_at, i.from_address, i.from_name, NULL, i.subject, i.body_text,
                    NULL, NULL, i.reply_to_email_id
               FROM inbound_emails i
              WHERE i.lead_id = $1 OR i.order_id = (SELECT o.id FROM orders o WHERE o.lead_id = $1)
         ) t
         ORDER BY at, direction DESC, id",
    )
    .bind(lead_id)
    .fetch_all(db)
    .await
}

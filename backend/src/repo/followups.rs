//! Quote follow-ups: the default sequence (`followup_steps`) and each lead's scheduled
//! letters (`lead_followups`). Runtime-checked queries, like the other recent tables.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct FollowupStep {
    pub id: i64,
    /// What the office calls it: "1 hét", "1 hónap".
    pub label: String,
    /// Days after the quotation was sent.
    pub delay_days: i32,
    /// The email template it sends.
    pub template_key: String,
    pub template_name: String,
    /// Inactive steps are not scheduled for new quotations.
    pub is_active: bool,
    /// quote (days after a quotation) or invoice (days after the payment deadline).
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct Followup {
    pub id: i64,
    pub lead_id: i64,
    pub label: String,
    pub template_key: String,
    pub template_name: String,
    pub due_at: DateTime<Utc>,
    /// scheduled | sent | cancelled | skipped
    pub status: String,
    pub email_id: Option<i64>,
    /// Why it was cancelled or skipped.
    pub note: Option<String>,
    pub created_by_name: Option<String>,
    pub updated_at: DateTime<Utc>,
}

const STEP: &str = "SELECT s.id, s.label, s.delay_days, s.template_key, t.name AS template_name, s.is_active, s.kind
    FROM followup_steps s JOIN email_templates t ON t.key = s.template_key";

pub async fn steps(db: impl PgExecutor<'_>, kind: &str) -> sqlx::Result<Vec<FollowupStep>> {
    sqlx::query_as(&format!(
        "{STEP} WHERE s.kind = $1 ORDER BY s.delay_days, s.id"
    ))
    .bind(kind)
    .fetch_all(db)
    .await
}

pub async fn step(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<FollowupStep>> {
    sqlx::query_as(&format!("{STEP} WHERE s.id = $1"))
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn insert_step(
    db: impl PgExecutor<'_>,
    label: &str,
    delay_days: i32,
    template_key: &str,
    kind: &str,
) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO followup_steps (label, delay_days, template_key, kind) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(label)
    .bind(delay_days)
    .bind(template_key)
    .bind(kind)
    .fetch_one(db)
    .await
}

pub async fn update_step(
    db: impl PgExecutor<'_>,
    id: i64,
    label: &str,
    delay_days: i32,
    template_key: &str,
    is_active: bool,
) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE followup_steps SET label = $2, delay_days = $3, template_key = $4, is_active = $5
         WHERE id = $1",
    )
    .bind(id)
    .bind(label)
    .bind(delay_days)
    .bind(template_key)
    .bind(is_active)
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Whether an email template with this key exists.
pub async fn template_exists(db: impl PgExecutor<'_>, key: &str) -> sqlx::Result<bool> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM email_templates WHERE key = $1)")
        .bind(key)
        .fetch_one(db)
        .await
}

const FOLLOWUP: &str =
    "SELECT f.id, f.lead_id, f.label, f.template_key, t.name AS template_name, f.due_at,
        f.status, f.email_id, f.note, u.display_name AS created_by_name, f.updated_at
    FROM lead_followups f
    JOIN email_templates t ON t.key = f.template_key
    LEFT JOIN users u ON u.id = f.created_by";

pub async fn for_lead(db: impl PgExecutor<'_>, lead_id: i64) -> sqlx::Result<Vec<Followup>> {
    sqlx::query_as(&format!(
        "{FOLLOWUP} WHERE f.lead_id = $1 ORDER BY f.due_at, f.id"
    ))
    .bind(lead_id)
    .fetch_all(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Followup>> {
    sqlx::query_as(&format!("{FOLLOWUP} WHERE f.id = $1"))
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn schedule(
    db: impl PgExecutor<'_>,
    lead_id: i64,
    label: &str,
    template_key: &str,
    due_at: DateTime<Utc>,
    created_by: Option<i64>,
) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO lead_followups (lead_id, label, template_key, due_at, created_by)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(lead_id)
    .bind(label)
    .bind(template_key)
    .bind(due_at)
    .bind(created_by)
    .fetch_one(db)
    .await
}

/// Cancels one scheduled follow-up; false when it was not scheduled (already sent...).
pub async fn cancel(db: impl PgExecutor<'_>, id: i64, note: &str) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE lead_followups SET status = 'cancelled', note = $2 WHERE id = $1 AND status = 'scheduled'",
    )
    .bind(id)
    .bind(note)
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Cancels every scheduled follow-up of a lead; returns how many.
pub async fn cancel_for_lead(
    db: impl PgExecutor<'_>,
    lead_id: i64,
    note: &str,
) -> sqlx::Result<u64> {
    let done = sqlx::query(
        "UPDATE lead_followups SET status = 'cancelled', note = $2
         WHERE lead_id = $1 AND status = 'scheduled'",
    )
    .bind(lead_id)
    .bind(note)
    .execute(db)
    .await?;
    Ok(done.rows_affected())
}

/// A due follow-up with what deciding about it needs, row-locked so two workers never
/// send the same letter.
#[derive(Debug, sqlx::FromRow)]
pub struct Due {
    pub id: i64,
    pub lead_id: i64,
    pub template_key: String,
    /// The lead's stage is final (won, lost...).
    pub lead_closed: bool,
    /// The lead already became an order.
    pub converted: bool,
    pub contact_email: Option<String>,
    pub partner_email: Option<String>,
    /// Who scheduled it (sent the quotation): the letter goes out in their name.
    pub sender_name: Option<String>,
    pub sender_email: Option<String>,
}

pub async fn lock_due(
    db: impl PgExecutor<'_>,
    now: DateTime<Utc>,
    limit: i64,
) -> sqlx::Result<Vec<Due>> {
    sqlx::query_as(
        "SELECT f.id, f.lead_id, f.template_key,
                coalesce(sd.is_terminal, false) AS lead_closed,
                EXISTS (SELECT 1 FROM orders o WHERE o.lead_id = f.lead_id) AS converted,
                l.contact_email, p.email AS partner_email,
                u.display_name AS sender_name, u.email AS sender_email
         FROM lead_followups f
         JOIN leads l ON l.id = f.lead_id
         LEFT JOIN users u ON u.id = f.created_by AND u.is_active
         LEFT JOIN partners p ON p.id = l.partner_id
         LEFT JOIN lead_current_stage cs ON cs.lead_id = f.lead_id
         LEFT JOIN stage_definitions sd ON sd.entity = 'lead' AND sd.key = cs.stage_key
         WHERE f.status = 'scheduled' AND f.due_at <= $1
         ORDER BY f.due_at, f.id
         LIMIT $2
         FOR UPDATE OF f SKIP LOCKED",
    )
    .bind(now)
    .bind(limit)
    .fetch_all(db)
    .await
}

pub async fn mark_sent(db: impl PgExecutor<'_>, id: i64, email_id: i64) -> sqlx::Result<()> {
    sqlx::query("UPDATE lead_followups SET status = 'sent', email_id = $2 WHERE id = $1")
        .bind(id)
        .bind(email_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn mark_skipped(db: impl PgExecutor<'_>, id: i64, note: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE lead_followups SET status = 'skipped', note = $2 WHERE id = $1")
        .bind(id)
        .bind(note)
        .execute(db)
        .await?;
    Ok(())
}

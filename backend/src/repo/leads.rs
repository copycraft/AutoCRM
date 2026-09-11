use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Lead {
    pub id: i64,
    pub title: String,
    pub partner_id: Option<i64>,
    pub contact_id: Option<i64>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub source: Option<String>,
    pub description: Option<String>,
    pub assigned_to: Option<i64>,
    pub created_by: Option<i64>,
    pub minicrm_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LeadInput {
    pub title: String,
    pub partner_id: Option<i64>,
    pub contact_id: Option<i64>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub source: Option<String>,
    pub description: Option<String>,
    pub assigned_to: Option<i64>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct LeadSummary {
    pub id: i64,
    pub title: String,
    pub partner_id: Option<i64>,
    pub partner_name: Option<String>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub source: Option<String>,
    pub assigned_to: Option<i64>,
    pub assigned_name: Option<String>,
    pub stage_key: String,
    pub stage_label: String,
    pub stage_entered_at: DateTime<Utc>,
    pub order_id: Option<i64>,
    pub order_number: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Lead>> {
    sqlx::query_as!(
        Lead,
        "SELECT id, title, partner_id, contact_id, contact_name, contact_email, contact_phone, source, description,
                assigned_to, created_by, minicrm_id, created_at, updated_at
         FROM leads WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await
}

/// Row-locks the lead for the rest of the transaction.
pub async fn lock(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Lead>> {
    sqlx::query_as!(
        Lead,
        "SELECT id, title, partner_id, contact_id, contact_name, contact_email, contact_phone, source, description,
                assigned_to, created_by, minicrm_id, created_at, updated_at
         FROM leads WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn insert(db: impl PgExecutor<'_>, l: &LeadInput, created_by: i64) -> sqlx::Result<Lead> {
    sqlx::query_as!(
        Lead,
        "INSERT INTO leads (title, partner_id, contact_id, contact_name, contact_email, contact_phone, source,
                            description, assigned_to, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING id, title, partner_id, contact_id, contact_name, contact_email, contact_phone, source, description,
                   assigned_to, created_by, minicrm_id, created_at, updated_at",
        l.title,
        l.partner_id,
        l.contact_id,
        l.contact_name,
        l.contact_email,
        l.contact_phone,
        l.source,
        l.description,
        l.assigned_to,
        created_by
    )
    .fetch_one(db)
    .await
}

pub async fn update(db: impl PgExecutor<'_>, id: i64, l: &LeadInput) -> sqlx::Result<Option<Lead>> {
    sqlx::query_as!(
        Lead,
        "UPDATE leads
         SET title = $2, partner_id = $3, contact_id = $4, contact_name = $5, contact_email = $6, contact_phone = $7,
             source = $8, description = $9, assigned_to = $10
         WHERE id = $1
         RETURNING id, title, partner_id, contact_id, contact_name, contact_email, contact_phone, source, description,
                   assigned_to, created_by, minicrm_id, created_at, updated_at",
        id,
        l.title,
        l.partner_id,
        l.contact_id,
        l.contact_name,
        l.contact_email,
        l.contact_phone,
        l.source,
        l.description,
        l.assigned_to
    )
    .fetch_optional(db)
    .await
}

pub async fn set_partner(db: impl PgExecutor<'_>, id: i64, partner_id: i64) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE leads SET partner_id = $2 WHERE id = $1",
        id,
        partner_id
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn search(
    db: impl PgExecutor<'_>,
    pattern: Option<&str>,
    stage_key: Option<&str>,
    assigned_to: Option<i64>,
    open_only: bool,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<LeadSummary>> {
    sqlx::query_as!(
        LeadSummary,
        r#"SELECT l.id, l.title, l.partner_id, p.name AS "partner_name?", l.contact_name, l.contact_email, l.source,
                  l.assigned_to, u.display_name AS "assigned_name?",
                  cs.stage_key AS "stage_key!", sd.label_hu AS "stage_label!", cs.entered_at AS "stage_entered_at!",
                  o.id AS "order_id?", o.number AS "order_number?", l.created_at
           FROM leads l
           JOIN lead_current_stage cs ON cs.lead_id = l.id
           JOIN stage_definitions sd ON sd.entity = 'lead' AND sd.key = cs.stage_key
           LEFT JOIN partners p ON p.id = l.partner_id
           LEFT JOIN users u ON u.id = l.assigned_to
           LEFT JOIN orders o ON o.lead_id = l.id
           WHERE ($1::text IS NULL OR l.title ILIKE $1 OR l.contact_name ILIKE $1 OR l.contact_email ILIKE $1 OR p.name ILIKE $1)
             AND ($2::text IS NULL OR cs.stage_key = $2)
             AND ($3::bigint IS NULL OR l.assigned_to = $3)
             AND (NOT $4 OR NOT sd.is_terminal)
           ORDER BY l.created_at DESC, l.id DESC
           LIMIT $5 OFFSET $6"#,
        pattern,
        stage_key,
        assigned_to,
        open_only,
        limit,
        offset
    )
    .fetch_all(db)
    .await
}

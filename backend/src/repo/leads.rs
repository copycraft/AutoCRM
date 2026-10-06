use chrono::{DateTime, NaiveDate, Utc};
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
    /// V2.3: what we quoted, in minor units. Not a `quotes` table with versions — at 60
    /// leads a month that is more machinery than the business justifies. Changes are
    /// written to audit_log, so a revised price still leaves a trail.
    pub quoted_value_minor: Option<i64>,
    #[schema(value_type = Option<crate::domain::money::Currency>)]
    pub currency: Option<String>,
    pub quote_valid_until: Option<NaiveDate>,
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
    pub quoted_value_minor: Option<i64>,
    pub currency: Option<String>,
    pub quote_valid_until: Option<NaiveDate>,
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
    pub quoted_value_minor: Option<i64>,
    pub currency: Option<String>,
    pub quote_valid_until: Option<NaiveDate>,
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
                assigned_to, quoted_value_minor, currency, quote_valid_until,
                created_by, minicrm_id, created_at, updated_at
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
                assigned_to, quoted_value_minor, currency, quote_valid_until,
                created_by, minicrm_id, created_at, updated_at
         FROM leads WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn insert(db: impl PgExecutor<'_>, l: &LeadInput, created_by: i64) -> sqlx::Result<Lead> {
    insert_by(db, l, Some(created_by)).await
}

/// `created_by` is None for website leads: no staff user is behind them.
pub async fn insert_by(
    db: impl PgExecutor<'_>,
    l: &LeadInput,
    created_by: Option<i64>,
) -> sqlx::Result<Lead> {
    sqlx::query_as!(
        Lead,
        "INSERT INTO leads (title, partner_id, contact_id, contact_name, contact_email, contact_phone, source,
                            description, assigned_to, quoted_value_minor, currency, quote_valid_until, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
         RETURNING id, title, partner_id, contact_id, contact_name, contact_email, contact_phone, source, description,
                   assigned_to, quoted_value_minor, currency, quote_valid_until,
                   created_by, minicrm_id, created_at, updated_at",
        l.title,
        l.partner_id,
        l.contact_id,
        l.contact_name,
        l.contact_email,
        l.contact_phone,
        l.source,
        l.description,
        l.assigned_to,
        l.quoted_value_minor,
        l.currency,
        l.quote_valid_until,
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
             source = $8, description = $9, assigned_to = $10, quoted_value_minor = $11, currency = $12,
             quote_valid_until = $13
         WHERE id = $1
         RETURNING id, title, partner_id, contact_id, contact_name, contact_email, contact_phone, source, description,
                   assigned_to, quoted_value_minor, currency, quote_valid_until,
                   created_by, minicrm_id, created_at, updated_at",
        id,
        l.title,
        l.partner_id,
        l.contact_id,
        l.contact_name,
        l.contact_email,
        l.contact_phone,
        l.source,
        l.description,
        l.assigned_to,
        l.quoted_value_minor,
        l.currency,
        l.quote_valid_until
    )
    .fetch_optional(db)
    .await
}

pub async fn set_assigned(
    db: impl PgExecutor<'_>,
    id: i64,
    assigned_to: Option<i64>,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE leads SET assigned_to = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(assigned_to)
        .execute(db)
        .await?;
    Ok(())
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

/// Sort keys accepted by lead search (`-` prefix for descending).
/// The value travels to SQL as a bind parameter matched against static CASE
/// branches, so the query stays fully compile-time checked.
pub const LEAD_SORTS: &[&str] = &["created_at", "title"];

pub const DEFAULT_SORT: &str = "-created_at";

// Filter queries take one parameter per filter; bundling would only rename them.
#[allow(clippy::too_many_arguments)]
pub async fn search(
    db: impl PgExecutor<'_>,
    pattern: Option<&str>,
    phone: Option<&str>,
    stage_key: Option<&str>,
    assigned_to: Option<i64>,
    tag_id: Option<i64>,
    open_only: bool,
    sort_key: &str,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<LeadSummary>> {
    sqlx::query_as!(
        LeadSummary,
        r#"SELECT l.id, l.title, l.partner_id, p.name AS "partner_name?", l.contact_name, l.contact_email, l.source,
                  l.assigned_to, u.display_name AS "assigned_name?",
                  l.quoted_value_minor, l.currency, l.quote_valid_until,
                  cs.stage_key AS "stage_key!", sd.label_hu AS "stage_label!", cs.entered_at AS "stage_entered_at!",
                  o.id AS "order_id?", o.number AS "order_number?", l.created_at
           FROM leads l
           JOIN lead_current_stage cs ON cs.lead_id = l.id
           JOIN stage_definitions sd ON sd.entity = 'lead' AND sd.key = cs.stage_key
           LEFT JOIN partners p ON p.id = l.partner_id
           LEFT JOIN users u ON u.id = l.assigned_to
           -- V2.7: a lead converts as many times as the enquiry had vehicles, so this is a
           -- LATERAL taking the first order rather than a join that would duplicate the lead.
           LEFT JOIN LATERAL (SELECT id, number FROM orders WHERE lead_id = l.id ORDER BY id LIMIT 1) o ON true
            WHERE ($1::text IS NULL OR l.title ILIKE $1 OR l.contact_name ILIKE $1 OR l.contact_email ILIKE $1 OR p.name ILIKE $1
                   -- Contact phone digits with Hungarian prefixes unified, mirroring
                   -- domain::partner::normalize_phone.
                   OR ($2::text IS NOT NULL AND regexp_replace(regexp_replace(regexp_replace(l.contact_phone, '[^0-9]', '', 'g'), '^00', ''), '^06', '36') LIKE $2))
             AND ($3::text IS NULL OR cs.stage_key = $3)
             AND ($4::bigint IS NULL OR l.assigned_to = $4)
             AND (NOT $5 OR NOT sd.is_terminal)
             AND ($9::bigint IS NULL OR EXISTS (SELECT 1 FROM lead_tag_links k WHERE k.lead_id = l.id AND k.tag_id = $9))
           ORDER BY
               CASE WHEN $8 = 'created_at' THEN l.created_at END ASC,
               CASE WHEN $8 = '-created_at' THEN l.created_at END DESC,
               CASE WHEN $8 = 'title' THEN l.title END ASC,
               CASE WHEN $8 = '-title' THEN l.title END DESC,
               l.id DESC
           LIMIT $6 OFFSET $7"#,
        pattern,
        phone,
        stage_key,
        assigned_to,
        open_only,
        limit,
        offset,
        sort_key,
        tag_id
    )
    .fetch_all(db)
    .await
}

/// Every lead of one partner, newest first (V6). One more query on the partner detail, so
/// that opening a customer shows the quotations that never became orders.
pub async fn for_partner(
    db: impl PgExecutor<'_>,
    partner_id: i64,
) -> sqlx::Result<Vec<LeadSummary>> {
    sqlx::query_as!(
        LeadSummary,
        r#"SELECT l.id, l.title, l.partner_id, p.name AS "partner_name?", l.contact_name, l.contact_email, l.source,
                  l.assigned_to, u.display_name AS "assigned_name?",
                  l.quoted_value_minor, l.currency, l.quote_valid_until,
                  cs.stage_key AS "stage_key!", sd.label_hu AS "stage_label!", cs.entered_at AS "stage_entered_at!",
                  o.id AS "order_id?", o.number AS "order_number?", l.created_at
           FROM leads l
           JOIN lead_current_stage cs ON cs.lead_id = l.id
           JOIN stage_definitions sd ON sd.entity = 'lead' AND sd.key = cs.stage_key
           LEFT JOIN partners p ON p.id = l.partner_id
           LEFT JOIN users u ON u.id = l.assigned_to
           LEFT JOIN LATERAL (SELECT id, number FROM orders WHERE lead_id = l.id ORDER BY id LIMIT 1) o ON true
           WHERE l.partner_id = $1
           ORDER BY l.created_at DESC, l.id DESC"#,
        partner_id
    )
    .fetch_all(db)
    .await
}

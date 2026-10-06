use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct EmailTemplate {
    pub id: i64,
    pub key: String,
    pub name: String,
    pub subject: String,
    pub body: String,
    pub locale: String,
    pub is_automatic: bool,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<i64>,
    /// The area it belongs to (the tabs): sales, projects, billing, marketing, hr, general.
    pub category: String,
    /// customer (to customers), workflow (for the office), design (samples to copy).
    pub folder: String,
    /// Set when moved to the bin.
    pub archived_at: Option<DateTime<Utc>>,
}

pub async fn list(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<EmailTemplate>> {
    sqlx::query_as!(
        EmailTemplate,
        "SELECT id, key, name, subject, body, locale, is_automatic, updated_at, updated_by, category, folder, archived_at FROM email_templates ORDER BY name"
    )
    .fetch_all(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<EmailTemplate>> {
    sqlx::query_as!(
        EmailTemplate,
        "SELECT id, key, name, subject, body, locale, is_automatic, updated_at, updated_by, category, folder, archived_at FROM email_templates WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn find_by_key(
    db: impl PgExecutor<'_>,
    key: &str,
) -> sqlx::Result<Option<EmailTemplate>> {
    sqlx::query_as!(
        EmailTemplate,
        "SELECT id, key, name, subject, body, locale, is_automatic, updated_at, updated_by, category, folder, archived_at FROM email_templates WHERE key = $1",
        key
    )
    .fetch_optional(db)
    .await
}

// One parameter per column; bundling them would only rename them.
#[allow(clippy::too_many_arguments)]
pub async fn insert(
    db: impl PgExecutor<'_>,
    key: &str,
    name: &str,
    subject: &str,
    body: &str,
    category: &str,
    folder: &str,
    user_id: i64,
) -> sqlx::Result<EmailTemplate> {
    sqlx::query_as!(
        EmailTemplate,
        "INSERT INTO email_templates (key, name, subject, body, updated_by, category, folder)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id, key, name, subject, body, locale, is_automatic, updated_at, updated_by, category, folder, archived_at",
        key,
        name,
        subject,
        body,
        user_id,
        category,
        folder
    )
    .fetch_one(db)
    .await
}

/// Key and is_automatic are fixed: code refers to the key, and whether a template is
/// sent without a human is a property of the code path using it.
#[allow(clippy::too_many_arguments)]
pub async fn update(
    db: impl PgExecutor<'_>,
    id: i64,
    name: &str,
    subject: &str,
    body: &str,
    category: &str,
    folder: &str,
    archived: bool,
    user_id: i64,
) -> sqlx::Result<Option<EmailTemplate>> {
    sqlx::query_as!(
        EmailTemplate,
        "UPDATE email_templates SET name = $2, subject = $3, body = $4, updated_by = $5,
             category = $6, folder = $7,
             archived_at = CASE WHEN $8 THEN coalesce(archived_at, now()) END
         WHERE id = $1
         RETURNING id, key, name, subject, body, locale, is_automatic, updated_at, updated_by, category, folder, archived_at",
        id,
        name,
        subject,
        body,
        user_id,
        category,
        folder,
        archived
    )
    .fetch_optional(db)
    .await
}

/// Why a template must stay usable: code sends it, or a follow-up step does.
pub async fn in_use(db: impl PgExecutor<'_>, key: &str, is_automatic: bool) -> sqlx::Result<Option<String>> {
    if is_automatic {
        return Ok(Some("automatikus levél használja".into()));
    }
    let step: Option<String> = sqlx::query_scalar(
        "SELECT label FROM followup_steps WHERE template_key = $1 AND is_active LIMIT 1",
    )
    .bind(key)
    .fetch_optional(db)
    .await?;
    Ok(step.map(|s| format!("a(z) „{s}” utánkövetés használja")))
}

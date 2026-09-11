use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

#[derive(Debug, Clone, Serialize)]
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
}

pub async fn list(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<EmailTemplate>> {
    sqlx::query_as!(
        EmailTemplate,
        "SELECT id, key, name, subject, body, locale, is_automatic, updated_at, updated_by FROM email_templates ORDER BY name"
    )
    .fetch_all(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<EmailTemplate>> {
    sqlx::query_as!(
        EmailTemplate,
        "SELECT id, key, name, subject, body, locale, is_automatic, updated_at, updated_by FROM email_templates WHERE id = $1",
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
        "SELECT id, key, name, subject, body, locale, is_automatic, updated_at, updated_by FROM email_templates WHERE key = $1",
        key
    )
    .fetch_optional(db)
    .await
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    key: &str,
    name: &str,
    subject: &str,
    body: &str,
    user_id: i64,
) -> sqlx::Result<EmailTemplate> {
    sqlx::query_as!(
        EmailTemplate,
        "INSERT INTO email_templates (key, name, subject, body, updated_by) VALUES ($1, $2, $3, $4, $5)
         RETURNING id, key, name, subject, body, locale, is_automatic, updated_at, updated_by",
        key,
        name,
        subject,
        body,
        user_id
    )
    .fetch_one(db)
    .await
}

/// Key and is_automatic are fixed: code refers to the key, and whether a template is
/// sent without a human is a property of the code path using it.
pub async fn update(
    db: impl PgExecutor<'_>,
    id: i64,
    name: &str,
    subject: &str,
    body: &str,
    user_id: i64,
) -> sqlx::Result<Option<EmailTemplate>> {
    sqlx::query_as!(
        EmailTemplate,
        "UPDATE email_templates SET name = $2, subject = $3, body = $4, updated_by = $5 WHERE id = $1
         RETURNING id, key, name, subject, body, locale, is_automatic, updated_at, updated_by",
        id,
        name,
        subject,
        body,
        user_id
    )
    .fetch_optional(db)
    .await
}

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

#[derive(Debug, Clone, Serialize)]
pub struct Contact {
    pub id: i64,
    pub partner_id: i64,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub position: Option<String>,
    pub notes: Option<String>,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ContactInput {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub position: Option<String>,
    pub notes: Option<String>,
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Contact>> {
    sqlx::query_as!(
        Contact,
        "SELECT id, partner_id, name, email, phone, position, notes, archived_at, created_at, updated_at
         FROM contacts WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn list_for_partner(
    db: impl PgExecutor<'_>,
    partner_id: i64,
    include_archived: bool,
) -> sqlx::Result<Vec<Contact>> {
    sqlx::query_as!(
        Contact,
        "SELECT id, partner_id, name, email, phone, position, notes, archived_at, created_at, updated_at
         FROM contacts WHERE partner_id = $1 AND ($2 OR archived_at IS NULL)
         ORDER BY archived_at NULLS FIRST, name",
        partner_id,
        include_archived
    )
    .fetch_all(db)
    .await
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    partner_id: i64,
    c: &ContactInput,
) -> sqlx::Result<Contact> {
    sqlx::query_as!(
        Contact,
        "INSERT INTO contacts (partner_id, name, email, phone, position, notes)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, partner_id, name, email, phone, position, notes, archived_at, created_at, updated_at",
        partner_id,
        c.name,
        c.email,
        c.phone,
        c.position,
        c.notes
    )
    .fetch_one(db)
    .await
}

pub async fn update(
    db: impl PgExecutor<'_>,
    id: i64,
    c: &ContactInput,
) -> sqlx::Result<Option<Contact>> {
    sqlx::query_as!(
        Contact,
        "UPDATE contacts SET name = $2, email = $3, phone = $4, position = $5, notes = $6
         WHERE id = $1
         RETURNING id, partner_id, name, email, phone, position, notes, archived_at, created_at, updated_at",
        id,
        c.name,
        c.email,
        c.phone,
        c.position,
        c.notes
    )
    .fetch_optional(db)
    .await
}

pub async fn set_archived(db: impl PgExecutor<'_>, id: i64, archived: bool) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "UPDATE contacts SET archived_at = CASE WHEN $2 THEN coalesce(archived_at, now()) ELSE NULL END WHERE id = $1",
        id,
        archived
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

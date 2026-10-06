//! Job listings and the applications their public form collects (HR recruitment).
//! Runtime-checked queries, like the rest of the HR module.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PostingRow {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub slug: String,
    pub status: String,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub application_count: i64,
}

#[derive(Debug, Clone)]
pub struct PostingInput {
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ApplicationRow {
    pub id: i64,
    pub posting_id: i64,
    pub full_name: String,
    pub email: String,
    pub phone: String,
    pub age: i32,
    pub city: Option<String>,
    pub message: Option<String>,
    pub resume_key: String,
    pub resume_filename: String,
    pub resume_content_type: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ApplicationInput {
    pub full_name: String,
    pub email: String,
    pub phone: String,
    pub age: i32,
    pub city: Option<String>,
    pub message: Option<String>,
    pub resume_key: String,
    pub resume_filename: String,
    pub resume_content_type: String,
}

const POSTING_SELECT: &str = "SELECT p.id, p.title, p.description, p.location, p.slug, p.status,
        p.published_at, p.created_at, p.updated_at,
        (SELECT count(*) FROM job_applications a WHERE a.posting_id = p.id) AS application_count
     FROM job_postings p";

const APPLICATION_COLUMNS: &str = "id, posting_id, full_name, email, phone, age, city, message,
        resume_key, resume_filename, resume_content_type, notes, created_at";

pub async fn list_postings(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<PostingRow>> {
    sqlx::query_as(&format!(
        "{POSTING_SELECT}
         ORDER BY (p.status = 'closed'), p.created_at DESC, p.id DESC"
    ))
    .fetch_all(db)
    .await
}

pub async fn find_posting(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<PostingRow>> {
    sqlx::query_as(&format!("{POSTING_SELECT} WHERE p.id = $1"))
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn find_posting_by_slug(
    db: impl PgExecutor<'_>,
    slug: &str,
) -> sqlx::Result<Option<PostingRow>> {
    sqlx::query_as(&format!("{POSTING_SELECT} WHERE p.slug = $1"))
        .bind(slug)
        .fetch_optional(db)
        .await
}

pub async fn insert_posting(
    db: impl PgExecutor<'_>,
    p: &PostingInput,
    slug: &str,
    created_by: i64,
) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO job_postings (title, description, location, slug, created_by)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(&p.title)
    .bind(&p.description)
    .bind(&p.location)
    .bind(slug)
    .bind(created_by)
    .fetch_one(db)
    .await
}

pub async fn update_posting(
    db: impl PgExecutor<'_>,
    id: i64,
    p: &PostingInput,
) -> sqlx::Result<bool> {
    let r = sqlx::query(
        "UPDATE job_postings SET title = $2, description = $3, location = $4 WHERE id = $1",
    )
    .bind(id)
    .bind(&p.title)
    .bind(&p.description)
    .bind(&p.location)
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// Moves the listing to `status`. `published_at` records the first time it went live.
pub async fn set_posting_status(
    db: impl PgExecutor<'_>,
    id: i64,
    status: &str,
) -> sqlx::Result<bool> {
    let r = sqlx::query(
        "UPDATE job_postings
         SET status = $2,
             published_at = CASE WHEN $2 = 'published' THEN coalesce(published_at, now())
                                 ELSE published_at END
         WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// Row-locks the listing for the rest of the transaction.
pub async fn lock_posting(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT status FROM job_postings WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn delete_posting(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let r = sqlx::query("DELETE FROM job_postings WHERE id = $1")
        .bind(id)
        .execute(db)
        .await?;
    Ok(r.rows_affected() == 1)
}

pub async fn list_applications(
    db: impl PgExecutor<'_>,
    posting_id: i64,
) -> sqlx::Result<Vec<ApplicationRow>> {
    sqlx::query_as(&format!(
        "SELECT {APPLICATION_COLUMNS} FROM job_applications
         WHERE posting_id = $1 ORDER BY created_at DESC, id DESC"
    ))
    .bind(posting_id)
    .fetch_all(db)
    .await
}

/// Every resume key of a listing, for cleaning the object store when the listing goes.
pub async fn resume_keys(db: impl PgExecutor<'_>, posting_id: i64) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar("SELECT resume_key FROM job_applications WHERE posting_id = $1")
        .bind(posting_id)
        .fetch_all(db)
        .await
}

pub async fn find_application(
    db: impl PgExecutor<'_>,
    id: i64,
) -> sqlx::Result<Option<ApplicationRow>> {
    sqlx::query_as(&format!(
        "SELECT {APPLICATION_COLUMNS} FROM job_applications WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(db)
    .await
}

pub async fn insert_application(
    db: impl PgExecutor<'_>,
    posting_id: i64,
    a: &ApplicationInput,
) -> sqlx::Result<ApplicationRow> {
    sqlx::query_as(&format!(
        "INSERT INTO job_applications
             (posting_id, full_name, email, phone, age, city, message,
              resume_key, resume_filename, resume_content_type)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING {APPLICATION_COLUMNS}"
    ))
    .bind(posting_id)
    .bind(&a.full_name)
    .bind(&a.email)
    .bind(&a.phone)
    .bind(a.age)
    .bind(&a.city)
    .bind(&a.message)
    .bind(&a.resume_key)
    .bind(&a.resume_filename)
    .bind(&a.resume_content_type)
    .fetch_one(db)
    .await
}

pub async fn set_notes(
    db: impl PgExecutor<'_>,
    id: i64,
    notes: Option<&str>,
) -> sqlx::Result<Option<ApplicationRow>> {
    sqlx::query_as(&format!(
        "UPDATE job_applications SET notes = $2 WHERE id = $1 RETURNING {APPLICATION_COLUMNS}"
    ))
    .bind(id)
    .bind(notes)
    .fetch_optional(db)
    .await
}

/// Deletes the profile; returns its resume key so the caller can drop the file.
pub async fn delete_application(
    db: impl PgExecutor<'_>,
    id: i64,
) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("DELETE FROM job_applications WHERE id = $1 RETURNING resume_key")
        .bind(id)
        .fetch_optional(db)
        .await
}

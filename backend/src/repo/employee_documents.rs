//! Employee documents with an expiry: medical fitness, contracts, licences, training.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgExecutor;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct EmployeeDocument {
    pub id: i64,
    pub employee_id: i64,
    pub kind: String,
    pub title: String,
    pub valid_until: Option<NaiveDate>,
    pub file_key: Option<String>,
    pub file_name: Option<String>,
    pub file_type: Option<String>,
    pub file_size: Option<i64>,
    pub notes: Option<String>,
    pub created_by: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewDocument {
    pub kind: String,
    pub title: String,
    pub valid_until: Option<NaiveDate>,
    pub file_key: Option<String>,
    pub file_name: Option<String>,
    pub file_type: Option<String>,
    pub file_size: Option<i64>,
    pub notes: Option<String>,
}

const COLS: &str = "id, employee_id, kind, title, valid_until, file_key, file_name, file_type, file_size, notes, created_by, created_at";

pub async fn list(
    db: impl PgExecutor<'_>,
    employee_id: i64,
) -> sqlx::Result<Vec<EmployeeDocument>> {
    sqlx::query_as(&format!(
        "SELECT {COLS} FROM employee_documents
          WHERE employee_id = $1 AND deleted_at IS NULL
          ORDER BY valid_until NULLS LAST, created_at DESC"
    ))
    .bind(employee_id)
    .fetch_all(db)
    .await
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    employee_id: i64,
    d: &NewDocument,
    created_by: i64,
) -> sqlx::Result<EmployeeDocument> {
    sqlx::query_as(&format!(
        "INSERT INTO employee_documents (employee_id, kind, title, valid_until, file_key, file_name, file_type, file_size, notes, created_by)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING {COLS}"
    ))
    .bind(employee_id)
    .bind(&d.kind)
    .bind(&d.title)
    .bind(d.valid_until)
    .bind(&d.file_key)
    .bind(&d.file_name)
    .bind(&d.file_type)
    .bind(d.file_size)
    .bind(&d.notes)
    .bind(created_by)
    .fetch_one(db)
    .await
}

pub async fn soft_delete(db: impl PgExecutor<'_>, id: i64, employee_id: i64) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "UPDATE employee_documents SET deleted_at = now()
          WHERE id = $1 AND employee_id = $2 AND deleted_at IS NULL RETURNING id",
    )
    .bind(id)
    .bind(employee_id)
    .fetch_optional(db)
    .await
    .map(|r: Option<i64>| r.is_some())
}

/// Case-insensitive exact match against the allowed kinds, for request validation.
pub fn kind_ok(kind: &str) -> bool {
    matches!(
        kind,
        "medical" | "contract" | "licence" | "training" | "other"
    )
}

/// Stores the uploaded file's location on the document. False when there is no such live
/// document of this employee.
pub async fn set_file(
    db: impl PgExecutor<'_>,
    id: i64,
    employee_id: i64,
    key: &str,
    name: &str,
    content_type: &str,
    size: i64,
) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE employee_documents SET file_key = $3, file_name = $4, file_type = $5, file_size = $6
          WHERE id = $1 AND employee_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(employee_id)
    .bind(key)
    .bind(name)
    .bind(content_type)
    .bind(size)
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// The stored file's key and name, when the document has one.
pub async fn file_of(
    db: impl PgExecutor<'_>,
    id: i64,
    employee_id: i64,
) -> sqlx::Result<Option<(String, String)>> {
    sqlx::query_as(
        "SELECT file_key, coalesce(file_name, 'dokumentum') FROM employee_documents
          WHERE id = $1 AND employee_id = $2 AND deleted_at IS NULL AND file_key IS NOT NULL",
    )
    .bind(id)
    .bind(employee_id)
    .fetch_optional(db)
    .await
}

/// Documents running out within `days` (or already expired), soonest first: for the HR
/// page's warning list.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ExpiringDocument {
    pub id: i64,
    pub employee_id: i64,
    pub employee_name: String,
    pub kind: String,
    pub title: String,
    pub valid_until: NaiveDate,
}

pub async fn expiring(
    db: impl PgExecutor<'_>,
    until: NaiveDate,
) -> sqlx::Result<Vec<ExpiringDocument>> {
    sqlx::query_as(
        "SELECT d.id, d.employee_id, e.full_name AS employee_name, d.kind, d.title, d.valid_until
           FROM employee_documents d
           JOIN employees e ON e.id = d.employee_id AND e.archived_at IS NULL
          WHERE d.deleted_at IS NULL AND d.valid_until IS NOT NULL AND d.valid_until <= $1
          ORDER BY d.valid_until, d.id",
    )
    .bind(until)
    .fetch_all(db)
    .await
}

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

pub async fn list(db: impl PgExecutor<'_>, employee_id: i64) -> sqlx::Result<Vec<EmployeeDocument>> {
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
    matches!(kind, "medical" | "contract" | "licence" | "training" | "other")
}

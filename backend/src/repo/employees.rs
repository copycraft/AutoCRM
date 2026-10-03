//! The staff directory (HR module). Runtime-checked queries: the table is small and the
//! module self-contained.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EmployeeRow {
    pub id: i64,
    pub full_name: String,
    pub email: Option<String>,
    pub company_phone: Option<String>,
    pub personal_phone: Option<String>,
    pub photo_key: Option<String>,
    pub annual_leave_days: i32,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct EmployeeInput {
    pub full_name: String,
    pub email: Option<String>,
    pub company_phone: Option<String>,
    pub personal_phone: Option<String>,
    pub annual_leave_days: i32,
}

const COLUMNS: &str = "id, full_name, email, company_phone, personal_phone, photo_key,
                       annual_leave_days, archived_at, created_at, updated_at";

pub async fn list(
    db: impl PgExecutor<'_>,
    pattern: Option<&str>,
    include_archived: bool,
) -> sqlx::Result<Vec<EmployeeRow>> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM employees
         WHERE ($1::text IS NULL OR full_name ILIKE $1 OR email ILIKE $1
                OR company_phone ILIKE $1 OR personal_phone ILIKE $1)
           AND ($2 OR archived_at IS NULL)
         ORDER BY archived_at IS NOT NULL, lower(full_name), id"
    ))
    .bind(pattern)
    .bind(include_archived)
    .fetch_all(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<EmployeeRow>> {
    sqlx::query_as(&format!("SELECT {COLUMNS} FROM employees WHERE id = $1"))
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    e: &EmployeeInput,
    created_by: i64,
) -> sqlx::Result<EmployeeRow> {
    sqlx::query_as(&format!(
        "INSERT INTO employees (full_name, email, company_phone, personal_phone, annual_leave_days, created_by)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING {COLUMNS}"
    ))
    .bind(&e.full_name)
    .bind(&e.email)
    .bind(&e.company_phone)
    .bind(&e.personal_phone)
    .bind(e.annual_leave_days)
    .bind(created_by)
    .fetch_one(db)
    .await
}

pub async fn update(
    db: impl PgExecutor<'_>,
    id: i64,
    e: &EmployeeInput,
) -> sqlx::Result<Option<EmployeeRow>> {
    sqlx::query_as(&format!(
        "UPDATE employees SET full_name = $2, email = $3, company_phone = $4, personal_phone = $5,
                              annual_leave_days = $6
         WHERE id = $1 RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(&e.full_name)
    .bind(&e.email)
    .bind(&e.company_phone)
    .bind(&e.personal_phone)
    .bind(e.annual_leave_days)
    .fetch_optional(db)
    .await
}

pub async fn set_archived(
    db: impl PgExecutor<'_>,
    id: i64,
    archived: bool,
) -> sqlx::Result<Option<EmployeeRow>> {
    sqlx::query_as(&format!(
        "UPDATE employees SET archived_at = CASE WHEN $2 THEN coalesce(archived_at, now()) END
         WHERE id = $1 RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(archived)
    .fetch_optional(db)
    .await
}

/// Row-locks the employee for the rest of the transaction.
pub async fn lock(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<EmployeeRow>> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM employees WHERE id = $1 FOR UPDATE"
    ))
    .bind(id)
    .fetch_optional(db)
    .await
}

pub async fn set_photo(
    db: impl PgExecutor<'_>,
    id: i64,
    key: Option<&str>,
) -> sqlx::Result<Option<EmployeeRow>> {
    sqlx::query_as(&format!(
        "UPDATE employees SET photo_key = $2 WHERE id = $1 RETURNING {COLUMNS}"
    ))
    .bind(id)
    .bind(key)
    .fetch_optional(db)
    .await
}

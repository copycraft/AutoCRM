//! Leave and absence records (HR module). Runtime-checked queries, like `employees`.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgExecutor;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AbsenceRow {
    pub id: i64,
    pub employee_id: i64,
    pub employee_name: String,
    pub kind: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

const SELECT: &str = "SELECT a.id, a.employee_id, e.full_name AS employee_name, a.kind,
                             a.start_date, a.end_date, a.note, a.created_at
                      FROM absences a JOIN employees e ON e.id = a.employee_id";

/// Absences that touch `from..=to`, optionally for one employee, soonest first.
pub async fn list(
    db: impl PgExecutor<'_>,
    from: NaiveDate,
    to: NaiveDate,
    employee_id: Option<i64>,
) -> sqlx::Result<Vec<AbsenceRow>> {
    sqlx::query_as(&format!(
        "{SELECT} WHERE a.end_date >= $1 AND a.start_date <= $2
                    AND ($3::bigint IS NULL OR a.employee_id = $3)
         ORDER BY a.start_date, e.full_name, a.id"
    ))
    .bind(from)
    .bind(to)
    .bind(employee_id)
    .fetch_all(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<AbsenceRow>> {
    sqlx::query_as(&format!("{SELECT} WHERE a.id = $1"))
        .bind(id)
        .fetch_optional(db)
        .await
}

/// Whether the employee already has leave touching `start..=end`.
pub async fn overlaps(
    db: impl PgExecutor<'_>,
    employee_id: i64,
    start: NaiveDate,
    end: NaiveDate,
) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM absences
                        WHERE employee_id = $1 AND end_date >= $2 AND start_date <= $3)",
    )
    .bind(employee_id)
    .bind(start)
    .bind(end)
    .fetch_one(db)
    .await
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    employee_id: i64,
    kind: &str,
    start: NaiveDate,
    end: NaiveDate,
    note: Option<&str>,
    created_by: i64,
) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO absences (employee_id, kind, start_date, end_date, note, created_by)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(employee_id)
    .bind(kind)
    .bind(start)
    .bind(end)
    .bind(note)
    .bind(created_by)
    .fetch_one(db)
    .await
}

pub async fn delete(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM absences WHERE id = $1")
        .bind(id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() == 1)
}

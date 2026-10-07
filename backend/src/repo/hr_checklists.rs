//! Joining and leaving checklists (0049): the steps HR goes through for every new starter
//! and every leaver. Starting one turns each step into a task on the employee, due a set
//! number of days after the start.

use chrono::{NaiveDate, TimeDelta};
use serde::Serialize;
use sqlx::{PgConnection, PgExecutor};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ChecklistItem {
    pub id: i64,
    /// onboarding or offboarding.
    pub kind: String,
    pub title: String,
    /// Due this many days after the checklist is started.
    pub due_days: i32,
    pub position: i32,
}

pub const KINDS: &[&str] = &["onboarding", "offboarding"];

pub async fn list(db: impl PgExecutor<'_>, kind: Option<&str>) -> sqlx::Result<Vec<ChecklistItem>> {
    sqlx::query_as(
        "SELECT id, kind, title, due_days, position FROM hr_checklist_items
          WHERE archived_at IS NULL AND ($1::text IS NULL OR kind = $1)
          ORDER BY kind, position, id",
    )
    .bind(kind)
    .fetch_all(db)
    .await
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    kind: &str,
    title: &str,
    due_days: i32,
) -> sqlx::Result<ChecklistItem> {
    sqlx::query_as(
        "INSERT INTO hr_checklist_items (kind, title, due_days, position)
         VALUES ($1, $2, $3, coalesce((SELECT max(position) + 10 FROM hr_checklist_items WHERE kind = $1), 10))
         RETURNING id, kind, title, due_days, position",
    )
    .bind(kind)
    .bind(title)
    .bind(due_days)
    .fetch_one(db)
    .await
}

pub async fn update(
    db: impl PgExecutor<'_>,
    id: i64,
    title: &str,
    due_days: i32,
    position: i32,
) -> sqlx::Result<Option<ChecklistItem>> {
    sqlx::query_as(
        "UPDATE hr_checklist_items SET title = $2, due_days = $3, position = $4
          WHERE id = $1 AND archived_at IS NULL
          RETURNING id, kind, title, due_days, position",
    )
    .bind(id)
    .bind(title)
    .bind(due_days)
    .bind(position)
    .fetch_optional(db)
    .await
}

pub async fn archive(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE hr_checklist_items SET archived_at = now() WHERE id = $1 AND archived_at IS NULL",
    )
    .bind(id)
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Creates one task per step on the employee. Returns how many.
pub async fn start(
    conn: &mut PgConnection,
    employee_id: i64,
    kind: &str,
    start: NaiveDate,
    assigned_to: Option<i64>,
    created_by: i64,
) -> sqlx::Result<usize> {
    let items = list(&mut *conn, Some(kind)).await?;
    for item in &items {
        sqlx::query(
            "INSERT INTO tasks (entity_type, entity_id, title, due_date, assigned_to, created_by)
             VALUES ('employee', $1, $2, $3, $4, $5)",
        )
        .bind(employee_id)
        .bind(&item.title)
        .bind(start + TimeDelta::days(i64::from(item.due_days)))
        .bind(assigned_to)
        .bind(created_by)
        .execute(&mut *conn)
        .await?;
    }
    Ok(items.len())
}

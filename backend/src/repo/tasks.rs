//! Follow-up tasks (Feladatok): reminders pinned to an order, lead or partner.
//!
//! Deliberately not a workflow engine — no states beyond open/done, no dependencies.
//! The dashboard widget reads a user's open tasks; record pages read their own.
//! Any authenticated user may create and toggle: a reminder is personal productivity,
//! not a business mutation, and `created_by` records who said so.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Task {
    pub id: i64,
    pub entity_type: String,
    pub entity_id: i64,
    pub title: String,
    pub due_date: Option<NaiveDate>,
    pub done_at: Option<DateTime<Utc>>,
    pub assigned_to: Option<i64>,
    pub assigned_name: Option<String>,
    pub created_by: i64,
    pub created_at: DateTime<Utc>,
}

pub struct NewTask {
    pub entity_type: String,
    pub entity_id: i64,
    pub title: String,
    pub due_date: Option<NaiveDate>,
    pub assigned_to: Option<i64>,
    pub created_by: i64,
}

pub async fn create(db: impl PgExecutor<'_>, n: &NewTask) -> sqlx::Result<Task> {
    sqlx::query_as!(
        Task,
        r#"INSERT INTO tasks (entity_type, entity_id, title, due_date, assigned_to, created_by)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id, entity_type, entity_id, title, due_date, done_at,
                     assigned_to, NULL AS "assigned_name?", created_by, created_at"#,
        n.entity_type,
        n.entity_id,
        n.title,
        n.due_date,
        n.assigned_to,
        n.created_by
    )
    .fetch_one(db)
    .await
}

/// Everything pinned to one record, open first, then most recently done.
pub async fn for_entity(
    db: impl PgExecutor<'_>,
    entity_type: &str,
    entity_id: i64,
) -> sqlx::Result<Vec<Task>> {
    sqlx::query_as!(
        Task,
        r#"SELECT t.id, t.entity_type, t.entity_id, t.title, t.due_date, t.done_at,
                  t.assigned_to, u.display_name AS "assigned_name?", t.created_by, t.created_at
           FROM tasks t LEFT JOIN users u ON u.id = t.assigned_to
           WHERE t.entity_type = $1 AND t.entity_id = $2
           ORDER BY t.done_at NULLS FIRST, t.due_date NULLS LAST, t.id DESC"#,
        entity_type,
        entity_id
    )
    .fetch_all(db)
    .await
}

/// A user's open tasks: assigned to them or created by them, most urgent first.
pub async fn open_for_user(db: impl PgExecutor<'_>, user_id: i64) -> sqlx::Result<Vec<Task>> {
    sqlx::query_as!(
        Task,
        r#"SELECT t.id, t.entity_type, t.entity_id, t.title, t.due_date, t.done_at,
                  t.assigned_to, u.display_name AS "assigned_name?", t.created_by, t.created_at
           FROM tasks t LEFT JOIN users u ON u.id = t.assigned_to
           WHERE t.done_at IS NULL AND (t.assigned_to = $1 OR t.created_by = $1)
           ORDER BY t.due_date NULLS LAST, t.id DESC"#,
        user_id
    )
    .fetch_all(db)
    .await
}

pub async fn set_done(
    db: impl PgExecutor<'_>,
    id: i64,
    done: bool,
) -> sqlx::Result<Option<Task>> {
    sqlx::query_as!(
        Task,
        r#"UPDATE tasks SET done_at = CASE WHEN $2 THEN now() ELSE NULL END WHERE id = $1
           RETURNING id, entity_type, entity_id, title, due_date, done_at,
                     assigned_to, NULL AS "assigned_name?", created_by, created_at"#,
        id,
        done
    )
    .fetch_optional(db)
    .await
}

pub async fn remove(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let r = sqlx::query!("DELETE FROM tasks WHERE id = $1", id)
        .execute(db)
        .await?;
    Ok(r.rows_affected() > 0)
}

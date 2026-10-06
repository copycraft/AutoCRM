//! The HR status lists (MiniCRM's employee statuses): configuration, edited by HR.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct EmployeeStatus {
    pub id: i64,
    /// The heading it is listed under: Távollévő munkavállaló, Aktív munkavállaló...
    pub section: String,
    pub label: String,
    /// `#rrggbb`.
    pub color: String,
    pub position: i32,
    /// Choosing it archives the employee; archiving puts them on it.
    pub ends_employment: bool,
    /// Where new and returning employees start.
    pub is_default: bool,
    pub archived_at: Option<DateTime<Utc>>,
    /// Employees on it now.
    pub employees: i64,
}

pub struct StatusInput {
    pub section: String,
    pub label: String,
    pub color: String,
}

const SELECT: &str = "SELECT s.id, s.section, s.label, s.color, s.position, s.ends_employment,
        s.is_default, s.archived_at, count(e.id) AS employees
    FROM employee_statuses s LEFT JOIN employees e ON e.status_id = s.id";

pub async fn list(db: impl PgExecutor<'_>, archived: bool) -> sqlx::Result<Vec<EmployeeStatus>> {
    sqlx::query_as(&format!(
        "{SELECT} WHERE (s.archived_at IS NULL) <> $1 GROUP BY s.id ORDER BY s.position, s.id"
    ))
    .bind(archived)
    .fetch_all(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<EmployeeStatus>> {
    sqlx::query_as(&format!("{SELECT} WHERE s.id = $1 GROUP BY s.id"))
        .bind(id)
        .fetch_optional(db)
        .await
}

/// At the end of its section, or of everything for a new section.
pub async fn insert(db: impl PgExecutor<'_>, s: &StatusInput) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO employee_statuses (section, label, color, position)
         VALUES ($1, $2, $3, coalesce(
             (SELECT max(position) + 1 FROM employee_statuses WHERE lower(section) = lower($1)),
             (SELECT coalesce(max(position), 0) + 1000 FROM employee_statuses)))
         RETURNING id",
    )
    .bind(&s.section)
    .bind(&s.label)
    .bind(&s.color)
    .fetch_one(db)
    .await
}

pub async fn update(
    db: impl PgExecutor<'_>,
    id: i64,
    s: &StatusInput,
    archived: bool,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE employee_statuses SET
             position = CASE WHEN lower(section) = lower($2) THEN position ELSE coalesce(
                 (SELECT max(position) + 1 FROM employee_statuses WHERE lower(section) = lower($2)),
                 (SELECT coalesce(max(position), 0) + 1000 FROM employee_statuses)) END,
             section = $2, label = $3, color = $4,
             archived_at = CASE WHEN $5 THEN coalesce(archived_at, now()) END
         WHERE id = $1",
    )
    .bind(id)
    .bind(&s.section)
    .bind(&s.label)
    .bind(&s.color)
    .bind(archived)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn reorder(db: impl PgExecutor<'_>, section: &str, ids: &[i64]) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE employee_statuses t
         SET position = base.p + o.n::int - 1
         FROM unnest($2::bigint[]) WITH ORDINALITY AS o(id, n),
              (SELECT min(position) AS p FROM employee_statuses WHERE lower(section) = lower($1)) base
         WHERE t.id = o.id AND lower(t.section) = lower($1)",
    )
    .bind(section)
    .bind(ids)
    .execute(db)
    .await?;
    Ok(())
}

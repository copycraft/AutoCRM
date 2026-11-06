//! Departments and who works in them (0059). Runtime-checked queries, like `employees`.

use serde::Serialize;
use sqlx::PgExecutor;

#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct Department {
    pub id: i64,
    pub key: String,
    pub name: String,
    /// `#rrggbb`: the colour of the department's module everywhere it shows.
    pub color: String,
    pub position: i32,
    pub is_active: bool,
}

/// One person in one department.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct Membership {
    pub user_id: i64,
    pub user_name: String,
    pub department_id: i64,
    /// Heads are told when work is released to the department.
    pub is_head: bool,
}

/// A department head to tell, with where to write to them.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Head {
    pub user_id: i64,
    pub email: String,
}

const COLUMNS: &str = "id, key, name, color, position, is_active";

pub async fn list(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<Department>> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM departments ORDER BY position, id"
    ))
    .fetch_all(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Department>> {
    sqlx::query_as(&format!("SELECT {COLUMNS} FROM departments WHERE id = $1"))
        .bind(id)
        .fetch_optional(db)
        .await
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    key: &str,
    name: &str,
    color: &str,
    position: i32,
) -> sqlx::Result<Department> {
    sqlx::query_as(&format!(
        "INSERT INTO departments (key, name, color, position) VALUES ($1, $2, $3, $4)
         RETURNING {COLUMNS}"
    ))
    .bind(key)
    .bind(name)
    .bind(color)
    .bind(position)
    .fetch_one(db)
    .await
}

pub async fn update(db: impl PgExecutor<'_>, d: &Department) -> sqlx::Result<Option<Department>> {
    sqlx::query_as(&format!(
        "UPDATE departments SET name = $2, color = $3, position = $4, is_active = $5
          WHERE id = $1 RETURNING {COLUMNS}"
    ))
    .bind(d.id)
    .bind(&d.name)
    .bind(&d.color)
    .bind(d.position)
    .bind(d.is_active)
    .fetch_optional(db)
    .await
}

/// Everyone in every department, by department then name.
pub async fn memberships(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<Membership>> {
    sqlx::query_as(
        "SELECT ud.user_id, u.display_name AS user_name, ud.department_id, ud.is_head
           FROM user_departments ud JOIN users u ON u.id = ud.user_id
          ORDER BY ud.department_id, u.display_name",
    )
    .fetch_all(db)
    .await
}

/// Replaces the departments one person works in: `(department_id, is_head)` pairs.
pub async fn set_for_user(
    conn: &mut sqlx::PgConnection,
    user_id: i64,
    departments: &[(i64, bool)],
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM user_departments WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *conn)
        .await?;
    for (department_id, is_head) in departments {
        sqlx::query(
            "INSERT INTO user_departments (user_id, department_id, is_head) VALUES ($1, $2, $3)",
        )
        .bind(user_id)
        .bind(department_id)
        .bind(is_head)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// The departments a person works in.
pub async fn of_user(db: impl PgExecutor<'_>, user_id: i64) -> sqlx::Result<Vec<i64>> {
    sqlx::query_scalar("SELECT department_id FROM user_departments WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(db)
        .await
}

/// The active heads of a department.
pub async fn heads(db: impl PgExecutor<'_>, department_id: i64) -> sqlx::Result<Vec<Head>> {
    sqlx::query_as(
        "SELECT u.id AS user_id, u.email
           FROM user_departments ud JOIN users u ON u.id = ud.user_id
          WHERE ud.department_id = $1 AND ud.is_head AND u.is_active
          ORDER BY u.id",
    )
    .bind(department_id)
    .fetch_all(db)
    .await
}

//! An order's technical content, one module per department (0059). Runtime-checked
//! queries, like `employees`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;

#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct OrderModule {
    pub id: i64,
    pub order_id: i64,
    pub department_id: i64,
    pub department_key: String,
    pub department_name: String,
    /// `#rrggbb`, the department's colour.
    pub department_color: String,
    pub content: String,
    /// `draft`, `released` (handed to the department, heads told) or `done`.
    pub status: String,
    pub content_updated_at: DateTime<Utc>,
    pub released_at: Option<DateTime<Utc>>,
    pub released_by_name: Option<String>,
    pub updated_by_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl OrderModule {
    /// Released, then written to again: the department is working from an older text.
    pub fn changed_since_release(&self) -> bool {
        self.released_at
            .is_some_and(|released| self.content_updated_at > released)
    }
}

/// A drawing or photo on a module's sheet: one of the order's own files, linked.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct ModuleFile {
    pub id: i64,
    pub module_id: i64,
    pub image_id: Option<i64>,
    pub document_id: Option<i64>,
    /// The file's name, for the list; null for a photo uploaded without one.
    pub filename: Option<String>,
}

const SELECT: &str = "SELECT m.id, m.order_id, m.department_id, d.key AS department_key,
                             d.name AS department_name, d.color AS department_color, m.content,
                             m.status, m.content_updated_at, m.released_at,
                             rb.display_name AS released_by_name,
                             ub.display_name AS updated_by_name, m.created_at, m.updated_at
                        FROM order_modules m
                        JOIN departments d ON d.id = m.department_id
                        LEFT JOIN users rb ON rb.id = m.released_by
                        LEFT JOIN users ub ON ub.id = m.updated_by";

/// The order's modules in the departments' order.
pub async fn list_for_order(
    db: impl PgExecutor<'_>,
    order_id: i64,
) -> sqlx::Result<Vec<OrderModule>> {
    sqlx::query_as(&format!(
        "{SELECT} WHERE m.order_id = $1 ORDER BY d.position, d.id"
    ))
    .bind(order_id)
    .fetch_all(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<OrderModule>> {
    sqlx::query_as(&format!("{SELECT} WHERE m.id = $1"))
        .bind(id)
        .fetch_optional(db)
        .await
}

/// Starts the department's module on the order, or returns the one it already has.
pub async fn insert(
    db: impl PgExecutor<'_>,
    order_id: i64,
    department_id: i64,
    content: &str,
    user_id: i64,
) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO order_modules (order_id, department_id, content, updated_by)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (order_id, department_id) DO UPDATE SET order_id = EXCLUDED.order_id
         RETURNING id",
    )
    .bind(order_id)
    .bind(department_id)
    .bind(content)
    .bind(user_id)
    .fetch_one(db)
    .await
}

pub async fn set_content(
    db: impl PgExecutor<'_>,
    id: i64,
    content: &str,
    user_id: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE order_modules SET content = $2, content_updated_at = now(), updated_by = $3
          WHERE id = $1 AND content IS DISTINCT FROM $2",
    )
    .bind(id)
    .bind(content)
    .bind(user_id)
    .execute(db)
    .await?;
    Ok(())
}

/// `draft` or `done`; releasing goes through [`mark_released`].
pub async fn set_status(
    db: impl PgExecutor<'_>,
    id: i64,
    status: &str,
    user_id: i64,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE order_modules SET status = $2, updated_by = $3 WHERE id = $1")
        .bind(id)
        .bind(status)
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn mark_released(db: impl PgExecutor<'_>, id: i64, user_id: i64) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE order_modules
            SET status = 'released', released_at = now(), released_by = $2, updated_by = $2
          WHERE id = $1",
    )
    .bind(id)
    .bind(user_id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn delete(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM order_modules WHERE id = $1")
        .bind(id)
        .execute(db)
        .await?;
    Ok(done.rows_affected() > 0)
}

/// Every file linked to any module of the order.
pub async fn files_for_order(
    db: impl PgExecutor<'_>,
    order_id: i64,
) -> sqlx::Result<Vec<ModuleFile>> {
    sqlx::query_as(
        "SELECT f.id, f.module_id, f.image_id, f.document_id,
                COALESCE(doc.filename, img.original_filename) AS filename
           FROM order_module_files f
           JOIN order_modules m ON m.id = f.module_id
           LEFT JOIN documents doc ON doc.id = f.document_id
           LEFT JOIN images img ON img.id = f.image_id
          WHERE m.order_id = $1
            AND doc.deleted_at IS NULL AND img.deleted_at IS NULL
          ORDER BY f.module_id, f.id",
    )
    .bind(order_id)
    .fetch_all(db)
    .await
}

/// Links one of the order's files to the module; linking it twice is a no-op.
pub async fn add_file(
    db: impl PgExecutor<'_>,
    module_id: i64,
    image_id: Option<i64>,
    document_id: Option<i64>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO order_module_files (module_id, image_id, document_id) VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(module_id)
    .bind(image_id)
    .bind(document_id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn remove_file(
    db: impl PgExecutor<'_>,
    module_id: i64,
    file_id: i64,
) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM order_module_files WHERE id = $1 AND module_id = $2")
        .bind(file_id)
        .bind(module_id)
        .execute(db)
        .await?;
    Ok(done.rows_affected() > 0)
}

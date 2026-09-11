use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Serialize, Serializer};
use sqlx::{PgConnection, PgExecutor};

use crate::domain::media::ImageCategory;

pub fn hex_bytes<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&hex::encode(bytes))
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Image {
    pub id: i64,
    pub order_id: i64,
    pub category: ImageCategory,
    #[serde(skip)]
    pub storage_key: String,
    #[serde(skip)]
    pub display_key: Option<String>,
    #[serde(skip)]
    pub thumb_key: Option<String>,
    pub content_type: String,
    pub original_filename: Option<String>,
    /// Hex-encoded sha256 of the original file.
    #[serde(serialize_with = "hex_bytes")]
    #[schema(value_type = String)]
    pub content_hash: Vec<u8>,
    pub byte_size: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub captured_at: Option<DateTime<Utc>>,
    pub processed_at: Option<DateTime<Utc>>,
    pub processing_error: Option<String>,
    pub uploaded_at: DateTime<Utc>,
    pub uploaded_by: Option<i64>,
    pub immutable: bool,
    pub deleted_at: Option<DateTime<Utc>>,
}

pub struct NewImage<'a> {
    pub order_id: i64,
    pub category: ImageCategory,
    pub storage_key: &'a str,
    pub content_type: &'a str,
    pub original_filename: Option<&'a str>,
    pub content_hash: &'a [u8],
    pub byte_size: i64,
    pub uploaded_by: Option<i64>,
    pub source_ref: Option<&'a str>,
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Image>> {
    sqlx::query_as!(
        Image,
        r#"SELECT id, order_id, category AS "category: ImageCategory", storage_key, display_key, thumb_key, content_type,
                  original_filename, content_hash, byte_size, width, height, captured_at, processed_at, processing_error,
                  uploaded_at, uploaded_by, immutable, deleted_at
           FROM images WHERE id = $1"#,
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn list_for_order(
    db: impl PgExecutor<'_>,
    order_id: i64,
    category: Option<ImageCategory>,
) -> sqlx::Result<Vec<Image>> {
    sqlx::query_as!(
        Image,
        r#"SELECT id, order_id, category AS "category: ImageCategory", storage_key, display_key, thumb_key, content_type,
                  original_filename, content_hash, byte_size, width, height, captured_at, processed_at, processing_error,
                  uploaded_at, uploaded_by, immutable, deleted_at
           FROM images
           WHERE order_id = $1 AND deleted_at IS NULL AND ($2::image_category IS NULL OR category = $2)
           ORDER BY category, coalesce(captured_at, uploaded_at), id"#,
        order_id,
        category as Option<ImageCategory>
    )
    .fetch_all(db)
    .await
}

pub async fn find_by_hash(
    db: impl PgExecutor<'_>,
    order_id: i64,
    hash: &[u8],
) -> sqlx::Result<Option<Image>> {
    sqlx::query_as!(
        Image,
        r#"SELECT id, order_id, category AS "category: ImageCategory", storage_key, display_key, thumb_key, content_type,
                  original_filename, content_hash, byte_size, width, height, captured_at, processed_at, processing_error,
                  uploaded_at, uploaded_by, immutable, deleted_at
           FROM images WHERE order_id = $1 AND content_hash = $2 AND deleted_at IS NULL"#,
        order_id,
        hash
    )
    .fetch_optional(db)
    .await
}

/// Idempotent: finalizing the same photo twice for one order returns the existing row.
/// The bool is true when a new row was created.
pub async fn insert(conn: &mut PgConnection, n: &NewImage<'_>) -> sqlx::Result<(Image, bool)> {
    let inserted = sqlx::query_as!(
        Image,
        r#"INSERT INTO images (order_id, category, storage_key, content_type, original_filename, content_hash,
                               byte_size, uploaded_by, source_ref, immutable)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
           ON CONFLICT (order_id, content_hash) WHERE deleted_at IS NULL DO NOTHING
           RETURNING id, order_id, category AS "category: ImageCategory", storage_key, display_key, thumb_key, content_type,
                     original_filename, content_hash, byte_size, width, height, captured_at, processed_at, processing_error,
                     uploaded_at, uploaded_by, immutable, deleted_at"#,
        n.order_id,
        n.category as ImageCategory,
        n.storage_key,
        n.content_type,
        n.original_filename,
        n.content_hash,
        n.byte_size,
        n.uploaded_by,
        n.source_ref,
        n.category.is_immutable()
    )
    .fetch_optional(&mut *conn)
    .await?;
    match inserted {
        Some(image) => Ok((image, true)),
        None => find_by_hash(&mut *conn, n.order_id, n.content_hash)
            .await?
            .map(|image| (image, false))
            .ok_or(sqlx::Error::RowNotFound),
    }
}

pub struct Derived<'a> {
    pub display_key: &'a str,
    pub thumb_key: &'a str,
    pub width: i32,
    pub height: i32,
    pub captured_at: Option<DateTime<Utc>>,
}

pub async fn set_derived(db: impl PgExecutor<'_>, id: i64, d: &Derived<'_>) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE images
         SET display_key = $2, thumb_key = $3, width = $4, height = $5, captured_at = coalesce($6, captured_at),
             processed_at = now(), processing_error = NULL
         WHERE id = $1",
        id,
        d.display_key,
        d.thumb_key,
        d.width,
        d.height,
        d.captured_at
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn set_processing_error(
    db: impl PgExecutor<'_>,
    id: i64,
    error: &str,
) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE images SET processing_error = $2, processed_at = now() WHERE id = $1",
        id,
        error
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Soft delete. The database trigger refuses this for immutable (intake) images.
pub async fn soft_delete(db: impl PgExecutor<'_>, id: i64, user_id: i64) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "UPDATE images SET deleted_at = now(), deleted_by = $2 WHERE id = $1 AND deleted_at IS NULL",
        id,
        user_id
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

pub async fn count_by_category(
    db: impl PgExecutor<'_>,
    order_id: i64,
) -> sqlx::Result<HashMap<ImageCategory, i64>> {
    let rows = sqlx::query!(
        r#"SELECT category AS "category: ImageCategory", count(*) AS "n!"
           FROM images WHERE order_id = $1 AND deleted_at IS NULL
           GROUP BY category"#,
        order_id
    )
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|r| (r.category, r.n)).collect())
}

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
    /// A line under the photo, editable even on evidence (it is not part of the bytes).
    pub caption: Option<String>,
    /// Which vehicle on a multi-vehicle order the photo is of.
    pub vehicle_id: Option<i64>,
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
                  uploaded_at, uploaded_by, immutable, deleted_at, caption, vehicle_id
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
                  uploaded_at, uploaded_by, immutable, deleted_at, caption, vehicle_id
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
                  uploaded_at, uploaded_by, immutable, deleted_at, caption, vehicle_id
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
                     uploaded_at, uploaded_by, immutable, deleted_at, caption, vehicle_id"#,
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

/// Caption and vehicle: the two things about a photo that may change after upload.
pub async fn set_caption_vehicle(
    db: impl PgExecutor<'_>,
    id: i64,
    caption: Option<&str>,
    vehicle_id: Option<i64>,
) -> sqlx::Result<Option<Image>> {
    sqlx::query_as!(
        Image,
        r#"UPDATE images SET caption = $2, vehicle_id = $3
           WHERE id = $1 AND deleted_at IS NULL
           RETURNING id, order_id, category AS "category: ImageCategory", storage_key, display_key, thumb_key, content_type,
                     original_filename, content_hash, byte_size, width, height, captured_at, processed_at, processing_error,
                     uploaded_at, uploaded_by, immutable, deleted_at, caption, vehicle_id"#,
        id,
        caption,
        vehicle_id
    )
    .fetch_optional(db)
    .await
}

/// Drawn shapes over a photo's display copy. Never burnt into a file.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Annotations {
    pub image_id: i64,
    /// `[{type, points|x,y,w,h, color, text?}]`, coordinates as fractions of the image.
    #[schema(value_type = Vec<Object>)]
    pub shapes: serde_json::Value,
    pub updated_by: Option<i64>,
    pub updated_by_name: Option<String>,
    pub updated_at: DateTime<Utc>,
}

pub async fn annotations(
    db: impl PgExecutor<'_>,
    image_id: i64,
) -> sqlx::Result<Option<Annotations>> {
    sqlx::query_as!(
        Annotations,
        r#"SELECT a.image_id, a.shapes, a.updated_by, u.display_name AS "updated_by_name?", a.updated_at
           FROM image_annotations a LEFT JOIN users u ON u.id = a.updated_by
           WHERE a.image_id = $1"#,
        image_id
    )
    .fetch_optional(db)
    .await
}

pub async fn save_annotations(
    db: impl PgExecutor<'_>,
    image_id: i64,
    shapes: &serde_json::Value,
    user_id: i64,
) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO image_annotations (image_id, shapes, updated_by) VALUES ($1, $2, $3)
         ON CONFLICT (image_id) DO UPDATE SET shapes = EXCLUDED.shapes, updated_by = EXCLUDED.updated_by, updated_at = now()",
        image_id,
        shapes,
        user_id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Which of an order's photos carry annotations.
pub async fn annotated_ids(db: impl PgExecutor<'_>, order_id: i64) -> sqlx::Result<Vec<i64>> {
    sqlx::query_scalar!(
        r#"SELECT a.image_id AS "id!" FROM image_annotations a JOIN images i ON i.id = a.image_id
           WHERE i.order_id = $1 AND jsonb_array_length(a.shapes) > 0"#,
        order_id
    )
    .fetch_all(db)
    .await
}

/// An RFC 3161 stamp over a photo's sha256.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct TimestampInfo {
    pub image_id: i64,
    pub tsa_url: String,
    /// When the authority certifies the bytes existed.
    pub gen_time: DateTime<Utc>,
    pub serial: String,
}

pub async fn timestamps_for_order(
    db: impl PgExecutor<'_>,
    order_id: i64,
) -> sqlx::Result<Vec<TimestampInfo>> {
    sqlx::query_as!(
        TimestampInfo,
        "SELECT t.image_id, t.tsa_url, t.gen_time, t.serial
         FROM image_timestamps t JOIN images i ON i.id = t.image_id WHERE i.order_id = $1",
        order_id
    )
    .fetch_all(db)
    .await
}

pub async fn timestamp_response(
    db: impl PgExecutor<'_>,
    image_id: i64,
) -> sqlx::Result<Option<Vec<u8>>> {
    sqlx::query_scalar!(
        "SELECT response FROM image_timestamps WHERE image_id = $1",
        image_id
    )
    .fetch_optional(db)
    .await
}

pub async fn has_timestamp(db: impl PgExecutor<'_>, image_id: i64) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM image_timestamps WHERE image_id = $1) AS "e!""#,
        image_id
    )
    .fetch_one(db)
    .await
}

pub async fn insert_timestamp(
    db: impl PgExecutor<'_>,
    image_id: i64,
    tsa_url: &str,
    response: &[u8],
    gen_time: DateTime<Utc>,
    serial: &str,
) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO image_timestamps (image_id, tsa_url, response, gen_time, serial)
         VALUES ($1, $2, $3, $4, $5) ON CONFLICT (image_id) DO NOTHING",
        image_id,
        tsa_url,
        response,
        gen_time,
        serial
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Evidence photos (intake, inspection) not stamped yet, oldest first.
pub async fn unstamped_evidence(db: impl PgExecutor<'_>, limit: i64) -> sqlx::Result<Vec<i64>> {
    sqlx::query_scalar!(
        "SELECT i.id FROM images i
         WHERE i.category IN ('intake', 'inspection') AND i.deleted_at IS NULL
           AND NOT EXISTS (SELECT 1 FROM image_timestamps t WHERE t.image_id = i.id)
         ORDER BY i.id LIMIT $1",
        limit
    )
    .fetch_all(db)
    .await
}

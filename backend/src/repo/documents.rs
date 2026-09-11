use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgConnection, PgExecutor};

use crate::domain::media::DocumentKind;

#[derive(Debug, Clone, Serialize)]
pub struct Document {
    pub id: i64,
    pub order_id: i64,
    pub kind: DocumentKind,
    pub filename: String,
    pub content_type: String,
    #[serde(skip)]
    pub storage_key: String,
    #[serde(serialize_with = "super::images::hex_bytes")]
    pub content_hash: Vec<u8>,
    pub byte_size: i64,
    pub uploaded_at: DateTime<Utc>,
    pub uploaded_by: Option<i64>,
    pub deleted_at: Option<DateTime<Utc>>,
}

pub struct NewDocument<'a> {
    pub order_id: i64,
    pub kind: DocumentKind,
    pub filename: &'a str,
    pub content_type: &'a str,
    pub storage_key: &'a str,
    pub content_hash: &'a [u8],
    pub byte_size: i64,
    pub uploaded_by: Option<i64>,
    pub source_ref: Option<&'a str>,
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Document>> {
    sqlx::query_as!(
        Document,
        r#"SELECT id, order_id, kind AS "kind: DocumentKind", filename, content_type, storage_key, content_hash,
                  byte_size, uploaded_at, uploaded_by, deleted_at
           FROM documents WHERE id = $1"#,
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn list_for_order(db: impl PgExecutor<'_>, order_id: i64) -> sqlx::Result<Vec<Document>> {
    sqlx::query_as!(
        Document,
        r#"SELECT id, order_id, kind AS "kind: DocumentKind", filename, content_type, storage_key, content_hash,
                  byte_size, uploaded_at, uploaded_by, deleted_at
           FROM documents WHERE order_id = $1 AND deleted_at IS NULL
           ORDER BY uploaded_at DESC, id DESC"#,
        order_id
    )
    .fetch_all(db)
    .await
}

pub async fn find_many(db: impl PgExecutor<'_>, ids: &[i64]) -> sqlx::Result<Vec<Document>> {
    sqlx::query_as!(
        Document,
        r#"SELECT id, order_id, kind AS "kind: DocumentKind", filename, content_type, storage_key, content_hash,
                  byte_size, uploaded_at, uploaded_by, deleted_at
           FROM documents WHERE id = ANY($1) AND deleted_at IS NULL
           ORDER BY id"#,
        ids
    )
    .fetch_all(db)
    .await
}

pub async fn find_by_hash(
    db: impl PgExecutor<'_>,
    order_id: i64,
    hash: &[u8],
) -> sqlx::Result<Option<Document>> {
    sqlx::query_as!(
        Document,
        r#"SELECT id, order_id, kind AS "kind: DocumentKind", filename, content_type, storage_key, content_hash,
                  byte_size, uploaded_at, uploaded_by, deleted_at
           FROM documents WHERE order_id = $1 AND content_hash = $2 AND deleted_at IS NULL"#,
        order_id,
        hash
    )
    .fetch_optional(db)
    .await
}

/// Idempotent: the same file finalized twice for one order yields the existing row.
pub async fn insert(
    conn: &mut PgConnection,
    d: &NewDocument<'_>,
) -> sqlx::Result<(Document, bool)> {
    let inserted = sqlx::query_as!(
        Document,
        r#"INSERT INTO documents (order_id, kind, filename, content_type, storage_key, content_hash, byte_size, uploaded_by, source_ref)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           ON CONFLICT (order_id, content_hash) WHERE deleted_at IS NULL DO NOTHING
           RETURNING id, order_id, kind AS "kind: DocumentKind", filename, content_type, storage_key, content_hash,
                     byte_size, uploaded_at, uploaded_by, deleted_at"#,
        d.order_id,
        d.kind as DocumentKind,
        d.filename,
        d.content_type,
        d.storage_key,
        d.content_hash,
        d.byte_size,
        d.uploaded_by,
        d.source_ref
    )
    .fetch_optional(&mut *conn)
    .await?;
    match inserted {
        Some(doc) => Ok((doc, true)),
        None => find_by_hash(&mut *conn, d.order_id, d.content_hash)
            .await?
            .map(|doc| (doc, false))
            .ok_or(sqlx::Error::RowNotFound),
    }
}

pub async fn soft_delete(db: impl PgExecutor<'_>, id: i64, user_id: i64) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "UPDATE documents SET deleted_at = now(), deleted_by = $2 WHERE id = $1 AND deleted_at IS NULL",
        id,
        user_id
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

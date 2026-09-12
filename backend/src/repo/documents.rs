use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::{PgConnection, PgExecutor};

use crate::domain::media::DocumentKind;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Document {
    pub id: i64,
    /// Exactly one of `order_id` and `lead_id` is set (V2.4): a quotation belongs to the
    /// lead it was sent for, and had nowhere to live before this.
    pub order_id: Option<i64>,
    pub lead_id: Option<i64>,
    /// Which vehicle on a multi-vehicle order this covers (V2.1). Usually null.
    pub vehicle_id: Option<i64>,
    pub kind: DocumentKind,
    pub filename: String,
    pub content_type: String,
    #[serde(skip)]
    pub storage_key: String,
    /// Hex-encoded sha256 of the file.
    #[serde(serialize_with = "super::images::hex_bytes")]
    #[schema(value_type = String)]
    pub content_hash: Vec<u8>,
    pub byte_size: i64,
    /// V2.5: an ATP certificate is issued by somebody and expires.
    pub issuer: Option<String>,
    pub valid_from: Option<NaiveDate>,
    pub valid_until: Option<NaiveDate>,
    pub uploaded_at: DateTime<Utc>,
    pub uploaded_by: Option<i64>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// What a document hangs off. Exactly one, enforced by `documents_one_owner`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner {
    Order(i64),
    Lead(i64),
}

impl Owner {
    pub fn ids(self) -> (Option<i64>, Option<i64>) {
        match self {
            Owner::Order(id) => (Some(id), None),
            Owner::Lead(id) => (None, Some(id)),
        }
    }

    /// The owner an email is about. An order wins when both are present: the document
    /// belongs to the job by then.
    pub fn from_about(order_id: Option<i64>, lead_id: Option<i64>) -> Option<Owner> {
        order_id
            .map(Owner::Order)
            .or_else(|| lead_id.map(Owner::Lead))
    }

    pub fn owns(self, d: &Document) -> bool {
        match self {
            Owner::Order(id) => d.order_id == Some(id),
            Owner::Lead(id) => d.lead_id == Some(id),
        }
    }

    /// Audit-log entity and id.
    pub fn entity(self) -> (&'static str, i64) {
        match self {
            Owner::Order(id) => ("order", id),
            Owner::Lead(id) => ("lead", id),
        }
    }
}

pub struct NewDocument<'a> {
    pub owner: Owner,
    pub vehicle_id: Option<i64>,
    pub kind: DocumentKind,
    pub filename: &'a str,
    pub content_type: &'a str,
    pub storage_key: &'a str,
    pub content_hash: &'a [u8],
    pub byte_size: i64,
    pub uploaded_by: Option<i64>,
    pub source_ref: Option<&'a str>,
}

/// Validity metadata, patched separately from the bytes (V2.5).
#[derive(Debug, Clone, Default)]
pub struct Validity {
    pub issuer: Option<String>,
    pub valid_from: Option<NaiveDate>,
    pub valid_until: Option<NaiveDate>,
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Document>> {
    sqlx::query_as!(
        Document,
        r#"SELECT id, order_id, lead_id, vehicle_id, kind AS "kind: DocumentKind", filename, content_type,
                  storage_key, content_hash, byte_size, issuer, valid_from, valid_until,
                  uploaded_at, uploaded_by, deleted_at
           FROM documents WHERE id = $1"#,
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn list_for_order(db: impl PgExecutor<'_>, order_id: i64) -> sqlx::Result<Vec<Document>> {
    sqlx::query_as!(
        Document,
        r#"SELECT id, order_id, lead_id, vehicle_id, kind AS "kind: DocumentKind", filename, content_type,
                  storage_key, content_hash, byte_size, issuer, valid_from, valid_until,
                  uploaded_at, uploaded_by, deleted_at
           FROM documents WHERE order_id = $1 AND deleted_at IS NULL
           ORDER BY uploaded_at DESC, id DESC"#,
        order_id
    )
    .fetch_all(db)
    .await
}

pub async fn list_for_lead(db: impl PgExecutor<'_>, lead_id: i64) -> sqlx::Result<Vec<Document>> {
    sqlx::query_as!(
        Document,
        r#"SELECT id, order_id, lead_id, vehicle_id, kind AS "kind: DocumentKind", filename, content_type,
                  storage_key, content_hash, byte_size, issuer, valid_from, valid_until,
                  uploaded_at, uploaded_by, deleted_at
           FROM documents WHERE lead_id = $1 AND deleted_at IS NULL
           ORDER BY uploaded_at DESC, id DESC"#,
        lead_id
    )
    .fetch_all(db)
    .await
}

pub async fn find_many(db: impl PgExecutor<'_>, ids: &[i64]) -> sqlx::Result<Vec<Document>> {
    sqlx::query_as!(
        Document,
        r#"SELECT id, order_id, lead_id, vehicle_id, kind AS "kind: DocumentKind", filename, content_type,
                  storage_key, content_hash, byte_size, issuer, valid_from, valid_until,
                  uploaded_at, uploaded_by, deleted_at
           FROM documents WHERE id = ANY($1) AND deleted_at IS NULL
           ORDER BY id"#,
        ids
    )
    .fetch_all(db)
    .await
}

/// Cross-owner document query (V2.5). The first one in the system: before this the only
/// way to reach a document was through its order, so "which ATP certificates expire next
/// quarter" had no answer at any layer.
pub async fn search(
    db: impl PgExecutor<'_>,
    kind: Option<DocumentKind>,
    expiring_before: Option<NaiveDate>,
    vehicle_id: Option<i64>,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<Document>> {
    sqlx::query_as!(
        Document,
        r#"SELECT id, order_id, lead_id, vehicle_id, kind AS "kind: DocumentKind", filename, content_type,
                  storage_key, content_hash, byte_size, issuer, valid_from, valid_until,
                  uploaded_at, uploaded_by, deleted_at
           FROM documents
          WHERE deleted_at IS NULL
            AND ($1::document_kind IS NULL OR kind = $1)
            AND ($2::date IS NULL OR (valid_until IS NOT NULL AND valid_until <= $2))
            AND ($3::bigint IS NULL OR vehicle_id = $3)
          ORDER BY valid_until ASC NULLS LAST, id DESC
          LIMIT $4 OFFSET $5"#,
        kind as Option<DocumentKind>,
        expiring_before,
        vehicle_id,
        limit,
        offset
    )
    .fetch_all(db)
    .await
}

pub async fn find_by_hash(
    db: impl PgExecutor<'_>,
    owner: Owner,
    hash: &[u8],
) -> sqlx::Result<Option<Document>> {
    let (order_id, lead_id) = owner.ids();
    sqlx::query_as!(
        Document,
        r#"SELECT id, order_id, lead_id, vehicle_id, kind AS "kind: DocumentKind", filename, content_type,
                  storage_key, content_hash, byte_size, issuer, valid_from, valid_until,
                  uploaded_at, uploaded_by, deleted_at
           FROM documents
          WHERE content_hash = $3 AND deleted_at IS NULL
            AND order_id IS NOT DISTINCT FROM $1 AND lead_id IS NOT DISTINCT FROM $2"#,
        order_id,
        lead_id,
        hash
    )
    .fetch_optional(db)
    .await
}

/// Idempotent: the same file finalized twice for one owner yields the existing row.
/// Two ON CONFLICT targets because the dedup index is partial on each owner column — one
/// index cannot cover both without hiding a null.
pub async fn insert(
    conn: &mut PgConnection,
    d: &NewDocument<'_>,
) -> sqlx::Result<(Document, bool)> {
    let (order_id, lead_id) = d.owner.ids();
    let inserted = match d.owner {
        Owner::Order(_) => {
            sqlx::query_as!(
                Document,
                r#"INSERT INTO documents (order_id, lead_id, vehicle_id, kind, filename, content_type,
                                          storage_key, content_hash, byte_size, uploaded_by, source_ref)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                   ON CONFLICT (order_id, content_hash) WHERE deleted_at IS NULL AND order_id IS NOT NULL DO NOTHING
                   RETURNING id, order_id, lead_id, vehicle_id, kind AS "kind: DocumentKind", filename, content_type,
                             storage_key, content_hash, byte_size, issuer, valid_from, valid_until,
                             uploaded_at, uploaded_by, deleted_at"#,
                order_id,
                lead_id,
                d.vehicle_id,
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
            .await?
        }
        Owner::Lead(_) => {
            sqlx::query_as!(
                Document,
                r#"INSERT INTO documents (order_id, lead_id, vehicle_id, kind, filename, content_type,
                                          storage_key, content_hash, byte_size, uploaded_by, source_ref)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                   ON CONFLICT (lead_id, content_hash) WHERE deleted_at IS NULL AND lead_id IS NOT NULL DO NOTHING
                   RETURNING id, order_id, lead_id, vehicle_id, kind AS "kind: DocumentKind", filename, content_type,
                             storage_key, content_hash, byte_size, issuer, valid_from, valid_until,
                             uploaded_at, uploaded_by, deleted_at"#,
                order_id,
                lead_id,
                d.vehicle_id,
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
            .await?
        }
    };
    match inserted {
        Some(doc) => Ok((doc, true)),
        None => find_by_hash(&mut *conn, d.owner, d.content_hash)
            .await?
            .map(|doc| (doc, false))
            .ok_or(sqlx::Error::RowNotFound),
    }
}

pub async fn set_validity(
    db: impl PgExecutor<'_>,
    id: i64,
    v: &Validity,
    vehicle_id: Option<i64>,
) -> sqlx::Result<Option<Document>> {
    sqlx::query_as!(
        Document,
        r#"UPDATE documents SET issuer = $2, valid_from = $3, valid_until = $4, vehicle_id = $5
            WHERE id = $1 AND deleted_at IS NULL
           RETURNING id, order_id, lead_id, vehicle_id, kind AS "kind: DocumentKind", filename, content_type,
                     storage_key, content_hash, byte_size, issuer, valid_from, valid_until,
                     uploaded_at, uploaded_by, deleted_at"#,
        id,
        v.issuer,
        v.valid_from,
        v.valid_until,
        vehicle_id
    )
    .fetch_optional(db)
    .await
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

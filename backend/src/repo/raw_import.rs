//! The MiniCRM source record kept on every migrated row (V1.5).
//!
//! `raw_import` was populated from day one and read by nothing: not an API, not a screen,
//! not a search predicate. A staff member opening a migrated 2019 order saw a title, a
//! partner and a date, with no way to discover that the van's plate, the customer's
//! address and forty activity entries were sitting in a JSONB column. These queries are
//! the read side of that column.

use serde_json::Value;
use sqlx::PgPool;

#[derive(Debug)]
pub struct RawImport {
    pub minicrm_id: i64,
    pub raw_import: Value,
}

/// `Ok(None)` means no such row; `Ok(Some(None))` means the row exists but was not
/// migrated — the caller shows nothing rather than an error.
type Found = Option<Option<RawImport>>;

fn found(row: Option<(Option<i64>, Option<Value>)>) -> Found {
    row.map(|(minicrm_id, raw_import)| {
        Some(RawImport {
            minicrm_id: minicrm_id?,
            raw_import: raw_import?,
        })
    })
}

pub async fn for_partner(db: &PgPool, id: i64) -> sqlx::Result<Found> {
    let row = sqlx::query!(
        "SELECT minicrm_id, raw_import FROM partners WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await?;
    Ok(found(row.map(|r| (r.minicrm_id, r.raw_import))))
}

pub async fn for_lead(db: &PgPool, id: i64) -> sqlx::Result<Found> {
    let row = sqlx::query!("SELECT minicrm_id, raw_import FROM leads WHERE id = $1", id)
        .fetch_optional(db)
        .await?;
    Ok(found(row.map(|r| (r.minicrm_id, r.raw_import))))
}

pub async fn for_order(db: &PgPool, id: i64) -> sqlx::Result<Found> {
    let row = sqlx::query!(
        "SELECT minicrm_id, raw_import FROM orders WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await?;
    Ok(found(row.map(|r| (r.minicrm_id, r.raw_import))))
}

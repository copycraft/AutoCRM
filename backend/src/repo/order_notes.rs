//! Imported MiniCRM activity on an order (V1.3). Read-only: nothing in AutoCRM writes
//! here except the migration loader.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct OrderNote {
    pub id: i64,
    pub order_id: i64,
    /// Provenance: the MiniCRM to-do this came from.
    pub minicrm_id: Option<i64>,
    /// The MiniCRM user's name as text — those accounts do not exist in AutoCRM.
    pub author_name: Option<String>,
    pub body: String,
    pub occurred_at: DateTime<Utc>,
}

pub async fn list_for_order(db: &PgPool, order_id: i64) -> sqlx::Result<Vec<OrderNote>> {
    sqlx::query_as!(
        OrderNote,
        "SELECT id, order_id, minicrm_id, author_name, body, occurred_at
           FROM order_notes WHERE order_id = $1
          ORDER BY occurred_at DESC, id DESC",
        order_id
    )
    .fetch_all(db)
    .await
}

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgExecutor;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct OrderItem {
    pub id: i64,
    pub order_id: i64,
    pub position: i32,
    pub description: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub quantity: Decimal,
    pub unit_price: i64,
    #[schema(value_type = crate::domain::money::Currency)]
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn list(db: impl PgExecutor<'_>, order_id: i64) -> sqlx::Result<Vec<OrderItem>> {
    sqlx::query_as!(
        OrderItem,
        "SELECT id, order_id, position, description, quantity, unit_price, currency, created_at, updated_at
         FROM order_items WHERE order_id = $1 ORDER BY position, id",
        order_id
    )
    .fetch_all(db)
    .await
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<OrderItem>> {
    sqlx::query_as!(
        OrderItem,
        "SELECT id, order_id, position, description, quantity, unit_price, currency, created_at, updated_at
         FROM order_items WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await
}

pub async fn next_position(db: impl PgExecutor<'_>, order_id: i64) -> sqlx::Result<i32> {
    let max: Option<i32> = sqlx::query_scalar!(
        "SELECT max(position) FROM order_items WHERE order_id = $1",
        order_id
    )
    .fetch_one(db)
    .await?;
    Ok(max.map_or(10, |m| m + 10))
}

pub async fn insert(
    db: impl PgExecutor<'_>,
    order_id: i64,
    position: i32,
    description: &str,
    quantity: Decimal,
    unit_price: i64,
    currency: &str,
) -> sqlx::Result<OrderItem> {
    sqlx::query_as!(
        OrderItem,
        "INSERT INTO order_items (order_id, position, description, quantity, unit_price, currency)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, order_id, position, description, quantity, unit_price, currency, created_at, updated_at",
        order_id,
        position,
        description,
        quantity,
        unit_price,
        currency
    )
    .fetch_one(db)
    .await
}

pub async fn update(
    db: impl PgExecutor<'_>,
    id: i64,
    position: i32,
    description: &str,
    quantity: Decimal,
    unit_price: i64,
) -> sqlx::Result<Option<OrderItem>> {
    sqlx::query_as!(
        OrderItem,
        "UPDATE order_items SET position = $2, description = $3, quantity = $4, unit_price = $5
         WHERE id = $1
         RETURNING id, order_id, position, description, quantity, unit_price, currency, created_at, updated_at",
        id,
        position,
        description,
        quantity,
        unit_price
    )
    .fetch_optional(db)
    .await
}

pub async fn delete(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let r = sqlx::query!("DELETE FROM order_items WHERE id = $1", id)
        .execute(db)
        .await?;
    Ok(r.rows_affected() == 1)
}

pub async fn count(db: impl PgExecutor<'_>, order_id: i64) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM order_items WHERE order_id = $1"#,
        order_id
    )
    .fetch_one(db)
    .await
}

//! Invoices being prepared in the Számlázó (0058). The lines are the order's items; a draft
//! holds only the invoice's own terms. Runtime-checked queries, like `employees`.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgExecutor;

#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct InvoiceDraft {
    pub order_id: i64,
    /// The fraction (0.27); null: the configured default at issue time.
    #[schema(value_type = Option<String>)]
    #[serde(with = "rust_decimal::serde::str_option")]
    pub vat_rate: Option<Decimal>,
    /// Null: the day it is issued.
    pub issue_date: Option<NaiveDate>,
    /// Null: the issue date.
    pub delivery_date: Option<NaiveDate>,
    /// Null: the issue date plus the configured payment days.
    pub payment_date: Option<NaiveDate>,
    /// `TRANSFER` or `CASH`; null: transfer.
    pub payment_method: Option<String>,
    pub created_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A draft as the billing desk lists it: which job, for whom, and what the items add up to.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct DraftSummary {
    pub order_id: i64,
    pub order_number: String,
    pub order_title: String,
    pub partner_name: String,
    #[schema(value_type = crate::domain::money::Currency)]
    pub currency: String,
    /// The order's items, net, in minor units.
    pub net_minor: i64,
    pub created_by_name: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// The invoice's own terms, as written to a draft.
#[derive(Debug, Clone, Default)]
pub struct Terms {
    pub vat_rate: Option<Decimal>,
    pub issue_date: Option<NaiveDate>,
    pub delivery_date: Option<NaiveDate>,
    pub payment_date: Option<NaiveDate>,
    pub payment_method: Option<String>,
}

const COLUMNS: &str = "order_id, vat_rate, issue_date, delivery_date, payment_date, payment_method,
                       created_by, created_at, updated_at";

pub async fn find(db: impl PgExecutor<'_>, order_id: i64) -> sqlx::Result<Option<InvoiceDraft>> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM invoice_drafts WHERE order_id = $1"
    ))
    .bind(order_id)
    .fetch_optional(db)
    .await
}

/// Writes the draft's terms, creating it if the order has none yet.
pub async fn upsert(
    db: impl PgExecutor<'_>,
    order_id: i64,
    terms: &Terms,
    user_id: i64,
) -> sqlx::Result<InvoiceDraft> {
    sqlx::query_as(&format!(
        "INSERT INTO invoice_drafts (order_id, vat_rate, issue_date, delivery_date, payment_date,
                                     payment_method, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (order_id) DO UPDATE
            SET vat_rate = EXCLUDED.vat_rate, issue_date = EXCLUDED.issue_date,
                delivery_date = EXCLUDED.delivery_date, payment_date = EXCLUDED.payment_date,
                payment_method = EXCLUDED.payment_method
         RETURNING {COLUMNS}"
    ))
    .bind(order_id)
    .bind(terms.vat_rate)
    .bind(terms.issue_date)
    .bind(terms.delivery_date)
    .bind(terms.payment_date)
    .bind(&terms.payment_method)
    .bind(user_id)
    .fetch_one(db)
    .await
}

pub async fn delete(db: impl PgExecutor<'_>, order_id: i64) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM invoice_drafts WHERE order_id = $1")
        .bind(order_id)
        .execute(db)
        .await?;
    Ok(done.rows_affected() > 0)
}

/// Every draft waiting in the Számlázó, oldest first: that one has waited longest.
pub async fn list(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<DraftSummary>> {
    sqlx::query_as(
        "SELECT d.order_id, o.number AS order_number, o.title AS order_title,
                p.name AS partner_name, o.currency,
                COALESCE((SELECT sum(round(i.quantity * i.unit_price))::bigint
                            FROM order_items i WHERE i.order_id = o.id), 0) AS net_minor,
                u.display_name AS created_by_name, d.updated_at
           FROM invoice_drafts d
           JOIN orders o ON o.id = d.order_id
           JOIN partners p ON p.id = o.partner_id
           LEFT JOIN users u ON u.id = d.created_by
          ORDER BY d.created_at, d.order_id",
    )
    .fetch_all(db)
    .await
}

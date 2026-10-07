//! Payments received on outgoing invoices (0049). A customer may pay in parts; the invoice
//! keeps the running sum in `paid_amount` and counts as paid (`paid_at`) once it reaches
//! the gross.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::{PgConnection, PgExecutor};

#[derive(Debug, Clone, Serialize, utoipa::ToSchema, sqlx::FromRow)]
pub struct InvoicePayment {
    pub id: i64,
    pub invoice_id: i64,
    /// Minor units, in the invoice's currency.
    pub amount_minor: i64,
    pub paid_on: NaiveDate,
    /// TRANSFER, CASH, CARD or OTHER.
    pub method: String,
    pub note: Option<String>,
    pub created_by: Option<i64>,
    pub created_by_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn list(db: impl PgExecutor<'_>, invoice_id: i64) -> sqlx::Result<Vec<InvoicePayment>> {
    sqlx::query_as(
        "SELECT p.id, p.invoice_id, p.amount_minor, p.paid_on, p.method, p.note, p.created_by,
                u.display_name AS created_by_name, p.created_at
           FROM invoice_payments p
           LEFT JOIN users u ON u.id = p.created_by
          WHERE p.invoice_id = $1
          ORDER BY p.paid_on, p.id",
    )
    .bind(invoice_id)
    .fetch_all(db)
    .await
}

/// What a payment can be booked against: an issued invoice (not a storno), locked.
pub struct Payable {
    pub gross_amount: i64,
    pub paid_amount: i64,
    pub currency: String,
}

pub async fn lock_payable(
    conn: &mut PgConnection,
    invoice_id: i64,
) -> sqlx::Result<Option<Payable>> {
    let row: Option<(i64, i64, String)> = sqlx::query_as(
        "SELECT gross_amount, paid_amount, currency FROM invoices
          WHERE id = $1 AND kind = 'invoice' AND status = 'issued'
          FOR UPDATE",
    )
    .bind(invoice_id)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(row.map(|(gross_amount, paid_amount, currency)| Payable {
        gross_amount,
        paid_amount,
        currency,
    }))
}

pub struct NewPayment<'a> {
    pub invoice_id: i64,
    pub amount_minor: i64,
    pub paid_on: NaiveDate,
    pub method: &'a str,
    pub note: Option<&'a str>,
    pub created_by: Option<i64>,
}

/// Books a payment and moves the invoice's sum (and `paid_at` when it is now settled).
/// The caller holds the lock from [`lock_payable`] and has checked the amount.
pub async fn insert(conn: &mut PgConnection, p: &NewPayment<'_>) -> sqlx::Result<i64> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO invoice_payments (invoice_id, amount_minor, paid_on, method, note, created_by)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(p.invoice_id)
    .bind(p.amount_minor)
    .bind(p.paid_on)
    .bind(p.method)
    .bind(p.note)
    .bind(p.created_by)
    .fetch_one(&mut *conn)
    .await?;
    resum(conn, p.invoice_id).await?;
    Ok(id)
}

/// Removes a payment booked by mistake. False when it is not on this invoice.
pub async fn delete(
    conn: &mut PgConnection,
    invoice_id: i64,
    payment_id: i64,
) -> sqlx::Result<bool> {
    let done = sqlx::query("DELETE FROM invoice_payments WHERE id = $1 AND invoice_id = $2")
        .bind(payment_id)
        .bind(invoice_id)
        .execute(&mut *conn)
        .await?;
    if done.rows_affected() == 1 {
        resum(conn, invoice_id).await?;
    }
    Ok(done.rows_affected() == 1)
}

/// Recomputes the sum from the payments. Settled: `paid_at` is the day of the payment that
/// completed it; not settled: no `paid_at`.
async fn resum(conn: &mut PgConnection, invoice_id: i64) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE invoices i
            SET paid_amount = s.total,
                paid_at = CASE WHEN s.total >= i.gross_amount AND i.gross_amount > 0
                               THEN coalesce(i.paid_at, (s.last_day::timestamp AT TIME ZONE 'Europe/Budapest'))
                          END
           FROM (SELECT coalesce(sum(amount_minor), 0)::bigint AS total, max(paid_on) AS last_day
                   FROM invoice_payments WHERE invoice_id = $1) s
          WHERE i.id = $1",
    )
    .bind(invoice_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// One line of a partner's statement of account: an issued invoice or storno.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema, sqlx::FromRow)]
pub struct StatementLine {
    pub invoice_id: i64,
    pub order_id: i64,
    pub order_number: String,
    pub number: String,
    /// invoice or storno.
    pub kind: String,
    /// issued, paid, stornoed or storno.
    pub bucket: String,
    pub currency: String,
    pub issue_date: NaiveDate,
    pub payment_date: Option<NaiveDate>,
    pub gross_amount: i64,
    pub paid_amount: i64,
    /// Still to pay: gross less payments, on an invoice that stands (not stornoed).
    pub outstanding: i64,
    pub paid_at: Option<DateTime<Utc>>,
}

/// The partner's invoices issued in the period, and any older ones still open.
pub async fn statement(
    db: impl PgExecutor<'_>,
    partner_id: i64,
    from: NaiveDate,
    to: NaiveDate,
) -> sqlx::Result<Vec<StatementLine>> {
    sqlx::query_as(
        "SELECT i.id AS invoice_id, i.order_id, o.number AS order_number, i.number,
                i.kind::text AS kind, b.bucket, i.currency, i.issue_date, i.payment_date,
                i.gross_amount, i.paid_amount,
                CASE WHEN b.bucket = 'issued' THEN greatest(i.gross_amount - i.paid_amount, 0)
                     ELSE 0 END AS outstanding,
                i.paid_at
           FROM invoices i
           JOIN invoice_buckets b ON b.id = i.id
           JOIN orders o ON o.id = i.order_id
          WHERE o.partner_id = $1
            AND b.bucket IN ('issued', 'paid', 'stornoed', 'storno')
            AND ((i.issue_date BETWEEN $2 AND $3) OR b.bucket = 'issued')
          ORDER BY i.currency, i.issue_date, i.id",
    )
    .bind(partner_id)
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await
}

/// The payments booked against a partner's invoices in the period.
pub async fn statement_payments(
    db: impl PgExecutor<'_>,
    partner_id: i64,
    from: NaiveDate,
    to: NaiveDate,
) -> sqlx::Result<Vec<InvoicePayment>> {
    sqlx::query_as(
        "SELECT p.id, p.invoice_id, p.amount_minor, p.paid_on, p.method, p.note, p.created_by,
                u.display_name AS created_by_name, p.created_at
           FROM invoice_payments p
           JOIN invoices i ON i.id = p.invoice_id
           JOIN orders o ON o.id = i.order_id
           LEFT JOIN users u ON u.id = p.created_by
          WHERE o.partner_id = $1 AND p.paid_on BETWEEN $2 AND $3
          ORDER BY p.paid_on, p.id",
    )
    .bind(partner_id)
    .bind(from)
    .bind(to)
    .fetch_all(db)
    .await
}

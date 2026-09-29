//! Invoices, their line snapshots, and proformas.
//!
//! Everything NAV told us is stored verbatim next to the row it is about: the transaction
//! id, the status, the fault code and the messages. That is what makes "why was this
//! rejected" answerable months later, by someone who was not there.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::Value;
use sqlx::{PgConnection, PgExecutor};

use crate::domain::invoice::{InvoiceKind, InvoiceStatus};
use crate::integrations::nav::NavMessage;

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Invoice {
    pub id: i64,
    pub order_id: i64,
    /// The number NAV knows this document by.
    pub number: String,
    pub kind: InvoiceKind,
    pub status: InvoiceStatus,
    /// The invoice this one cancels. Set exactly on a storno.
    pub original_invoice_id: Option<i64>,
    pub currency: String,
    pub issue_date: NaiveDate,
    pub delivery_date: NaiveDate,
    pub payment_date: Option<NaiveDate>,
    /// How the customer pays: one of TRANSFER, CASH, CARD, VOUCHER, OTHER.
    pub payment_method: String,
    /// Minor units (fillér / eurocent). Negative on a storno: it is the reversal.
    pub net_amount: i64,
    pub vat_amount: i64,
    pub gross_amount: i64,
    /// NAV's handle for the submission. Worth quoting in any support question.
    pub nav_transaction_id: Option<String>,
    /// NAV's own per-invoice status: `DONE` when stored, `ABORTED` when rejected.
    pub nav_status: Option<String>,
    /// NAV's fault code on a rejection, e.g. `INVOICE_NUMBER_ALREADY_EXISTS`.
    pub nav_error_code: Option<String>,
    pub nav_message: Option<String>,
    /// Everything NAV said, including warnings on an accepted invoice.
    #[schema(value_type = Vec<NavMessage>)]
    pub nav_messages: Value,
    pub annulment_transaction_id: Option<String>,
    pub annulment_code: Option<String>,
    pub annulment_reason: Option<String>,
    pub annulled_at: Option<DateTime<Utc>>,
    /// The stored PDF, once there is one.
    pub document_id: Option<i64>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub issued_at: Option<DateTime<Utc>>,
    pub created_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct InvoiceLine {
    pub id: i64,
    pub invoice_id: i64,
    pub position: i32,
    pub description: String,
    #[schema(value_type = String)]
    pub quantity: Decimal,
    pub unit: String,
    pub unit_price: i64,
    /// The fraction: 0.2700 is 27%.
    #[schema(value_type = String)]
    pub vat_rate: Decimal,
    pub net_amount: i64,
    pub vat_amount: i64,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Proforma {
    pub id: i64,
    pub order_id: i64,
    pub number: String,
    pub currency: String,
    pub issue_date: NaiveDate,
    pub payment_date: Option<NaiveDate>,
    pub net_amount: i64,
    pub vat_amount: i64,
    pub gross_amount: i64,
    /// The rendered PDF. Always present: a proforma is its document.
    pub document_id: i64,
    pub created_by: Option<i64>,
    pub created_at: DateTime<Utc>,
}

pub struct NewInvoice<'a> {
    pub order_id: i64,
    pub number: &'a str,
    pub kind: InvoiceKind,
    pub original_invoice_id: Option<i64>,
    pub currency: &'a str,
    pub issue_date: NaiveDate,
    pub delivery_date: NaiveDate,
    pub payment_date: Option<NaiveDate>,
    pub payment_method: &'a str,
    pub net_amount: i64,
    pub vat_amount: i64,
    pub gross_amount: i64,
    pub created_by: Option<i64>,
}

pub struct NewLine<'a> {
    pub position: i32,
    pub description: &'a str,
    pub quantity: Decimal,
    pub unit: &'a str,
    pub unit_price: i64,
    pub vat_rate: Decimal,
    pub net_amount: i64,
    pub vat_amount: i64,
}

pub struct NewProforma<'a> {
    pub order_id: i64,
    pub number: &'a str,
    pub currency: &'a str,
    pub issue_date: NaiveDate,
    pub payment_date: Option<NaiveDate>,
    pub net_amount: i64,
    pub vat_amount: i64,
    pub gross_amount: i64,
    pub document_id: i64,
    pub created_by: Option<i64>,
}

/// Next number in the invoice series. Takes a transaction-scoped advisory lock, like order
/// numbers: two invoices issued in the same second must not draw the same number, and NAV
/// rejects a reused one outright.
pub async fn next_number(conn: &mut PgConnection, prefix: &str, year: i32) -> sqlx::Result<String> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtext('autocrm.invoice_number'))")
        .execute(&mut *conn)
        .await?;
    let series = format!("{prefix}{year}");
    let max: Option<i32> = sqlx::query_scalar!(
        r#"SELECT max(split_part(number, '-', 2)::int) FROM invoices
            WHERE number ~ ('^' || $1::text || '-[0-9]{4,9}$')"#,
        series
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(crate::domain::invoice::format_invoice_number(
        prefix,
        year,
        max.unwrap_or(0) + 1,
    ))
}

/// Next number in the proforma series. Its own series and its own lock: a díjbekérő is not
/// an invoice and must never consume an invoice number.
pub async fn next_proforma_number(
    conn: &mut PgConnection,
    prefix: &str,
    year: i32,
) -> sqlx::Result<String> {
    sqlx::query!("SELECT pg_advisory_xact_lock(hashtext('autocrm.proforma_number'))")
        .execute(&mut *conn)
        .await?;
    let series = format!("{prefix}{year}");
    let max: Option<i32> = sqlx::query_scalar!(
        r#"SELECT max(split_part(number, '-', 2)::int) FROM proformas
            WHERE number ~ ('^' || $1::text || '-[0-9]{4,9}$')"#,
        series
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(crate::domain::invoice::format_invoice_number(
        prefix,
        year,
        max.unwrap_or(0) + 1,
    ))
}

pub async fn insert(conn: &mut PgConnection, i: &NewInvoice<'_>) -> sqlx::Result<Invoice> {
    sqlx::query_as!(
        Invoice,
        r#"INSERT INTO invoices (order_id, number, kind, status, original_invoice_id, currency,
                                 issue_date, delivery_date, payment_date, payment_method, net_amount,
                                 vat_amount, gross_amount, created_by, submitted_at)
           VALUES ($1, $2, $3, 'submitting', $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, now())
           RETURNING id, order_id, number, kind AS "kind: InvoiceKind", status AS "status: InvoiceStatus",
                     original_invoice_id, currency, issue_date, delivery_date, payment_date,
                     payment_method, net_amount, vat_amount, gross_amount, nav_transaction_id, nav_status,
                     nav_error_code, nav_message, nav_messages, annulment_transaction_id,
                     annulment_code, annulment_reason, annulled_at, document_id, submitted_at,
                     issued_at, created_by, created_at, updated_at"#,
        i.order_id,
        i.number,
        i.kind as InvoiceKind,
        i.original_invoice_id,
        i.currency,
        i.issue_date,
        i.delivery_date,
        i.payment_date,
        i.payment_method,
        i.net_amount,
        i.vat_amount,
        i.gross_amount,
        i.created_by
    )
    .fetch_one(&mut *conn)
    .await
}

pub async fn insert_line(
    conn: &mut PgConnection,
    invoice_id: i64,
    line: &NewLine<'_>,
) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO invoice_lines (invoice_id, position, description, quantity, unit,
                                    unit_price, vat_rate, net_amount, vat_amount)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        invoice_id,
        line.position,
        line.description,
        line.quantity,
        line.unit,
        line.unit_price,
        line.vat_rate,
        line.net_amount,
        line.vat_amount
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Invoice>> {
    sqlx::query_as!(
        Invoice,
        r#"SELECT id, order_id, number, kind AS "kind: InvoiceKind", status AS "status: InvoiceStatus",
                  original_invoice_id, currency, issue_date, delivery_date, payment_date,
                  payment_method, net_amount, vat_amount, gross_amount, nav_transaction_id, nav_status,
                  nav_error_code, nav_message, nav_messages, annulment_transaction_id,
                  annulment_code, annulment_reason, annulled_at, document_id, submitted_at,
                  issued_at, created_by, created_at, updated_at
             FROM invoices WHERE id = $1"#,
        id
    )
    .fetch_optional(db)
    .await
}

/// Locks the row for a state change, so two workers cannot decide different things about
/// the same submission.
pub async fn lock(conn: &mut PgConnection, id: i64) -> sqlx::Result<Option<Invoice>> {
    sqlx::query_as!(
        Invoice,
        r#"SELECT id, order_id, number, kind AS "kind: InvoiceKind", status AS "status: InvoiceStatus",
                  original_invoice_id, currency, issue_date, delivery_date, payment_date,
                  payment_method, net_amount, vat_amount, gross_amount, nav_transaction_id, nav_status,
                  nav_error_code, nav_message, nav_messages, annulment_transaction_id,
                  annulment_code, annulment_reason, annulled_at, document_id, submitted_at,
                  issued_at, created_by, created_at, updated_at
             FROM invoices WHERE id = $1 FOR UPDATE"#,
        id
    )
    .fetch_optional(&mut *conn)
    .await
}

pub async fn list_for_order(db: impl PgExecutor<'_>, order_id: i64) -> sqlx::Result<Vec<Invoice>> {
    sqlx::query_as!(
        Invoice,
        r#"SELECT id, order_id, number, kind AS "kind: InvoiceKind", status AS "status: InvoiceStatus",
                  original_invoice_id, currency, issue_date, delivery_date, payment_date,
                  payment_method, net_amount, vat_amount, gross_amount, nav_transaction_id, nav_status,
                  nav_error_code, nav_message, nav_messages, annulment_transaction_id,
                  annulment_code, annulment_reason, annulled_at, document_id, submitted_at,
                  issued_at, created_by, created_at, updated_at
             FROM invoices WHERE order_id = $1 ORDER BY id DESC"#,
        order_id
    )
    .fetch_all(db)
    .await
}

pub async fn lines_for(db: impl PgExecutor<'_>, invoice_id: i64) -> sqlx::Result<Vec<InvoiceLine>> {
    sqlx::query_as!(
        InvoiceLine,
        r#"SELECT id, invoice_id, position, description, quantity, unit, unit_price,
                  vat_rate, net_amount, vat_amount
             FROM invoice_lines WHERE invoice_id = $1 ORDER BY position, id"#,
        invoice_id
    )
    .fetch_all(db)
    .await
}

/// NAV stored the report. `kind` decides what that means for the document: an invoice
/// becomes `issued`, a storno becomes `issued` and its original becomes `stornoed`.
pub async fn mark_issued(
    conn: &mut PgConnection,
    id: i64,
    transaction_id: &str,
    nav_status: &str,
    messages: &[NavMessage],
) -> sqlx::Result<Option<Invoice>> {
    let messages = serde_json::to_value(messages).unwrap_or(Value::Array(Vec::new()));
    sqlx::query_as!(
        Invoice,
        r#"UPDATE invoices
              SET status = 'issued', nav_transaction_id = $2, nav_status = $3,
                  nav_messages = $4, nav_error_code = NULL, nav_message = NULL,
                  issued_at = now()
            WHERE id = $1
        RETURNING id, order_id, number, kind AS "kind: InvoiceKind", status AS "status: InvoiceStatus",
                  original_invoice_id, currency, issue_date, delivery_date, payment_date,
                  payment_method, net_amount, vat_amount, gross_amount, nav_transaction_id, nav_status,
                  nav_error_code, nav_message, nav_messages, annulment_transaction_id,
                  annulment_code, annulment_reason, annulled_at, document_id, submitted_at,
                  issued_at, created_by, created_at, updated_at"#,
        id,
        transaction_id,
        nav_status,
        messages
    )
    .fetch_optional(&mut *conn)
    .await
}

/// NAV refused it, or the sidecar did. The number is spent; a corrected invoice is a new
/// document with a new number.
pub async fn mark_rejected(
    conn: &mut PgConnection,
    id: i64,
    error_code: Option<&str>,
    message: &str,
    messages: &[NavMessage],
    transaction_id: Option<&str>,
) -> sqlx::Result<Option<Invoice>> {
    let messages = serde_json::to_value(messages).unwrap_or(Value::Array(Vec::new()));
    sqlx::query_as!(
        Invoice,
        r#"UPDATE invoices
              SET status = 'rejected', nav_error_code = $2, nav_message = $3,
                  nav_messages = $4, nav_transaction_id = COALESCE($5, nav_transaction_id),
                  nav_status = 'ABORTED'
            WHERE id = $1
        RETURNING id, order_id, number, kind AS "kind: InvoiceKind", status AS "status: InvoiceStatus",
                  original_invoice_id, currency, issue_date, delivery_date, payment_date,
                  payment_method, net_amount, vat_amount, gross_amount, nav_transaction_id, nav_status,
                  nav_error_code, nav_message, nav_messages, annulment_transaction_id,
                  annulment_code, annulment_reason, annulled_at, document_id, submitted_at,
                  issued_at, created_by, created_at, updated_at"#,
        id,
        error_code,
        message,
        messages,
        transaction_id
    )
    .fetch_optional(&mut *conn)
    .await
}

/// NAV already held the number with our exact totals: adopt the report instead of
/// filing it twice (INV-L8).
///
/// The transaction id of the timed-out first attempt is unknowable — NAV never tells us
/// — so it stays NULL and the row says why in `nav_message`. Everything else is the
/// same `issued` a normal success produces, and the row is returned by re-reading it,
/// so this adds no compile-time-checked query (no `.sqlx` cache entry needed).
pub async fn mark_issued_reconciled(
    conn: &mut PgConnection,
    id: i64,
    message: &str,
) -> sqlx::Result<()> {
    let messages = Value::Array(Vec::new());
    sqlx::query(
        "UPDATE invoices
            SET status = 'issued', nav_status = 'DONE', nav_transaction_id = NULL,
                nav_error_code = NULL, nav_message = $2, nav_messages = $3,
                issued_at = now()
          WHERE id = $1",
    )
    .bind(id)
    .bind(message)
    .bind(messages)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn mark_stornoed(conn: &mut PgConnection, id: i64) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE invoices SET status = 'stornoed' WHERE id = $1 AND status = 'issued'",
        id
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn mark_annulled(
    conn: &mut PgConnection,
    id: i64,
    transaction_id: &str,
    code: &str,
    reason: &str,
) -> sqlx::Result<Option<Invoice>> {
    sqlx::query_as!(
        Invoice,
        r#"UPDATE invoices
              SET status = 'annulled', annulment_transaction_id = $2, annulment_code = $3,
                  annulment_reason = $4, annulled_at = now()
            WHERE id = $1
        RETURNING id, order_id, number, kind AS "kind: InvoiceKind", status AS "status: InvoiceStatus",
                  original_invoice_id, currency, issue_date, delivery_date, payment_date,
                  payment_method, net_amount, vat_amount, gross_amount, nav_transaction_id, nav_status,
                  nav_error_code, nav_message, nav_messages, annulment_transaction_id,
                  annulment_code, annulment_reason, annulled_at, document_id, submitted_at,
                  issued_at, created_by, created_at, updated_at"#,
        id,
        transaction_id,
        code,
        reason
    )
    .fetch_optional(&mut *conn)
    .await
}

pub async fn set_document(conn: &mut PgConnection, id: i64, document_id: i64) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE invoices SET document_id = $2 WHERE id = $1",
        id,
        document_id
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub async fn insert_proforma(
    conn: &mut PgConnection,
    p: &NewProforma<'_>,
) -> sqlx::Result<Proforma> {
    sqlx::query_as!(
        Proforma,
        "INSERT INTO proformas (order_id, number, currency, issue_date, payment_date,
                                net_amount, vat_amount, gross_amount, document_id, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING id, order_id, number, currency, issue_date, payment_date, net_amount,
                   vat_amount, gross_amount, document_id, created_by, created_at",
        p.order_id,
        p.number,
        p.currency,
        p.issue_date,
        p.payment_date,
        p.net_amount,
        p.vat_amount,
        p.gross_amount,
        p.document_id,
        p.created_by
    )
    .fetch_one(&mut *conn)
    .await
}

pub async fn list_proformas_for_order(
    db: impl PgExecutor<'_>,
    order_id: i64,
) -> sqlx::Result<Vec<Proforma>> {
    sqlx::query_as!(
        Proforma,
        "SELECT id, order_id, number, currency, issue_date, payment_date, net_amount,
                vat_amount, gross_amount, document_id, created_by, created_at
           FROM proformas WHERE order_id = $1 ORDER BY id DESC",
        order_id
    )
    .fetch_all(db)
    .await
}

pub async fn find_proforma(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<Proforma>> {
    sqlx::query_as!(
        Proforma,
        "SELECT id, order_id, number, currency, issue_date, payment_date, net_amount,
                vat_amount, gross_amount, document_id, created_by, created_at
           FROM proformas WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await
}

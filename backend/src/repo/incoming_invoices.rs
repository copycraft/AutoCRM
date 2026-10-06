//! Incoming (supplier) invoices: the uploaded document and what the office read off it.
//! The list an invoice is on is worked out from the figures (see BUCKET). Runtime-checked
//! queries, like the other recent tables.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

/// open_invoice, open_proforma, transferred, cash, partial, cash_receipt, booking_only.
const BUCKET: &str = "CASE
        WHEN booking_only THEN 'booking_only'
        WHEN kind = 'receipt' THEN 'cash_receipt'
        WHEN gross_amount > 0 AND paid_amount >= gross_amount THEN
            CASE WHEN payment_method = 'CASH' THEN 'cash' ELSE 'transferred' END
        WHEN paid_amount > 0 THEN 'partial'
        WHEN kind = 'proforma' THEN 'open_proforma'
        ELSE 'open_invoice'
    END";

pub const BUCKETS: &[&str] = &[
    "open_invoice",
    "open_proforma",
    "transferred",
    "cash",
    "partial",
    "cash_receipt",
    "booking_only",
];

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct IncomingInvoice {
    pub id: i64,
    /// `invoice` | `proforma` (díjbekérő) | `receipt` (pénztárbizonylat).
    pub kind: String,
    pub supplier_name: String,
    pub supplier_tax_number: Option<String>,
    pub partner_id: Option<i64>,
    pub invoice_number: Option<String>,
    pub issue_date: Option<NaiveDate>,
    pub due_date: Option<NaiveDate>,
    pub currency: String,
    /// Minor units; null until read off the document.
    pub net_amount: Option<i64>,
    pub vat_amount: Option<i64>,
    pub gross_amount: Option<i64>,
    /// `TRANSFER` | `CASH` | `CARD`.
    pub payment_method: String,
    pub paid_amount: i64,
    pub paid_on: Option<NaiveDate>,
    /// Exists only in the books (Csak könyvelésben létező).
    pub booking_only: bool,
    pub notes: Option<String>,
    pub file_name: Option<String>,
    pub file_type: Option<String>,
    pub file_size: Option<i64>,
    /// The list it is on, from the figures.
    pub bucket: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Everything the office can edit.
pub struct Fields {
    pub kind: String,
    pub supplier_name: String,
    pub supplier_tax_number: Option<String>,
    pub partner_id: Option<i64>,
    pub invoice_number: Option<String>,
    pub issue_date: Option<NaiveDate>,
    pub due_date: Option<NaiveDate>,
    pub currency: String,
    pub net_amount: Option<i64>,
    pub vat_amount: Option<i64>,
    pub gross_amount: Option<i64>,
    pub payment_method: String,
    pub paid_amount: i64,
    pub paid_on: Option<NaiveDate>,
    pub booking_only: bool,
    pub notes: Option<String>,
}

impl From<&IncomingInvoice> for Fields {
    fn from(i: &IncomingInvoice) -> Self {
        Fields {
            kind: i.kind.clone(),
            supplier_name: i.supplier_name.clone(),
            supplier_tax_number: i.supplier_tax_number.clone(),
            partner_id: i.partner_id,
            invoice_number: i.invoice_number.clone(),
            issue_date: i.issue_date,
            due_date: i.due_date,
            currency: i.currency.clone(),
            net_amount: i.net_amount,
            vat_amount: i.vat_amount,
            gross_amount: i.gross_amount,
            payment_method: i.payment_method.clone(),
            paid_amount: i.paid_amount,
            paid_on: i.paid_on,
            booking_only: i.booking_only,
            notes: i.notes.clone(),
        }
    }
}

fn select() -> String {
    format!(
        "SELECT id, kind, supplier_name, supplier_tax_number, partner_id, invoice_number,
                issue_date, due_date, currency, net_amount, vat_amount, gross_amount,
                payment_method, paid_amount, paid_on, booking_only, notes,
                file_name, file_type, file_size, {BUCKET} AS bucket, created_at, updated_at
         FROM incoming_invoices"
    )
}

pub async fn find(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<IncomingInvoice>> {
    sqlx::query_as(&format!("{} WHERE id = $1 AND deleted_at IS NULL", select()))
        .bind(id)
        .fetch_optional(db)
        .await
}

/// Newest first. `pattern` is an ILIKE over supplier, number and file name.
pub async fn search(
    db: impl PgExecutor<'_>,
    pattern: Option<&str>,
    bucket: Option<&str>,
    limit: i64,
    offset: i64,
) -> sqlx::Result<Vec<IncomingInvoice>> {
    sqlx::query_as(&format!(
        "{} WHERE deleted_at IS NULL
           AND ($1::text IS NULL OR supplier_name ILIKE $1 OR invoice_number ILIKE $1
                OR file_name ILIKE $1 OR supplier_tax_number ILIKE $1)
           AND ($2::text IS NULL OR {BUCKET} = $2)
         ORDER BY coalesce(issue_date, created_at::date) DESC, id DESC
         LIMIT $3 OFFSET $4",
        select()
    ))
    .bind(pattern)
    .bind(bucket)
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct IncomingBucketCount {
    pub bucket: String,
    pub count: i64,
}

pub async fn bucket_counts(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<IncomingBucketCount>> {
    sqlx::query_as(&format!(
        "SELECT {BUCKET} AS bucket, count(*) AS count FROM incoming_invoices
         WHERE deleted_at IS NULL GROUP BY 1"
    ))
    .fetch_all(db)
    .await
}

pub struct NewFile<'a> {
    pub key: &'a str,
    pub name: &'a str,
    pub content_type: &'a str,
    pub size: i64,
    pub hash: &'a [u8],
}

/// A new incoming invoice holding only its document; the office fills in the rest.
pub async fn insert_with_file(
    db: impl PgExecutor<'_>,
    file: &NewFile<'_>,
    created_by: i64,
) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO incoming_invoices (file_key, file_name, file_type, file_size, file_hash, created_by)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(file.key)
    .bind(file.name)
    .bind(file.content_type)
    .bind(file.size)
    .bind(file.hash)
    .bind(created_by)
    .fetch_one(db)
    .await
}

/// The live row already holding this exact file, if any.
pub async fn by_hash(db: impl PgExecutor<'_>, hash: &[u8]) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar("SELECT id FROM incoming_invoices WHERE file_hash = $1 AND deleted_at IS NULL")
        .bind(hash)
        .fetch_optional(db)
        .await
}

pub async fn update(db: impl PgExecutor<'_>, id: i64, f: &Fields) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE incoming_invoices SET
             kind = $2, supplier_name = $3, supplier_tax_number = $4, partner_id = $5,
             invoice_number = $6, issue_date = $7, due_date = $8, currency = $9,
             net_amount = $10, vat_amount = $11, gross_amount = $12, payment_method = $13,
             paid_amount = $14, paid_on = $15, booking_only = $16, notes = $17
         WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(&f.kind)
    .bind(&f.supplier_name)
    .bind(&f.supplier_tax_number)
    .bind(f.partner_id)
    .bind(&f.invoice_number)
    .bind(f.issue_date)
    .bind(f.due_date)
    .bind(&f.currency)
    .bind(f.net_amount)
    .bind(f.vat_amount)
    .bind(f.gross_amount)
    .bind(&f.payment_method)
    .bind(f.paid_amount)
    .bind(f.paid_on)
    .bind(f.booking_only)
    .bind(&f.notes)
    .execute(db)
    .await?;
    Ok(())
}

/// The storage key of the row's document.
pub async fn file_key(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<Option<(String, String)>> {
    sqlx::query_as(
        "SELECT file_key, coalesce(file_name, 'szamla') FROM incoming_invoices
         WHERE id = $1 AND deleted_at IS NULL AND file_key IS NOT NULL",
    )
    .bind(id)
    .fetch_optional(db)
    .await
}

/// Kept, not deleted: an invoice that was booked must stay traceable.
pub async fn soft_delete(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let done = sqlx::query(
        "UPDATE incoming_invoices SET deleted_at = now() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Each supplier used before, with the tax number, payment method and currency of their
/// latest invoice: picking the name fills in the rest on the next one.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct KnownSupplier {
    pub supplier_name: String,
    pub supplier_tax_number: Option<String>,
    pub payment_method: String,
    pub currency: String,
}

pub async fn known_suppliers(db: impl PgExecutor<'_>) -> sqlx::Result<Vec<KnownSupplier>> {
    sqlx::query_as(
        "SELECT DISTINCT ON (lower(supplier_name)) supplier_name, supplier_tax_number,
                payment_method, currency
         FROM incoming_invoices WHERE deleted_at IS NULL AND supplier_name <> ''
         ORDER BY lower(supplier_name), updated_at DESC
         LIMIT 500",
    )
    .fetch_all(db)
    .await
}

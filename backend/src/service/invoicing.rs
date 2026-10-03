//! Invoicing: turning an order into a reported invoice, and the documents and letters
//! that follow from it.
//!
//! Reporting to NAV is asynchronous by nature — the tax authority validates in its own
//! time — so nothing here happens inside the request that asks for it. Issuing an invoice
//! writes a row in `submitting` and queues a job; the job talks to the sidecar, records
//! what NAV said, stores the PDF and queues the letter. The screen polls the row. That
//! keeps a slow tax authority off the request thread and out of the order workflow, and it
//! means a NAV outage delays an invoice rather than losing one.
//!
//! Proformas take the other path: a díjbekérő is reported nowhere, so rendering one is a
//! single fast call and is done inline.

use anyhow::anyhow;
use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;

use crate::AppState;
use crate::config::{Config, NavConfig};
use crate::domain::invoice::{
    InvoiceKind, InvoiceStatus, PaymentMethod, line_amounts, minor_to_decimal_string,
    split_address_line, validate_vat_rate,
};
use crate::domain::media::{DocumentKind, document_storage_key};
use crate::domain::money::{Currency, Money};
use crate::domain::partner::PartnerKind;
use crate::error::{AppError, AppResult};
use crate::integrations::nav::{self, NavError, NavSidecar};
use crate::repo::documents::{self, NewDocument, Owner};
use crate::repo::invoices::{self, Invoice, NewInvoice, NewLine, NewProforma, Proforma};
use crate::repo::orders::Order;
use crate::repo::partners::Partner;
use crate::repo::{audit, fx, jobs, order_items, orders, partners};
use crate::service::auth::AuthUser;
use crate::service::email::{About, AttachmentRef, AutomaticEmail, queue_automatic};
use crate::service::{automation, business_today};

pub mod triggers {
    pub const INVOICE_ISSUED: &str = "invoice_issued";
    pub const INVOICE_STORNOED: &str = "invoice_stornoed";
    pub const INVOICE_ANNULLED: &str = "invoice_annulled";
    pub const PROFORMA: &str = "proforma";
}

/// NAV's annulment codes. A technical annulment says the *report* was wrong, which is a
/// different claim from "the invoice was wrong" — that is a storno.
///
/// `pub(crate)`: the lookups endpoint publishes these to the clients.
pub(crate) const ANNULMENT_CODES: [&str; 4] = [
    "ERRATIC_DATA",
    "ERRATIC_INVOICE_NUMBER",
    "ERRATIC_INVOICE_ISSUE_DATE",
    "ERRATIC_ELECTRONIC_HASH_VALUE",
];

fn default_true() -> bool {
    true
}

/// What the caller may decide when issuing an invoice. Everything is optional: with an
/// order that already carries its line items, "issue the invoice" is a confirmation, not a
/// form.
#[derive(Debug, Clone, Default, Deserialize, utoipa::ToSchema)]
pub struct IssueRequest {
    /// The fraction, not the percentage: 0.27 is 27%. Defaults to `NAV_DEFAULT_VAT_RATE`.
    #[schema(value_type = Option<String>)]
    #[serde(default, deserialize_with = "optional_decimal")]
    pub vat_rate: Option<Decimal>,
    /// Defaults to today in the business timezone.
    pub issue_date: Option<NaiveDate>,
    /// Fulfilment date. Defaults to the issue date.
    pub delivery_date: Option<NaiveDate>,
    /// Defaults to the issue date plus `NAV_PAYMENT_DAYS`.
    pub payment_date: Option<NaiveDate>,
    /// `TRANSFER` (the default) or `CASH`. Anything else is refused before a number
    /// is drawn.
    pub payment_method: Option<String>,
    /// Send the customer the invoice once NAV has stored it.
    #[serde(default = "default_true")]
    pub send_email: bool,
}

#[derive(Debug, Clone, Default, Deserialize, utoipa::ToSchema)]
pub struct ProformaRequest {
    #[schema(value_type = Option<String>)]
    #[serde(default, deserialize_with = "optional_decimal")]
    pub vat_rate: Option<Decimal>,
    pub issue_date: Option<NaiveDate>,
    pub payment_date: Option<NaiveDate>,
    /// Printed under the totals, e.g. payment instructions.
    pub note: Option<String>,
    #[serde(default = "default_true")]
    pub send_email: bool,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct AnnulRequest {
    /// One of NAV's codes; `ERRATIC_DATA` is the usual one.
    pub code: String,
    /// Why. NAV stores it and shows it in the Online Számla portal.
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct StornoRequest {
    /// Defaults to today in the business timezone.
    pub issue_date: Option<NaiveDate>,
    #[serde(default = "default_true")]
    pub send_email: bool,
}

/// A rendered proforma and the document it was stored as.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ProformaCreated {
    #[serde(flatten)]
    pub proforma: Proforma,
    /// Queued letter, when one was asked for and the customer has an address.
    pub email_id: Option<i64>,
}

fn optional_decimal<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Decimal>, D::Error> {
    use serde::de::Error;
    match Option::<serde_json::Value>::deserialize(d)? {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) => s.trim().parse().map(Some).map_err(D::Error::custom),
        Some(serde_json::Value::Number(n)) => {
            n.to_string().parse().map(Some).map_err(D::Error::custom)
        }
        Some(_) => Err(D::Error::custom("expected a decimal number or string")),
    }
}

/// The sidecar, or a clear explanation of why there isn't one.
pub fn sidecar(config: &Config) -> AppResult<(NavSidecar, &NavConfig)> {
    let nav = config.nav.as_ref().ok_or_else(|| {
        AppError::rule(
            "invoicing_not_configured",
            "invoicing is not configured on this server: set NAV_SIDECAR_URL and the \
             NAV_SUPPLIER_* variables",
        )
    })?;
    let client = NavSidecar::new(nav).map_err(|e| AppError::internal(format!("{e}")))?;
    Ok((client, nav))
}

/// Us, from configuration.
fn supplier_party(nav: &NavConfig) -> nav::Supplier {
    nav::Supplier {
        name: nav.supplier.name.clone(),
        tax_number: nav.supplier.tax_number.clone(),
        bank_account: nav.supplier.bank_account.clone(),
        address: nav::Address {
            country_code: Some("HU".into()),
            postal_code: nav.supplier.postal_code.clone(),
            city: nav.supplier.city.clone(),
            street_name: nav.supplier.street_name.clone(),
            public_place_category: nav.supplier.public_place_category.clone(),
            number: nav.supplier.number.clone(),
        },
    }
}

/// The customer, from the partner record.
///
/// Every refusal here names the field to fix, because the person reading it is about to go
/// and fix it. An invoice with a made-up address is worse than an invoice that was not
/// issued: the second is a five-minute edit, the first is a defective document that has
/// already been reported.
fn customer_party(partner: &Partner) -> AppResult<nav::Customer> {
    let missing = |field: &str| {
        AppError::rule(
            "invoice_data_missing",
            format!(
                "the invoice cannot be issued: the partner '{}' has no {field}. \
                 Fill it in on the partner and try again.",
                partner.name
            ),
        )
    };

    let postal_code = partner
        .postal_code
        .clone()
        .ok_or_else(|| missing("postal code"))?;
    let city = partner.city.clone().ok_or_else(|| missing("city"))?;
    let address_line = partner
        .address_line
        .clone()
        .ok_or_else(|| missing("street address"))?;
    let (street_name, category, number) = split_address_line(&address_line).ok_or_else(|| {
        AppError::rule(
            "invoice_data_missing",
            format!(
                "the invoice cannot be issued: the address '{address_line}' of partner '{}' \
                 cannot be split into street, type and house number (expected something like \
                 'Kossuth Lajos utca 12.'). Correct it on the partner and try again.",
                partner.name
            ),
        )
    })?;

    let domestic = partner.country.eq_ignore_ascii_case("HU");
    // A foreign customer is reported as OTHER without a Hungarian tax number; sending an
    // EU VAT number in the Hungarian field is a rejection waiting to happen.
    let (tax_number, vat_status) = match (domestic, partner.kind) {
        (true, PartnerKind::Business) => (
            Some(
                partner
                    .tax_number
                    .clone()
                    .ok_or_else(|| missing("tax number"))?,
            ),
            "DOMESTIC",
        ),
        (true, PartnerKind::Person) => (None, "PRIVATE_PERSON"),
        (false, _) => (None, "OTHER"),
    };

    Ok(nav::Customer {
        name: partner.name.clone(),
        tax_number,
        vat_status: Some(vat_status.into()),
        address: nav::Address {
            country_code: Some(partner.country.to_uppercase()),
            postal_code,
            city,
            street_name,
            public_place_category: category,
            number,
        },
    })
}

/// The order's line items, priced and taxed, as both our snapshot and the sidecar's lines.
struct Snapshot {
    lines: Vec<(NewLineOwned, nav::Line)>,
    net: i64,
    vat: i64,
}

/// Owned twin of `repo::invoices::NewLine`, so the snapshot can outlive the borrow of the
/// order items it was built from.
struct NewLineOwned {
    position: i32,
    description: String,
    quantity: Decimal,
    unit: String,
    unit_price: i64,
    vat_rate: Decimal,
    net_amount: i64,
    vat_amount: i64,
}

fn snapshot_lines(
    items: &[order_items::OrderItem],
    currency: Currency,
    vat_rate: Decimal,
    sign: i64,
) -> AppResult<Snapshot> {
    let mut lines = Vec::with_capacity(items.len());
    let mut net_total: i64 = 0;
    let mut vat_total: i64 = 0;

    for (index, item) in items.iter().enumerate() {
        let unit_price = Money::new(item.unit_price * sign, currency);
        let (net, vat) = line_amounts(unit_price, item.quantity, vat_rate)
            .map_err(|e| AppError::internal(format!("line {}: {e}", index + 1)))?;
        net_total = net_total
            .checked_add(net.minor())
            .ok_or_else(|| AppError::internal("invoice net total overflows"))?;
        vat_total = vat_total
            .checked_add(vat.minor())
            .ok_or_else(|| AppError::internal("invoice VAT total overflows"))?;

        let position = i32::try_from(index + 1).unwrap_or(i32::MAX);
        lines.push((
            NewLineOwned {
                position,
                description: item.description.clone(),
                quantity: item.quantity,
                unit: "PIECE".into(),
                unit_price: unit_price.minor(),
                vat_rate,
                net_amount: net.minor(),
                vat_amount: vat.minor(),
            },
            nav::Line {
                description: item.description.clone(),
                quantity: item.quantity.normalize().to_string(),
                unit: Some("PIECE".into()),
                unit_price: minor_to_decimal_string(unit_price.minor(), currency),
                vat_percentage: vat_rate.normalize().to_string(),
                nature: None,
            },
        ));
    }

    Ok(Snapshot {
        lines,
        net: net_total,
        vat: vat_total,
    })
}

/// The rate to HUF that NAV requires on a non-HUF invoice.
async fn exchange_rate(
    conn: &mut PgConnection,
    currency: Currency,
    day: NaiveDate,
) -> AppResult<Option<String>> {
    if currency == Currency::HUF {
        return Ok(None);
    }
    let rate = fx::rate_on_or_before(&mut *conn, currency.code(), day)
        .await?
        .ok_or_else(|| {
            AppError::rule(
                "fx_rate_missing",
                format!(
                    "no MNB rate is stored for {} on or before {day}; fetch the rates and \
                     try again",
                    currency.code()
                ),
            )
        })?;
    Ok(Some(rate.rate.normalize().to_string()))
}

/// Issue an invoice for an order: snapshot, number, row, job.
///
/// Refuses rather than duplicating. One invoice may be in flight at a time, and an order
/// that already has a live invoice must have it stornoed before another is issued —
/// reporting the same job twice is not an accident that fixes itself.
pub async fn create_invoice(
    state: &AppState,
    user: &AuthUser,
    order_id: i64,
    req: &IssueRequest,
) -> AppResult<Invoice> {
    let (_, nav) = sidecar(&state.config)?;
    let vat_rate = match req.vat_rate {
        Some(rate) => validate_vat_rate(rate).map_err(AppError::validation)?,
        None => nav.default_vat_rate,
    };
    // Closed set, validated here: the sidecar would refuse anything else after the
    // number is drawn, spending it. Absent means a bank transfer.
    let payment_method = match &req.payment_method {
        None => PaymentMethod::DEFAULT,
        Some(raw) => PaymentMethod::parse(raw).map_err(|e| AppError::validation(e.to_string()))?,
    };
    let today = business_today(state.config.business_tz);
    let issue_date = req.issue_date.unwrap_or(today);
    let delivery_date = req.delivery_date.unwrap_or(issue_date);
    let payment_date = req
        .payment_date
        .unwrap_or(issue_date + chrono::TimeDelta::days(nav.payment_days));

    let mut tx = state.db.begin().await?;
    // Locked, not just read: the one-live-invoice check below is only a guarantee if two
    // concurrent requests for the same order cannot both pass it before either inserts.
    let order = orders::lock(&mut *tx, order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let currency = crate::service::orders::order_currency(&order)?;

    for existing in invoices::list_for_order(&mut *tx, order_id).await? {
        match existing.status {
            InvoiceStatus::Submitting => {
                return Err(AppError::conflict(
                    "invoice_in_flight",
                    format!(
                        "invoice {} is still being reported to NAV; wait for the verdict",
                        existing.number
                    ),
                ));
            }
            InvoiceStatus::Issued if existing.kind == InvoiceKind::Invoice => {
                return Err(AppError::conflict(
                    "invoice_exists",
                    format!(
                        "this order already has a live invoice ({}); storno it before \
                         issuing another",
                        existing.number
                    ),
                ));
            }
            _ => {}
        }
    }

    let items = order_items::list(&mut *tx, order_id).await?;
    if items.is_empty() {
        return Err(AppError::rule(
            "no_items",
            "an invoice needs at least one line item on the order",
        ));
    }
    let snapshot = snapshot_lines(&items, currency, vat_rate, 1)?;
    // Both sides of the boundary are checked: the sidecar validates the document, and we
    // check what only we know — that this partner can be invoiced at all.
    let partner = partners::find(&mut *tx, order.partner_id)
        .await?
        .ok_or(AppError::NotFound("partner"))?;
    customer_party(&partner)?;
    exchange_rate(&mut tx, currency, issue_date).await?;

    let number = invoices::next_number(&mut tx, &nav.invoice_prefix, issue_date.year()).await?;
    let invoice = invoices::insert(
        &mut tx,
        &NewInvoice {
            order_id,
            number: &number,
            kind: InvoiceKind::Invoice,
            original_invoice_id: None,
            currency: currency.code(),
            issue_date,
            delivery_date,
            payment_date: Some(payment_date),
            payment_method: payment_method.as_str(),
            net_amount: snapshot.net,
            vat_amount: snapshot.vat,
            gross_amount: snapshot.net + snapshot.vat,
            created_by: Some(user.user_id),
        },
    )
    .await?;
    for (line, _) in &snapshot.lines {
        insert_line(&mut tx, invoice.id, line).await?;
    }

    audit::record(
        &mut *tx,
        Some(user.user_id),
        "order",
        order_id,
        "invoice_issue",
        json!({
            "invoice_id": invoice.id,
            "number": invoice.number,
            "gross": invoice.gross_amount,
            "currency": invoice.currency,
        }),
    )
    .await?;
    enqueue_submit(
        &mut tx,
        invoice.id,
        req.send_email,
        req.payment_method.clone(),
    )
    .await?;
    tx.commit().await?;

    tracing::info!(invoice_id = invoice.id, number = %invoice.number, order_id, "invoice queued for reporting");
    Ok(invoice)
}

async fn insert_line(
    conn: &mut PgConnection,
    invoice_id: i64,
    line: &NewLineOwned,
) -> sqlx::Result<()> {
    invoices::insert_line(
        conn,
        invoice_id,
        &NewLine {
            position: line.position,
            description: &line.description,
            quantity: line.quantity,
            unit: &line.unit,
            unit_price: line.unit_price,
            vat_rate: line.vat_rate,
            net_amount: line.net_amount,
            vat_amount: line.vat_amount,
        },
    )
    .await
}

async fn enqueue_submit(
    conn: &mut PgConnection,
    invoice_id: i64,
    send_email: bool,
    payment_method: Option<String>,
) -> sqlx::Result<()> {
    jobs::enqueue(
        conn,
        crate::jobs::kinds::NAV_SUBMIT_INVOICE,
        json!({
            "invoice_id": invoice_id,
            "send_email": send_email,
            "payment_method": payment_method,
        }),
        None,
        // One submission per invoice, ever: the dedupe key is the invoice itself.
        Some(&format!("nav_submit:{invoice_id}")),
    )
    .await?;
    Ok(())
}

/// Whether `candidate` is a storno of invoice `original_id` that still counts: one being
/// reported, or one NAV stored. A rejected attempt was never reported and does not count,
/// the same rule as the `invoices_one_storno_per_invoice` index.
fn blocks_another_storno(
    kind: InvoiceKind,
    status: InvoiceStatus,
    original_invoice_id: Option<i64>,
    original_id: i64,
) -> bool {
    kind == InvoiceKind::Storno
        && original_invoice_id == Some(original_id)
        && status != InvoiceStatus::Rejected
}

/// Storno an issued invoice: a second document that reverses the first.
pub async fn create_storno(
    state: &AppState,
    user: &AuthUser,
    invoice_id: i64,
    req: &StornoRequest,
) -> AppResult<Invoice> {
    let (_, nav) = sidecar(&state.config)?;
    let today = business_today(state.config.business_tz);
    let issue_date = req.issue_date.unwrap_or(today);

    let mut tx = state.db.begin().await?;
    let original = invoices::lock(&mut tx, invoice_id)
        .await?
        .ok_or(AppError::NotFound("invoice"))?;
    if original.kind != InvoiceKind::Invoice {
        return Err(AppError::rule(
            "not_stornoable",
            "a storno cannot itself be stornoed",
        ));
    }
    if original.status != InvoiceStatus::Issued {
        return Err(AppError::rule(
            "not_stornoable",
            format!(
                "only an issued invoice can be stornoed; {} is {}",
                original.number,
                original.status.as_str()
            ),
        ));
    }

    // The original stays `issued` until NAV stores its storno, so the status alone does not
    // say whether a storno is already on its way. Refuse by name rather than drawing a
    // number and tripping over `invoices_one_storno_per_invoice`.
    let siblings = invoices::list_for_order(&mut *tx, original.order_id).await?;
    if let Some(existing) = siblings
        .iter()
        .find(|i| blocks_another_storno(i.kind, i.status, i.original_invoice_id, original.id))
    {
        return Err(AppError::rule(
            "not_stornoable",
            format!(
                "a storno of {} already exists ({}, {})",
                original.number,
                existing.number,
                existing.status.as_str()
            ),
        ));
    }

    let lines = invoices::lines_for(&mut *tx, original.id).await?;
    let number = invoices::next_number(&mut tx, &nav.invoice_prefix, issue_date.year()).await?;
    let storno = invoices::insert(
        &mut tx,
        &NewInvoice {
            order_id: original.order_id,
            number: &number,
            kind: InvoiceKind::Storno,
            original_invoice_id: Some(original.id),
            currency: &original.currency,
            issue_date,
            delivery_date: original.delivery_date,
            payment_date: original.payment_date,
            payment_method: &original.payment_method,
            // The reversal, so the figures are the original's negated. NAV builds the same
            // document from the original it holds; storing it this way means our own books
            // add up without special-casing the sign at every reading.
            net_amount: -original.net_amount,
            vat_amount: -original.vat_amount,
            gross_amount: -original.gross_amount,
            created_by: Some(user.user_id),
        },
    )
    .await?;
    for line in &lines {
        invoices::insert_line(
            &mut tx,
            storno.id,
            &NewLine {
                position: line.position,
                description: &line.description,
                quantity: line.quantity,
                unit: &line.unit,
                unit_price: -line.unit_price,
                vat_rate: line.vat_rate,
                net_amount: -line.net_amount,
                vat_amount: -line.vat_amount,
            },
        )
        .await?;
    }

    audit::record(
        &mut *tx,
        Some(user.user_id),
        "order",
        original.order_id,
        "invoice_storno",
        json!({
            "invoice_id": storno.id,
            "number": storno.number,
            "original_invoice_id": original.id,
            "original_number": original.number,
        }),
    )
    .await?;
    enqueue_submit(&mut tx, storno.id, req.send_email, None).await?;
    tx.commit().await?;

    tracing::info!(invoice_id = storno.id, number = %storno.number, original = %original.number, "storno queued for reporting");
    Ok(storno)
}

/// Technically annul a data report. Admin-only, and irreversible from here: NAV's own
/// approval happens in the Online Számla portal afterwards.
pub async fn annul_invoice(
    state: &AppState,
    user: &AuthUser,
    invoice_id: i64,
    req: &AnnulRequest,
) -> AppResult<Invoice> {
    sidecar(&state.config)?;
    if !ANNULMENT_CODES.contains(&req.code.as_str()) {
        return Err(AppError::validation(format!(
            "annulment code must be one of {}",
            ANNULMENT_CODES.join(", ")
        )));
    }
    let reason = crate::api::required("reason", &req.reason)?;

    let mut tx = state.db.begin().await?;
    let invoice = invoices::lock(&mut tx, invoice_id)
        .await?
        .ok_or(AppError::NotFound("invoice"))?;
    if !matches!(
        invoice.status,
        InvoiceStatus::Issued | InvoiceStatus::Stornoed
    ) {
        return Err(AppError::rule(
            "not_annullable",
            format!(
                "only a reported invoice can be technically annulled; {} is {}",
                invoice.number,
                invoice.status.as_str()
            ),
        ));
    }

    audit::record(
        &mut *tx,
        Some(user.user_id),
        "order",
        invoice.order_id,
        "invoice_annul",
        json!({ "invoice_id": invoice.id, "number": invoice.number, "code": req.code, "reason": reason }),
    )
    .await?;
    jobs::enqueue(
        &mut *tx,
        crate::jobs::kinds::NAV_ANNUL_INVOICE,
        json!({
            "invoice_id": invoice.id,
            "code": req.code,
            "reason": reason,
            "send_email": true,
        }),
        None,
        Some(&format!("nav_annul:{}", invoice.id)),
    )
    .await?;
    tx.commit().await?;
    Ok(invoice)
}

// ── The jobs ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SubmitPayload {
    pub invoice_id: i64,
    #[serde(default = "default_true")]
    pub send_email: bool,
    #[serde(default)]
    pub payment_method: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AnnulPayload {
    pub invoice_id: i64,
    pub code: String,
    pub reason: String,
    #[serde(default = "default_true")]
    pub send_email: bool,
}

/// Reports one invoice or storno to NAV, then stores the PDF and queues the letter.
///
/// Returning `Ok` when NAV *rejects* the document is deliberate: a rejection is an answer,
/// it is recorded on the row with NAV's own fault code, and repeating the request would
/// only produce the same answer. Only a failure to get an answer at all is an error, and
/// the queue retries that with backoff.
pub async fn submit_invoice(state: &AppState, payload: &SubmitPayload) -> anyhow::Result<()> {
    let (client, nav) = sidecar(&state.config)?;
    let Some(invoice) = invoices::find(&state.db, payload.invoice_id).await? else {
        tracing::warn!(
            invoice_id = payload.invoice_id,
            "invoice vanished before it was reported"
        );
        return Ok(());
    };
    if invoice.status != InvoiceStatus::Submitting {
        // Already decided: a retry of an already-finished job, or a human intervened.
        return Ok(());
    }

    let outcome =
        match invoice.kind {
            InvoiceKind::Invoice => {
                let request =
                    match build_request(state, nav, &invoice, payload.payment_method.as_deref())
                        .await
                    {
                        Ok(request) => request,
                        Err(e) => {
                            // Unbuildable, not unreported: the partner may be fixed or the FX
                            // may arrive while the queue retries, but when it gives up the row
                            // must reach a terminal state (INV-L10). The marker tells the
                            // dead-letter branch what this is.
                            let message = format!("building the invoice: {e}");
                            note_attempt(state, invoice.id, &message).await?;
                            return Err(anyhow!("{UNBUILDABLE_MARKER}{message}"));
                        }
                    };
                client.create_invoice(&request).await
            }
            InvoiceKind::Storno => {
                let original_id = invoice
                    .original_invoice_id
                    .ok_or_else(|| anyhow!("storno {} has no original", invoice.number))?;
                let original = invoices::find(&state.db, original_id)
                    .await?
                    .ok_or_else(|| {
                        anyhow!("the invoice storno {} cancels is gone", invoice.number)
                    })?;
                client
                    .storno(
                        &original.number,
                        &nav::StornoRequest {
                            storno_invoice_number: invoice.number.clone(),
                            issue_date: Some(invoice.issue_date),
                        },
                    )
                    .await
            }
        };

    match outcome {
        Ok(response) => {
            record_success(state, &invoice, &response, payload.send_email).await?;
            Ok(())
        }
        Err(error) if error.is_retryable() => {
            // The report may or may not have reached NAV, so the row stays `submitting`:
            // saying "rejected" here would be a guess, and the wrong one costs an invoice.
            note_attempt(state, invoice.id, &error.to_string()).await?;
            Err(anyhow!("{error}"))
        }
        Err(error) => {
            // A duplicate-number answer after a timeout is not a rejection of our
            // document: the first attempt may have been stored. Read back what NAV
            // holds before deciding (INV-L8). A storno is a numbered document like any
            // other and is re-filed the same way on a retry, so it is reconciled the
            // same way: otherwise NAV holds the reversal while our books say the
            // original is still live, and the office is free to storno it twice.
            let mut reconcile_note: Option<String> = None;
            if error.nav_error_code() == Some("INVOICE_NUMBER_ALREADY_EXISTS") {
                match try_adopt(state, &invoice, payload.send_email).await {
                    Ok(Adopt::Yes) => return Ok(()),
                    Ok(Adopt::No(note)) => reconcile_note = Some(note),
                    // The read-back is itself of unknown outcome: stay `submitting`
                    // and retry later, like any other unreachable error.
                    Err(e) => return Err(e),
                }
            }
            let fault = error.fault();
            let message = match reconcile_note {
                Some(note) => format!("{error} ({note})"),
                None => error.to_string(),
            };
            let mut tx = state.db.begin().await?;
            invoices::mark_rejected(
                &mut tx,
                invoice.id,
                error.nav_error_code(),
                &message,
                error.messages(),
                fault.and_then(|f| f.transaction_id.as_deref()),
            )
            .await?;
            audit::record(
                &mut *tx,
                None,
                "order",
                invoice.order_id,
                "invoice_rejected",
                json!({
                    "invoice_id": invoice.id,
                    "number": invoice.number,
                    "nav_error_code": error.nav_error_code(),
                    "message": message,
                }),
            )
            .await?;
            tx.commit().await?;
            tracing::warn!(
                invoice_id = invoice.id,
                number = %invoice.number,
                code = error.nav_error_code().unwrap_or("-"),
                "NAV rejected the report"
            );
            Ok(())
        }
    }
}

/// Records a failed attempt without changing the status.
///
/// The screen shows this while the queue keeps trying, so "nothing is happening" is never
/// the whole story a user gets.
async fn note_attempt(state: &AppState, invoice_id: i64, message: &str) -> anyhow::Result<()> {
    sqlx::query!(
        "UPDATE invoices SET nav_message = $2 WHERE id = $1 AND status = 'submitting'",
        invoice_id,
        message
    )
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn record_success(
    state: &AppState,
    invoice: &Invoice,
    response: &nav::SubmissionResponse,
    send_email: bool,
) -> anyhow::Result<()> {
    let mut tx = state.db.begin().await?;
    let updated = invoices::mark_issued(
        &mut tx,
        invoice.id,
        &response.transaction_id,
        &response.status,
        &response.messages,
    )
    .await?
    .ok_or_else(|| anyhow!("invoice {} vanished while being recorded", invoice.id))?;
    if let Some(original_id) = invoice.original_invoice_id {
        invoices::mark_stornoed(&mut tx, original_id).await?;
    }
    tx.commit().await?;

    // The PDF is a separate concern from the report: NAV has the data either way, and an
    // object store that is briefly unavailable must not turn a stored invoice into a
    // failed one. A missing PDF is visible on the row and can be fetched again.
    finish_issuance(state, &updated, send_email).await?;
    tracing::info!(
        invoice_id = updated.id,
        number = %updated.number,
        transaction_id = %response.transaction_id,
        "NAV stored the report"
    );
    Ok(())
}

/// What follows every stored report — fresh or reconciled: the PDF and the customer
/// letter. Both are best effort; neither changes the stored status.
async fn finish_issuance(
    state: &AppState,
    updated: &Invoice,
    send_email: bool,
) -> anyhow::Result<()> {
    let document_id = match store_invoice_pdf(state, updated).await {
        Ok(id) => Some(id),
        Err(e) => {
            tracing::error!(invoice_id = updated.id, error = %e, "the invoice was reported but its PDF could not be stored");
            None
        }
    };

    if send_email {
        if let Err(e) = queue_invoice_email(state, updated, document_id).await {
            tracing::error!(invoice_id = updated.id, error = %e, "the invoice was reported but its letter could not be queued");
        }
    }
    Ok(())
}

/// The outcome of reading back a duplicate number (INV-L8).
enum Adopt {
    /// NAV holds exactly our document: adopt it, file nothing new.
    Yes,
    /// Not ours (or nothing there): the human-readable reason, kept on the row so the
    /// rejection that follows says why adoption was refused.
    No(String),
}

/// A retry answered `INVOICE_NUMBER_ALREADY_EXISTS` after a timeout: read back what
/// NAV holds under the number and adopt it when it is our document.
///
/// `Ok(Yes)` adopted and finished (PDF, letter) like any stored report. `Ok(No)` means
/// the row should be rejected with the reason attached. `Err` means the read-back is
/// itself of unknown outcome (unreachable sidecar): the row stays `submitting` and the
/// job retries, because guessing here is exactly the bug being fixed.
async fn try_adopt(state: &AppState, invoice: &Invoice, send_email: bool) -> anyhow::Result<Adopt> {
    let (client, _) = sidecar(&state.config)?;
    let fetched = match client.fetch_invoice(&invoice.number).await {
        Ok(fetched) => fetched,
        Err(error) if error.is_retryable() => {
            note_attempt(state, invoice.id, &error.to_string()).await?;
            return Err(anyhow!("{error}"));
        }
        Err(error) => {
            return Ok(Adopt::No(format!(
                "NAV holds nothing under {} to adopt ({error})",
                invoice.number
            )));
        }
    };
    match reconcile_decision(invoice, &fetched) {
        Ok(()) => {
            adopt_reconciled(state, invoice, send_email).await?;
            Ok(Adopt::Yes)
        }
        Err(reason) => Ok(Adopt::No(format!(
            "NAV holds a different document under {}: {reason}; not adopted",
            invoice.number
        ))),
    }
}

/// Pure identity check between the stored row and what NAV holds: the number, the
/// issue date and the totals in the stored currency. Amounts are compared as minor
/// units, never as floats. Unit-tested below.
fn reconcile_decision(stored: &Invoice, fetched: &nav::FetchedInvoice) -> Result<(), String> {
    if fetched.invoice_number != stored.number {
        return Err(format!(
            "read-back names {}, not {}",
            fetched.invoice_number, stored.number
        ));
    }
    let held_date: NaiveDate = fetched.issue_date.parse().map_err(|_| {
        format!(
            "NAV reports an unreadable issue date: {}",
            fetched.issue_date
        )
    })?;
    if held_date != stored.issue_date {
        return Err(format!(
            "issue date {held_date}, ours is {}",
            stored.issue_date
        ));
    }
    let Some(totals) = &fetched.totals else {
        return Err("NAV reports no totals for this number".to_string());
    };
    if totals.currency != stored.currency {
        return Err(format!(
            "currency {}, ours is {}",
            totals.currency, stored.currency
        ));
    }
    for (label, held, ours) in [
        ("net", totals.net.as_str(), stored.net_amount),
        ("vat", totals.vat.as_str(), stored.vat_amount),
        ("gross", totals.gross.as_str(), stored.gross_amount),
    ] {
        let held_minor = nav_decimal_to_minor(held)
            .ok_or_else(|| format!("unreadable {label} total: {held}"))?;
        if held_minor != ours {
            return Err(format!(
                "{label} total {held} {}, ours is {} {}",
                totals.currency,
                minor_to_decimal_string(
                    ours,
                    stored.currency.parse().map_err(|_| format!(
                        "stored currency is not a known currency: {}",
                        stored.currency
                    ))?
                ),
                stored.currency
            ));
        }
    }
    Ok(())
}

/// A NAV decimal total ("1270000.00") as minor units. More than two fraction digits is
/// not a money amount we could have sent: refuse rather than round.
fn nav_decimal_to_minor(value: &str) -> Option<i64> {
    let amount: Decimal = value.trim().parse().ok()?;
    if amount.scale() > 2 {
        return None;
    }
    (amount * Decimal::new(100, 0)).try_into().ok()
}

/// Prefix on a submit-job failure that means the request could not be built (INV-L10).
///
/// The partner may be fixed or the FX may arrive while the queue retries, but when the
/// queue gives up, the dead-letter branch must recognise this failure and move the row
/// to a terminal state instead of leaving it `submitting` forever.
const UNBUILDABLE_MARKER: &str = "invoice_unbuildable: ";

/// Whether a dead-lettered submit job failed because the request could not be built.
/// Same crate as the worker, so the branch in `jobs::run_one` can ask.
pub(crate) fn is_unbuildable_dead_letter(message: &str) -> bool {
    message.starts_with(UNBUILDABLE_MARKER)
}

/// Called by the worker when a submit job that could never build its request runs out
/// of attempts (INV-L10).
///
/// The row becomes `rejected` with a local `unbuildable` code: nothing was ever sent
/// to NAV, the message says so (the number is still spent — a corrected invoice is a
/// new document, like any other rejection), and the order is free for a new invoice.
/// Without this the row stays `submitting` forever, the screen polls it forever, and
/// no invoice can ever be issued for the order again.
pub async fn job_dead_lettered_unbuildable(
    state: &AppState,
    invoice_id: i64,
    error: &str,
) -> anyhow::Result<()> {
    let mut tx = state.db.begin().await?;
    let Some(invoice) = invoices::lock(&mut tx, invoice_id).await? else {
        return Ok(());
    };
    if invoice.status != InvoiceStatus::Submitting {
        // Decided meanwhile (a human intervened, or a late retry built it after all).
        return Ok(());
    }
    let cause = error.strip_prefix(UNBUILDABLE_MARKER).unwrap_or(error);
    let message = format!(
        "the invoice could not be built for reporting ({cause}); nothing was sent to NAV, but the number is spent: fix the data and issue a new invoice"
    );
    invoices::mark_rejected(
        &mut tx,
        invoice.id,
        Some("unbuildable"),
        &message,
        &[],
        None,
    )
    .await?;
    audit::record(
        &mut *tx,
        None,
        "order",
        invoice.order_id,
        "invoice_rejected",
        json!({
            "invoice_id": invoice.id,
            "number": invoice.number,
            "nav_error_code": "unbuildable",
            "message": message,
        }),
    )
    .await?;
    tx.commit().await?;
    tracing::warn!(
        invoice_id,
        "submit job dead-lettered an unbuildable invoice; marked rejected"
    );
    Ok(())
}

/// Adopts the stored report NAV already holds: same `issued` a fresh success produces,
/// minus the transaction id the timed-out attempt never gave us. Then the PDF and the
/// letter, like any stored report.
async fn adopt_reconciled(
    state: &AppState,
    invoice: &Invoice,
    send_email: bool,
) -> anyhow::Result<()> {
    let message = format!(
        "NAV already held {} with identical totals (reconciled after INVOICE_NUMBER_ALREADY_EXISTS); adopted without a new report. The first attempt's transaction id is unknown.",
        invoice.number
    );
    let mut tx = state.db.begin().await?;
    invoices::mark_issued_reconciled(&mut tx, invoice.id, &message).await?;
    // Same as a fresh success: a stored storno means the original is reversed.
    if let Some(original_id) = invoice.original_invoice_id {
        invoices::mark_stornoed(&mut tx, original_id).await?;
    }
    audit::record(
        &mut *tx,
        None,
        "order",
        invoice.order_id,
        "invoice_reconciled",
        json!({ "invoice_id": invoice.id, "number": invoice.number }),
    )
    .await?;
    tx.commit().await?;
    let updated = invoices::find(&state.db, invoice.id)
        .await?
        .ok_or_else(|| anyhow!("invoice {} vanished while being adopted", invoice.id))?;
    finish_issuance(state, &updated, send_email).await?;
    tracing::info!(
        invoice_id = updated.id,
        number = %updated.number,
        "NAV already held the report; adopted it"
    );
    Ok(())
}

/// Fetches the PDF from the sidecar — which renders it from what NAV holds — and files it
/// with the order's other documents.
pub async fn store_invoice_pdf(state: &AppState, invoice: &Invoice) -> anyhow::Result<i64> {
    let (client, _) = sidecar(&state.config)?;
    let pdf = client.invoice_pdf(&invoice.number).await?;
    let filename = format!("{}.pdf", invoice.number);
    let document_id = store_pdf(
        state,
        invoice.order_id,
        DocumentKind::Invoice,
        &filename,
        pdf,
        Some(&invoice.number),
    )
    .await?;
    let mut conn = state.db.acquire().await?;
    invoices::set_document(&mut conn, invoice.id, document_id).await?;
    Ok(document_id)
}

/// Fetches the PDF of an issued invoice again (INV-L9).
///
/// The report is already stored at NAV; this only re-renders it from what NAV holds
/// and files it with the order's documents. Refused when there is nothing to fetch
/// (the invoice is not issued) or nothing missing (the PDF is already filed).
pub async fn refetch_pdf(state: &AppState, id: i64) -> AppResult<Invoice> {
    let invoice = invoices::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("invoice"))?;
    if invoice.status != InvoiceStatus::Issued {
        return Err(AppError::rule(
            "not_issued",
            format!(
                "only an issued invoice has a report to render; {} is {}",
                invoice.number,
                invoice.status.as_str()
            ),
        ));
    }
    if invoice.document_id.is_some() {
        return Err(AppError::conflict(
            "duplicate",
            format!("invoice {} already has its PDF", invoice.number),
        ));
    }
    let _document_id = store_invoice_pdf(state, &invoice).await.map_err(|e| {
        AppError::rule(
            "pdf_unavailable",
            format!("the PDF could not be fetched again: {e:#}"),
        )
    })?;
    invoices::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("invoice"))
}

/// Stores a PDF the sidecar produced as a document of the order.
async fn store_pdf(
    state: &AppState,
    order_id: i64,
    kind: DocumentKind,
    filename: &str,
    bytes: Vec<u8>,
    source_ref: Option<&str>,
) -> anyhow::Result<i64> {
    let hash = Sha256::digest(&bytes);
    let hash_hex = hex::encode(hash);
    let byte_size = i64::try_from(bytes.len())?;
    let key = document_storage_key(order_id, &hash_hex, "pdf");
    state
        .storage
        .put_bytes(&key, bytes, "application/pdf")
        .await?;

    let mut conn = state.db.acquire().await?;
    let (document, _) = documents::insert(
        &mut conn,
        &NewDocument {
            owner: Owner::Order(order_id),
            vehicle_id: None,
            kind,
            filename,
            content_type: "application/pdf",
            storage_key: &key,
            content_hash: hash.as_slice(),
            byte_size,
            // Generated, not uploaded: nobody chose this file.
            uploaded_by: None,
            source_ref,
        },
    )
    .await?;
    Ok(document.id)
}

/// Annuls a report, then tells the customer.
/// Files one technical annulment. `retried` is true from the job's second attempt on.
///
/// An attempt that timed out may still have reached NAV, and the sidecar keeps nothing,
/// so a retry files the annulment again. NAV's refusal of that second filing is then not
/// a verdict on the first one: the row says so, and points at the Online Számla portal,
/// where the pending annulment (if the first attempt landed) waits for approval anyway.
pub async fn annul_job(
    state: &AppState,
    payload: &AnnulPayload,
    retried: bool,
) -> anyhow::Result<()> {
    let (client, _) = sidecar(&state.config)?;
    let Some(invoice) = invoices::find(&state.db, payload.invoice_id).await? else {
        return Ok(());
    };
    if invoice.status == InvoiceStatus::Annulled {
        return Ok(());
    }

    let response = match client
        .annul(
            &invoice.number,
            &nav::AnnulRequest {
                code: payload.code.clone(),
                reason: payload.reason.clone(),
            },
        )
        .await
    {
        Ok(response) => response,
        Err(error) if error.is_retryable() => {
            // `note_attempt` only writes to a row still `submitting`, and an invoice being
            // annulled is `issued` or `stornoed`: record the attempt on it directly, so the
            // screen can say why the annulment has not happened yet.
            sqlx::query(
                "UPDATE invoices SET nav_message = $2 WHERE id = $1 AND status <> 'annulled'",
            )
            .bind(invoice.id)
            .bind(format!("annulment not yet filed: {error}"))
            .execute(&state.db)
            .await?;
            return Err(anyhow!("{error}"));
        }
        Err(error) => {
            // NAV refused the annulment itself. The invoice keeps its status — it is still
            // whatever it was — and the refusal is recorded where the screen can show it.
            let message = if retried {
                format!(
                    "NAV refused this annulment attempt ({error}), but an earlier attempt timed out and may already have been filed: check the invoice in the Online Számla portal before annulling again"
                )
            } else {
                error.to_string()
            };
            sqlx::query!(
                "UPDATE invoices SET nav_error_code = $2, nav_message = $3 WHERE id = $1",
                invoice.id,
                error.nav_error_code(),
                message
            )
            .execute(&state.db)
            .await?;
            tracing::warn!(invoice_id = invoice.id, error = %error, "NAV refused the annulment");
            return Ok(());
        }
    };

    let mut tx = state.db.begin().await?;
    let updated = invoices::mark_annulled(
        &mut tx,
        invoice.id,
        &response.transaction_id,
        &payload.code,
        &payload.reason,
    )
    .await?
    .ok_or_else(|| anyhow!("invoice {} vanished while being annulled", invoice.id))?;
    audit::record(
        &mut *tx,
        None,
        "order",
        invoice.order_id,
        "invoice_annulled",
        json!({
            "invoice_id": invoice.id,
            "number": invoice.number,
            "transaction_id": response.transaction_id,
            "code": payload.code,
        }),
    )
    .await?;
    tx.commit().await?;

    if payload.send_email {
        if let Err(e) = queue_invoice_email(state, &updated, None).await {
            tracing::error!(invoice_id = updated.id, error = %e, "the annulment letter could not be queued");
        }
    }
    Ok(())
}

// ── Proformas ───────────────────────────────────────────────────────────────

/// Renders a proforma (díjbekérő) and files it with the order's documents.
///
/// Nothing here touches NAV, an invoice status, or a transaction: a proforma is a request
/// for payment, reported nowhere, and usable long before an invoice exists. It is done
/// inline rather than queued because there is no tax authority to wait for.
pub async fn create_proforma(
    state: &AppState,
    user: &AuthUser,
    order_id: i64,
    req: &ProformaRequest,
) -> AppResult<ProformaCreated> {
    let (client, nav) = sidecar(&state.config)?;
    let vat_rate = match req.vat_rate {
        Some(rate) => validate_vat_rate(rate).map_err(AppError::validation)?,
        None => nav.default_vat_rate,
    };
    let today = business_today(state.config.business_tz);
    let issue_date = req.issue_date.unwrap_or(today);
    let payment_date = req
        .payment_date
        .unwrap_or(issue_date + chrono::TimeDelta::days(nav.payment_days));

    let mut conn = state.db.acquire().await?;
    let order = orders::find(&mut *conn, order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let currency = crate::service::orders::order_currency(&order)?;
    let items = order_items::list(&mut *conn, order_id).await?;
    if items.is_empty() {
        return Err(AppError::rule(
            "no_items",
            "a proforma needs at least one line item on the order",
        ));
    }
    let partner = partners::find(&mut *conn, order.partner_id)
        .await?
        .ok_or(AppError::NotFound("partner"))?;
    let snapshot = snapshot_lines(&items, currency, vat_rate, 1)?;
    let exchange_rate = exchange_rate(&mut conn, currency, issue_date).await?;
    let customer = customer_party(&partner)?;
    drop(conn);

    // The number is drawn in the same transaction that records the proforma, and the
    // series lock is held across the render. The number is `max + 1` over stored rows, so
    // releasing the lock before the row exists would let a concurrent request draw the
    // same number, render and file a PDF under it, and only then fail on the unique index.
    // A failed render rolls back and leaves no gap.
    let mut tx = state.db.begin().await?;
    let number =
        invoices::next_proforma_number(&mut tx, &nav.proforma_prefix, issue_date.year()).await?;

    let request = nav::ProformaRequest {
        invoice: nav::InvoiceRequest {
            invoice_number: number.clone(),
            issue_date,
            delivery_date: Some(issue_date),
            payment_date: Some(payment_date),
            currency: currency.code().to_string(),
            exchange_rate,
            payment_method: Some("TRANSFER".into()),
            order_numbers: Some(vec![order.number.clone()]),
            supplier: supplier_party(nav),
            customer,
            lines: snapshot.lines.iter().map(|(_, l)| l.clone()).collect(),
        },
        id: number.clone(),
        note: req.note.clone(),
    };

    let rendered = client
        .create_proforma(&request)
        .await
        .map_err(nav_to_app_error)?;
    let pdf = base64_decode(&rendered.pdf_base64)?;

    let filename = format!("{number}.pdf");
    let document_id = store_pdf(
        state,
        order_id,
        DocumentKind::Proforma,
        &filename,
        pdf,
        Some(&number),
    )
    .await
    .map_err(|e| AppError::internal(format!("storing the proforma: {e}")))?;

    let proforma = invoices::insert_proforma(
        &mut tx,
        &NewProforma {
            order_id,
            number: &number,
            currency: currency.code(),
            issue_date,
            payment_date: Some(payment_date),
            net_amount: snapshot.net,
            vat_amount: snapshot.vat,
            gross_amount: snapshot.net + snapshot.vat,
            document_id,
            created_by: Some(user.user_id),
        },
    )
    .await?;
    audit::record(
        &mut *tx,
        Some(user.user_id),
        "order",
        order_id,
        "proforma_created",
        json!({ "proforma_id": proforma.id, "number": proforma.number, "document_id": document_id }),
    )
    .await?;

    let email_id = if req.send_email {
        queue_proforma_email(&mut tx, state, &order, &proforma, document_id).await?
    } else {
        None
    };
    tx.commit().await?;

    tracing::info!(proforma_id = proforma.id, number = %proforma.number, order_id, "proforma rendered (not reported to NAV)");
    Ok(ProformaCreated { proforma, email_id })
}

// ── Letters ─────────────────────────────────────────────────────────────────

fn money_text(minor: i64, currency: &str) -> String {
    let parsed: Currency = currency.parse().unwrap_or(Currency::HUF);
    format!("{}", Money::new(minor, parsed))
}

fn hu_date(day: NaiveDate) -> String {
    day.format("%Y.%m.%d.").to_string()
}

/// The letter that goes with an issued invoice, a storno, or an annulment.
async fn queue_invoice_email(
    state: &AppState,
    invoice: &Invoice,
    document_id: Option<i64>,
) -> anyhow::Result<Option<i64>> {
    let mut tx = state.db.begin().await?;
    let Some(order) = orders::find(&mut *tx, invoice.order_id).await? else {
        return Ok(None);
    };
    let Some(recipient) = automation::customer_recipient(&mut tx, &order).await? else {
        tracing::warn!(
            invoice_id = invoice.id,
            "no customer address; not sending the invoice letter"
        );
        return Ok(None);
    };

    let (template_key, trigger) = match (invoice.status, invoice.kind) {
        (InvoiceStatus::Annulled, _) => ("invoice_annulled", triggers::INVOICE_ANNULLED),
        (_, InvoiceKind::Storno) => ("invoice_stornoed", triggers::INVOICE_STORNOED),
        (_, InvoiceKind::Invoice) => ("invoice_issued", triggers::INVOICE_ISSUED),
    };

    let mut values = crate::domain::template::TemplateValues::new();
    values.insert("invoice.number", invoice.number.clone());
    values.insert("invoice.issue_date", hu_date(invoice.issue_date));
    values.insert(
        "invoice.total",
        money_text(invoice.gross_amount, &invoice.currency),
    );
    values.insert(
        "invoice.payment_method",
        PaymentMethod::parse(&invoice.payment_method)
            .map(|m| m.hu_label().to_string())
            .unwrap_or_else(|_| invoice.payment_method.clone()),
    );
    if let Some(day) = invoice.payment_date {
        values.insert("invoice.payment_date", hu_date(day));
    }
    if let Some(original_id) = invoice.original_invoice_id
        && let Some(original) = invoices::find(&mut *tx, original_id).await?
    {
        values.insert("invoice.original_number", original.number);
    }

    // An annulment notice carries no document: the point of it is that the report should
    // never have existed.
    let attachments = match (document_id, invoice.status) {
        (Some(id), status) if status != InvoiceStatus::Annulled => vec![AttachmentRef {
            document_id: id,
            filename: None,
            byte_size: None,
            mode: None,
            content_id: None,
        }],
        _ => Vec::new(),
    };

    let email_id = queue_automatic(
        &mut tx,
        &state.config,
        AutomaticEmail {
            template_key,
            about: About {
                order_id: Some(invoice.order_id),
                ..Default::default()
            },
            to: &recipient,
            trigger,
            // One letter per invoice per outcome, whatever the queue retries.
            idempotency_key: format!("{trigger}:{}", invoice.id),
            attachments,
            extra_values: values,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(email_id)
}

async fn queue_proforma_email(
    conn: &mut PgConnection,
    state: &AppState,
    order: &Order,
    proforma: &Proforma,
    document_id: i64,
) -> AppResult<Option<i64>> {
    let Some(recipient) = automation::customer_recipient(&mut *conn, order).await? else {
        tracing::warn!(
            proforma_id = proforma.id,
            "no customer address; not sending the proforma"
        );
        return Ok(None);
    };
    let mut values = crate::domain::template::TemplateValues::new();
    values.insert("proforma.number", proforma.number.clone());
    values.insert(
        "proforma.total",
        money_text(proforma.gross_amount, &proforma.currency),
    );
    if let Some(day) = proforma.payment_date {
        values.insert("proforma.payment_date", hu_date(day));
    }

    queue_automatic(
        conn,
        &state.config,
        AutomaticEmail {
            template_key: "proforma_created",
            about: About {
                order_id: Some(order.id),
                ..Default::default()
            },
            to: &recipient,
            trigger: triggers::PROFORMA,
            idempotency_key: format!("proforma:{}", proforma.id),
            attachments: vec![AttachmentRef {
                document_id,
                filename: None,
                byte_size: None,
                mode: None,
                content_id: None,
            }],
            extra_values: values,
        },
    )
    .await
}

// ── Reading ─────────────────────────────────────────────────────────────────

/// The invoice's chain at NAV: the original and everything that modified it.
pub async fn chain(state: &AppState, invoice: &Invoice) -> AppResult<Vec<nav::ChainElement>> {
    let (client, _) = sidecar(&state.config)?;
    let response = client
        .chain(&invoice.number)
        .await
        .map_err(nav_to_app_error)?;
    Ok(response.elements)
}

/// Builds the document the sidecar reports, from the stored snapshot.
///
/// The snapshot, not the order: the order may have been edited since, and an invoice is
/// what was invoiced.
async fn build_request(
    state: &AppState,
    nav_cfg: &NavConfig,
    invoice: &Invoice,
    payment_method: Option<&str>,
) -> AppResult<nav::InvoiceRequest> {
    let mut conn = state.db.acquire().await?;
    let order = orders::find(&mut *conn, invoice.order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let partner = partners::find(&mut *conn, order.partner_id)
        .await?
        .ok_or(AppError::NotFound("partner"))?;
    let currency: Currency = invoice
        .currency
        .parse()
        .map_err(|e| AppError::internal(format!("{e}")))?;
    let lines = invoices::lines_for(&mut *conn, invoice.id).await?;
    let exchange_rate = exchange_rate(&mut conn, currency, invoice.issue_date).await?;
    // The stored method is the record; the job payload wins when it parses, so jobs
    // enqueued before the method was validated still report what the office chose.
    let method = payment_method
        .and_then(|raw| PaymentMethod::parse(raw).ok())
        .or_else(|| PaymentMethod::parse(&invoice.payment_method).ok())
        .unwrap_or(PaymentMethod::DEFAULT);

    Ok(nav::InvoiceRequest {
        invoice_number: invoice.number.clone(),
        issue_date: invoice.issue_date,
        delivery_date: Some(invoice.delivery_date),
        payment_date: invoice.payment_date,
        currency: currency.code().to_string(),
        exchange_rate,
        payment_method: Some(method.as_str().to_string()),
        order_numbers: Some(vec![order.number.clone()]),
        supplier: supplier_party(nav_cfg),
        customer: customer_party(&partner)?,
        lines: lines
            .iter()
            .map(|line| nav::Line {
                description: line.description.clone(),
                quantity: line.quantity.normalize().to_string(),
                unit: Some(line.unit.clone()),
                unit_price: minor_to_decimal_string(line.unit_price, currency),
                vat_percentage: line.vat_rate.normalize().to_string(),
                nature: None,
            })
            .collect(),
    })
}

/// Surfaces a NAV failure to the caller with its fault code intact.
///
/// Used on the paths that answer a request directly — the proforma render and the chain
/// query. The asynchronous ones record the same detail on the invoice row instead.
pub fn nav_to_app_error(error: NavError) -> AppError {
    match &error {
        NavError::Unreachable(detail) => AppError::rule(
            "nav_unreachable",
            format!("the invoicing service could not be reached: {detail}"),
        ),
        NavError::Refused(fault) => {
            let code = error.nav_error_code();
            let detail = match code {
                Some(code) => format!("{}: {}", code, fault.message),
                None => fault.message.clone(),
            };
            match fault.kind.as_str() {
                "not_found" => AppError::NotFound("invoice at NAV"),
                "nav_unreachable" => AppError::rule("nav_unreachable", detail),
                _ => AppError::rule("nav_rejected", detail),
            }
        }
        NavError::Contract(detail) => AppError::internal(format!("NAV sidecar: {detail}")),
    }
}

fn base64_decode(value: &str) -> AppResult<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|e| AppError::internal(format!("the rendered PDF was not valid base64: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_stornos_of_this_invoice_count() {
        assert!(!blocks_another_storno(
            InvoiceKind::Storno,
            InvoiceStatus::Submitting,
            Some(8),
            7
        ));
        assert!(!blocks_another_storno(
            InvoiceKind::Invoice,
            InvoiceStatus::Issued,
            None,
            7
        ));
    }

    // INV-L8: adopting a duplicate-number answer is only safe when NAV holds exactly
    // our document. The decision is pure so it is tested without a database.
    fn stored_row() -> Invoice {
        use chrono::{TimeZone, Utc};
        Invoice {
            id: 1,
            order_id: 7,
            number: "AT2026-0001".into(),
            kind: InvoiceKind::Invoice,
            status: InvoiceStatus::Submitting,
            original_invoice_id: None,
            currency: "HUF".into(),
            issue_date: NaiveDate::from_ymd_opt(2026, 9, 21).unwrap(),
            delivery_date: NaiveDate::from_ymd_opt(2026, 9, 21).unwrap(),
            payment_date: None,
            payment_method: "TRANSFER".into(),
            net_amount: 100_000_000,
            vat_amount: 27_000_000,
            gross_amount: 127_000_000,
            nav_transaction_id: None,
            nav_status: None,
            nav_error_code: None,
            nav_message: None,
            nav_messages: serde_json::Value::Array(Vec::new()),
            annulment_transaction_id: None,
            annulment_code: None,
            annulment_reason: None,
            annulled_at: None,
            document_id: None,
            submitted_at: None,
            issued_at: None,
            created_by: None,
            created_at: Utc.with_ymd_and_hms(2026, 9, 21, 9, 0, 0).unwrap(),
            updated_at: Utc.with_ymd_and_hms(2026, 9, 21, 9, 0, 0).unwrap(),
        }
    }

    fn held_row() -> nav::FetchedInvoice {
        nav::FetchedInvoice {
            invoice_number: "AT2026-0001".into(),
            issue_date: "2026-09-21".into(),
            totals: Some(nav::Totals {
                currency: "HUF".into(),
                net: "1000000.00".into(),
                vat: "270000.00".into(),
                gross: "1270000.00".into(),
            }),
        }
    }

    #[test]
    fn a_retry_is_adopted_when_nav_holds_our_exact_document() {
        assert_eq!(reconcile_decision(&stored_row(), &held_row()), Ok(()));
    }

    #[test]
    fn a_retry_is_refused_when_any_total_differs() {
        let mut held = held_row();
        held.totals.as_mut().unwrap().gross = "1270000.01".into();
        let Err(reason) = reconcile_decision(&stored_row(), &held) else {
            panic!("a different gross must not be adopted");
        };
        assert!(reason.contains("gross"), "reason was {reason}");
    }

    #[test]
    fn a_retry_is_refused_when_currency_or_date_differ() {
        let mut held = held_row();
        held.totals.as_mut().unwrap().currency = "EUR".into();
        assert!(reconcile_decision(&stored_row(), &held).is_err());

        let mut held = held_row();
        held.issue_date = "2026-09-22".into();
        let Err(reason) = reconcile_decision(&stored_row(), &held) else {
            panic!("a different issue date must not be adopted");
        };
        assert!(reason.contains("2026-09-22"), "reason was {reason}");
    }

    #[test]
    fn a_retry_is_refused_without_totals_or_with_unreadable_ones() {
        let mut held = held_row();
        held.totals = None;
        assert!(reconcile_decision(&stored_row(), &held).is_err());

        let mut held = held_row();
        held.totals.as_mut().unwrap().net = "a lot".into();
        assert!(reconcile_decision(&stored_row(), &held).is_err());

        // Three fraction digits is not an amount we could have sent.
        let mut held = held_row();
        held.totals.as_mut().unwrap().net = "1000000.001".into();
        assert!(reconcile_decision(&stored_row(), &held).is_err());
    }

    #[test]
    fn nav_decimals_become_minor_units_exactly() {
        assert_eq!(nav_decimal_to_minor("1270000.00"), Some(127_000_000));
        assert_eq!(nav_decimal_to_minor("0.27"), Some(27));
        assert_eq!(nav_decimal_to_minor("1000000.001"), None);
        assert_eq!(nav_decimal_to_minor("a lot"), None);
    }

    #[test]
    fn only_unbuildable_submit_failures_are_terminal_on_dead_letter() {
        assert!(is_unbuildable_dead_letter(
            "invoice_unbuildable: building the invoice: invoice_data_missing: ..."
        ));
        assert!(!is_unbuildable_dead_letter(
            "the NAV sidecar is unreachable: connection refused"
        ));
        assert!(!is_unbuildable_dead_letter(
            "NAV rejected the invoice: INVOICE_NUMBER_ALREADY_EXISTS: ..."
        ));
        assert!(!is_unbuildable_dead_letter("timed out after 600s"));
    }

    #[test]
    fn a_storno_being_reported_blocks_another_storno_of_the_same_invoice() {
        assert!(blocks_another_storno(
            InvoiceKind::Storno,
            InvoiceStatus::Submitting,
            Some(7),
            7
        ));
        assert!(blocks_another_storno(
            InvoiceKind::Storno,
            InvoiceStatus::Issued,
            Some(7),
            7
        ));
    }

    #[test]
    fn a_rejected_storno_attempt_does_not_block_the_next_one() {
        assert!(!blocks_another_storno(
            InvoiceKind::Storno,
            InvoiceStatus::Rejected,
            Some(7),
            7
        ));
    }
}

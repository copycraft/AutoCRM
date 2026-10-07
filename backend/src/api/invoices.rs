//! Invoicing over HTTP: issue, storno, annul, the chain, and proformas.
//!
//! Issuing is asynchronous — the handler answers `202 Accepted` with the invoice in
//! `submitting`, and the screen polls it until NAV has decided. Everything NAV said,
//! including its fault code on a rejection, is on the row that comes back; nothing is
//! flattened into "failed".

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use crate::AppState;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::integrations::nav::ChainElement;
use crate::repo::invoices::{BilledInvoice, BilledProforma, Invoice, InvoiceLine, Proforma};
use crate::repo::{documents, invoices, orders};
use crate::service::invoicing::{
    self, AnnulRequest, IssueRequest, ProformaCreated, ProformaRequest, StornoRequest,
};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_all))
        .routes(routes!(bucket_counts))
        .routes(routes!(mark_paid))
        .routes(routes!(set_reminders))
        .routes(routes!(overdue))
        .routes(routes!(list_all_proformas))
        .routes(routes!(list_for_order, create))
        .routes(routes!(detail))
        .routes(routes!(storno))
        .routes(routes!(annul))
        .routes(routes!(chain))
        .routes(routes!(refetch_pdf))
        .routes(routes!(list_proformas, create_proforma))
        // 0049
        .routes(routes!(list_payments, add_payment))
        .routes(routes!(delete_payment))
        .routes(routes!(statement))
}

/// One document in the invoice's chain at NAV.
///
/// The sidecar speaks NAV's camelCase; this API does not. Restating the four fields the
/// screen uses keeps the contract in one shape rather than leaving a camelCase island in
/// the middle of it.
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct ChainStep {
    pub invoice_number: String,
    /// `CREATE`, `MODIFY` or `STORNO`.
    pub operation: String,
    /// When NAV recorded it, UTC.
    pub ins_date: String,
    /// The document this one modifies, on a storno or a correction.
    pub original_invoice_number: Option<String>,
}

impl From<ChainElement> for ChainStep {
    fn from(e: ChainElement) -> Self {
        ChainStep {
            invoice_number: e.invoice_number,
            operation: e.operation,
            ins_date: e.ins_date,
            original_invoice_number: e.modifies.map(|m| m.original_invoice_number),
        }
    }
}

/// An invoice with everything the screen shows about it.
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct InvoiceDetail {
    #[serde(flatten)]
    pub invoice: Invoice,
    pub lines: Vec<InvoiceLine>,
    /// Filename of the stored PDF, when there is one. Download it through
    /// `GET /documents/{id}/download` like any other document.
    pub document_filename: Option<String>,
}

/// The Számlázó page: every invoice and storno, newest first, across orders.
///
/// Stornos are rows like any other (`kind = 'storno'`, negative amounts,
/// `original_invoice_id` set) — no second query to see them. Reads are open;
/// issuing stays behind `IssueInvoices` on the per-order path.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct BillingQuery {
    /// `submitting` | `issued` | `rejected` | `stornoed` | `annulled`. Omitted: all.
    status: Option<String>,
    /// `invoice` | `storno`. Omitted: both.
    kind: Option<String>,
    /// The list: `to_issue` | `issued` | `paid` | `archived` | `stornoed` | `storno`.
    bucket: Option<String>,
    /// Newest N rows. Defaults to 100, at most 500.
    limit: Option<i64>,
}

fn billing_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(100).clamp(1, 500)
}

#[utoipa::path(
    get, path = "/invoices", tag = "invoices",
    params(BillingQuery),
    responses((status = 200, body = Items<BilledInvoice>))
)]
async fn list_all(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<BillingQuery>,
) -> AppResult<Json<Items<BilledInvoice>>> {
    const STATUSES: &[&str] = &["submitting", "issued", "rejected", "stornoed", "annulled"];
    const KINDS: &[&str] = &["invoice", "storno"];
    if let Some(status) = &q.status
        && !STATUSES.contains(&status.as_str())
    {
        return Err(AppError::validation(format!(
            "status must be one of {}",
            STATUSES.join(", ")
        )));
    }
    if let Some(bucket) = &q.bucket
        && !BUCKETS.contains(&bucket.as_str())
    {
        return Err(AppError::validation(format!(
            "bucket must be one of {}",
            BUCKETS.join(", ")
        )));
    }
    if let Some(kind) = &q.kind
        && !KINDS.contains(&kind.as_str())
    {
        return Err(AppError::validation(format!(
            "kind must be one of {}",
            KINDS.join(", ")
        )));
    }
    Ok(Items::new(
        invoices::list_invoices(
            &state.db,
            q.status.as_deref(),
            q.kind.as_deref(),
            q.bucket.as_deref(),
            billing_limit(q.limit),
        )
        .await?,
    ))
}

/// The Számlázó page: every díjbekérő, newest first, across orders.
#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ProformasQuery {
    /// Newest N rows. Defaults to 100, at most 500.
    limit: Option<i64>,
}

#[utoipa::path(
    get, path = "/proformas", tag = "invoices",
    params(ProformasQuery),
    responses((status = 200, body = Items<BilledProforma>))
)]
async fn list_all_proformas(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<ProformasQuery>,
) -> AppResult<Json<Items<BilledProforma>>> {
    Ok(Items::new(
        invoices::list_proformas(&state.db, billing_limit(q.limit)).await?,
    ))
}

#[utoipa::path(
    get, path = "/orders/{id}/invoices", tag = "invoices",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<Invoice>))
)]
async fn list_for_order(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Items<Invoice>>> {
    Ok(Items::new(invoices::list_for_order(&state.db, id).await?))
}

/// Issue an invoice for an order and report it to NAV.
#[utoipa::path(
    post, path = "/orders/{id}/invoices", tag = "invoices",
    params(("id" = i64, Path)),
    request_body = IssueRequest,
    responses(
        (status = 202, body = Invoice, description = "Queued for reporting; poll GET /invoices/{id}"),
        (status = 409, description = "An invoice is already in flight or live for this order"),
    )
)]
async fn create(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(body): ApiJson<IssueRequest>,
) -> AppResult<(StatusCode, Json<Invoice>)> {
    me.require(Capability::IssueInvoices)?;
    let invoice = invoicing::create_invoice(&state, &me, id, &body).await?;
    Ok((StatusCode::ACCEPTED, Json(invoice)))
}

#[utoipa::path(
    get, path = "/invoices/{id}", tag = "invoices",
    params(("id" = i64, Path)),
    responses((status = 200, body = InvoiceDetail))
)]
async fn detail(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<InvoiceDetail>> {
    let invoice = invoices::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("invoice"))?;
    let lines = invoices::lines_for(&state.db, id).await?;
    let document_filename = match invoice.document_id {
        Some(document_id) => documents::find(&state.db, document_id)
            .await?
            .map(|d| d.filename),
        None => None,
    };
    Ok(Json(InvoiceDetail {
        invoice,
        lines,
        document_filename,
    }))
}

/// Storno an issued invoice. The answer is the storno document, being reported.
#[utoipa::path(
    post, path = "/invoices/{id}/storno", tag = "invoices",
    params(("id" = i64, Path)),
    request_body = StornoRequest,
    responses(
        (status = 202, body = Invoice),
        (status = 422, description = "Only an issued invoice can be stornoed"),
    )
)]
async fn storno(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(body): ApiJson<StornoRequest>,
) -> AppResult<(StatusCode, Json<Invoice>)> {
    me.require(Capability::IssueInvoices)?;
    let storno = invoicing::create_storno(&state, &me, id, &body).await?;
    Ok((StatusCode::ACCEPTED, Json(storno)))
}

/// Technically annul a data report. Admin only.
#[utoipa::path(
    post, path = "/invoices/{id}/annul", tag = "invoices",
    params(("id" = i64, Path)),
    request_body = AnnulRequest,
    responses(
        (status = 202, body = Invoice),
        (status = 403, description = "Admin only"),
    )
)]
async fn annul(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(body): ApiJson<AnnulRequest>,
) -> AppResult<(StatusCode, Json<Invoice>)> {
    me.require(Capability::AnnulInvoices)?;
    let invoice = invoicing::annul_invoice(&state, &me, id, &body).await?;
    Ok((StatusCode::ACCEPTED, Json(invoice)))
}

/// Fetch the PDF of an issued invoice again (INV-L9).
///
/// For the invoice whose report NAV stored but whose file never arrived — a slow
/// object store at the wrong moment. Re-renders from what NAV holds and files it with
/// the order's documents. The letter queued at issue time is left alone: it may already
/// have gone out without the attachment, and re-sending mail is the office's call.
#[utoipa::path(
    post, path = "/invoices/{id}/pdf", tag = "invoices",
    params(("id" = i64, Path)),
    responses(
        (status = 200, body = Invoice),
        (status = 409, description = "The invoice already has its PDF"),
        (status = 422, description = "Only an issued invoice has a report to render"),
    )
)]
async fn refetch_pdf(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Invoice>> {
    me.require(Capability::IssueInvoices)?;
    Ok(Json(invoicing::refetch_pdf(&state, id).await?))
}

/// The invoice's modification chain, straight from NAV.
#[utoipa::path(
    get, path = "/invoices/{id}/chain", tag = "invoices",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<ChainStep>))
)]
async fn chain(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Items<ChainStep>>> {
    let invoice = invoices::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("invoice"))?;
    let steps = invoicing::chain(&state, &invoice)
        .await?
        .into_iter()
        .map(ChainStep::from)
        .collect();
    Ok(Items::new(steps))
}

#[utoipa::path(
    get, path = "/orders/{id}/proformas", tag = "invoices",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<Proforma>))
)]
async fn list_proformas(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Items<Proforma>>> {
    Ok(Items::new(
        invoices::list_proformas_for_order(&state.db, id).await?,
    ))
}

/// Render a proforma (díjbekérő) for an order.
///
/// Nothing is reported to NAV, no invoice status changes, and no invoice needs to exist:
/// a proforma is a request for payment, and usable from the day the order is priced.
#[utoipa::path(
    post, path = "/orders/{id}/proformas", tag = "invoices",
    params(("id" = i64, Path)),
    request_body = ProformaRequest,
    responses((status = 201, body = ProformaCreated))
)]
async fn create_proforma(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(body): ApiJson<ProformaRequest>,
) -> AppResult<(StatusCode, Json<ProformaCreated>)> {
    me.require(Capability::IssueInvoices)?;
    // A proforma hangs off the order, so the order has to exist before anything is rendered.
    orders::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let created = invoicing::create_proforma(&state, &me, id, &body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

const BUCKETS: &[&str] = &[
    "to_issue", "issued", "paid", "archived", "stornoed", "storno",
];

/// How many invoices are on each list, for the Számlázó sidebar.
#[utoipa::path(
    get, path = "/invoices/buckets", tag = "invoices",
    responses((status = 200, body = Items<invoices::BucketCount>))
)]
async fn bucket_counts(
    State(state): State<AppState>,
    Auth(_): Auth,
) -> AppResult<Json<Items<invoices::BucketCount>>> {
    Ok(Items::new(invoices::bucket_counts(&state.db).await?))
}

#[derive(Deserialize, ToSchema)]
struct PaidBody {
    paid: bool,
}

/// Marks an issued invoice paid (Fizetve) or back to unpaid (Kiállítva). Cash invoices
/// are paid on issue without this.
#[utoipa::path(
    post, path = "/invoices/{id}/paid", tag = "invoices",
    params(("id" = i64, Path)),
    request_body = PaidBody,
    responses((status = 204, description = "Updated"), (status = 422, description = "Not an issued invoice"))
)]
async fn mark_paid(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<PaidBody>,
) -> AppResult<StatusCode> {
    me.require(Capability::IssueInvoices)?;
    let mut tx = state.db.begin().await?;
    if !invoices::set_paid(&mut *tx, id, b.paid).await? {
        return Err(AppError::rule(
            "not_issued",
            "only an issued invoice can be marked paid",
        ));
    }
    crate::repo::audit::record(
        &mut *tx,
        Some(me.user_id),
        "invoice",
        id,
        if b.paid { "paid" } else { "unpaid" },
        serde_json::json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
struct RemindersBody {
    /// True stops the automatic payment reminders for this invoice.
    off: bool,
}

/// Stops (or restarts) the automatic payment reminders of one invoice: a customer who pays
/// late by agreement should not be chased.
#[utoipa::path(
    post, path = "/invoices/{id}/reminders", tag = "invoices",
    params(("id" = i64, Path)),
    request_body = RemindersBody,
    responses((status = 204, description = "Updated"), (status = 404, description = "No such invoice"))
)]
async fn set_reminders(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<RemindersBody>,
) -> AppResult<StatusCode> {
    me.require(Capability::IssueInvoices)?;
    let mut tx = state.db.begin().await?;
    if !invoices::set_reminders_off(&mut *tx, id, b.off).await? {
        return Err(AppError::NotFound("invoice"));
    }
    crate::repo::audit::record(
        &mut *tx,
        Some(me.user_id),
        "invoice",
        id,
        if b.off {
            "reminders_off"
        } else {
            "reminders_on"
        },
        serde_json::json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct OverdueQuery {
    /// Only this customer's.
    partner_id: Option<i64>,
}

/// Unpaid invoices past their deadline, oldest first.
#[utoipa::path(
    get, path = "/invoices/overdue", tag = "invoices",
    params(OverdueQuery),
    responses((status = 200, body = Items<invoices::OverdueInvoice>))
)]
async fn overdue(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<OverdueQuery>,
) -> AppResult<Json<Items<invoices::OverdueInvoice>>> {
    let today = crate::service::business_today(state.config.business_tz);
    Ok(Items::new(
        invoices::overdue(&state.db, today, q.partner_id, 500).await?,
    ))
}

// --- Payments and the statement of account (0049) ---

use crate::repo::payments::{self, InvoicePayment, NewPayment, StatementLine};

#[utoipa::path(
    get, path = "/invoices/{id}/payments", tag = "invoices",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<InvoicePayment>))
)]
async fn list_payments(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Items<InvoicePayment>>> {
    Ok(Items::new(payments::list(&state.db, id).await?))
}

#[derive(Deserialize, ToSchema)]
struct PaymentBody {
    /// Minor units in the invoice's currency; at most what is still open.
    amount_minor: i64,
    /// Defaults to today.
    paid_on: Option<NaiveDate>,
    /// TRANSFER (default), CASH, CARD or OTHER.
    method: Option<String>,
    note: Option<String>,
}

#[utoipa::path(
    post, path = "/invoices/{id}/payments", tag = "invoices",
    params(("id" = i64, Path)),
    request_body = PaymentBody,
    responses(
        (status = 201, body = Items<InvoicePayment>, description = "Booked; the invoice's payments after it. Reaching the gross marks the invoice paid."),
        (status = 422, description = "Not an issued invoice (`not_issued`), or more than is open (`overpayment`)")
    )
)]
async fn add_payment(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<PaymentBody>,
) -> AppResult<(StatusCode, Json<Items<InvoicePayment>>)> {
    me.require(Capability::IssueInvoices)?;
    if b.amount_minor <= 0 {
        return Err(AppError::validation("amount_minor must be positive"));
    }
    let method = b
        .method
        .as_deref()
        .map(str::trim)
        .unwrap_or("TRANSFER")
        .to_ascii_uppercase();
    if !["TRANSFER", "CASH", "CARD", "OTHER"].contains(&method.as_str()) {
        return Err(AppError::validation(
            "method: expected TRANSFER, CASH, CARD or OTHER",
        ));
    }
    let note = super::optional(b.note);
    if note.as_deref().is_some_and(|n| n.chars().count() > 500) {
        return Err(AppError::validation("note: at most 500 characters"));
    }
    let today = crate::service::business_today(state.config.business_tz);
    let paid_on = b.paid_on.unwrap_or(today);
    if paid_on > today {
        return Err(AppError::validation("paid_on cannot be in the future"));
    }
    let mut tx = state.db.begin().await?;
    let payable = payments::lock_payable(&mut tx, id).await?.ok_or_else(|| {
        AppError::rule(
            "not_issued",
            "payments are booked against an issued invoice",
        )
    })?;
    let open = payable.gross_amount - payable.paid_amount;
    if b.amount_minor > open {
        return Err(AppError::rule(
            "overpayment",
            format!(
                "only {open} (minor units, {}) is still open on this invoice",
                payable.currency
            ),
        ));
    }
    let payment_id = payments::insert(
        &mut tx,
        &NewPayment {
            invoice_id: id,
            amount_minor: b.amount_minor,
            paid_on,
            method: &method,
            note: note.as_deref(),
            created_by: Some(me.user_id),
        },
    )
    .await?;
    crate::repo::audit::record(
        &mut *tx,
        Some(me.user_id),
        "invoice",
        id,
        "payment",
        serde_json::json!({ "payment_id": payment_id, "amount_minor": b.amount_minor, "paid_on": paid_on }),
    )
    .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Items::new(payments::list(&state.db, id).await?),
    ))
}

#[utoipa::path(
    delete, path = "/invoices/{id}/payments/{payment_id}", tag = "invoices",
    params(("id" = i64, Path), ("payment_id" = i64, Path)),
    responses((status = 204, description = "Removed; the invoice is open again for that much"))
)]
async fn delete_payment(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath((id, payment_id)): ApiPath<(i64, i64)>,
) -> AppResult<StatusCode> {
    me.require(Capability::IssueInvoices)?;
    let mut tx = state.db.begin().await?;
    if payments::lock_payable(&mut tx, id).await?.is_none() {
        return Err(AppError::rule(
            "not_issued",
            "payments are booked against an issued invoice",
        ));
    }
    if !payments::delete(&mut tx, id, payment_id).await? {
        return Err(AppError::NotFound("payment"));
    }
    crate::repo::audit::record(
        &mut *tx,
        Some(me.user_id),
        "invoice",
        id,
        "payment_removed",
        serde_json::json!({ "payment_id": payment_id }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct StatementQuery {
    /// First day; defaults to a year before `to`.
    from: Option<NaiveDate>,
    /// Last day; defaults to today.
    to: Option<NaiveDate>,
}

/// Per currency: what was invoiced in the period, what came in, and what is open.
#[derive(Serialize, ToSchema)]
struct StatementTotal {
    currency: String,
    /// Gross of the invoices and stornos issued in the period.
    invoiced: i64,
    /// Payments received in the period.
    received: i64,
    /// Open now, whenever issued.
    outstanding: i64,
    /// Of that, past its deadline.
    overdue: i64,
}

#[derive(Serialize, ToSchema)]
struct Statement {
    partner_id: i64,
    partner_name: String,
    partner_address: String,
    partner_tax_number: Option<String>,
    from: NaiveDate,
    to: NaiveDate,
    generated_on: NaiveDate,
    lines: Vec<StatementLine>,
    payments: Vec<InvoicePayment>,
    totals: Vec<StatementTotal>,
}

/// A partner's statement of account (folyószámla-kivonat, egyenlegközlő): invoices and
/// stornos issued in the period, older invoices still open, and the payments received.
#[utoipa::path(
    get, path = "/partners/{id}/statement", tag = "invoices",
    params(("id" = i64, Path), StatementQuery),
    responses((status = 200, body = Statement))
)]
async fn statement(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiQuery(q): ApiQuery<StatementQuery>,
) -> AppResult<Json<Statement>> {
    let today = crate::service::business_today(state.config.business_tz);
    let to = q.to.unwrap_or(today);
    let from = q.from.unwrap_or(to - chrono::TimeDelta::days(365));
    if from > to {
        return Err(AppError::validation("from must not be after to"));
    }
    let partner = crate::repo::partners::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("partner"))?;
    let lines = payments::statement(&state.db, id, from, to).await?;
    let received = payments::statement_payments(&state.db, id, from, to).await?;

    let mut totals: Vec<StatementTotal> = Vec::new();
    fn total_for(totals: &mut Vec<StatementTotal>, currency: &str) -> usize {
        match totals.iter().position(|t| t.currency == currency) {
            Some(i) => i,
            None => {
                totals.push(StatementTotal {
                    currency: currency.to_string(),
                    invoiced: 0,
                    received: 0,
                    outstanding: 0,
                    overdue: 0,
                });
                totals.len() - 1
            }
        }
    }
    let mut currency_of = std::collections::HashMap::new();
    for line in &lines {
        currency_of.insert(line.invoice_id, line.currency.clone());
        let i = total_for(&mut totals, &line.currency);
        if line.issue_date >= from && line.issue_date <= to {
            totals[i].invoiced += line.gross_amount;
        }
        totals[i].outstanding += line.outstanding;
        if line.payment_date.is_some_and(|d| d < today) {
            totals[i].overdue += line.outstanding;
        }
    }
    for p in &received {
        let currency = match currency_of.get(&p.invoice_id) {
            Some(c) => c.clone(),
            None => invoices::find(&state.db, p.invoice_id)
                .await?
                .map(|i| i.currency)
                .unwrap_or_else(|| "HUF".into()),
        };
        let i = total_for(&mut totals, &currency);
        totals[i].received += p.amount_minor;
    }
    let address = [
        partner.postal_code.as_deref(),
        partner.city.as_deref(),
        partner.address_line.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    Ok(Json(Statement {
        partner_id: partner.id,
        partner_name: partner.name,
        partner_address: address,
        partner_tax_number: partner.tax_number,
        from,
        to,
        generated_on: today,
        lines,
        payments: received,
        totals,
    }))
}

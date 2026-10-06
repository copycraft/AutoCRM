//! Invoicing over HTTP: issue, storno, annul, the chain, and proformas.
//!
//! Issuing is asynchronous — the handler answers `202 Accepted` with the invoice in
//! `submitting`, and the screen polls it until NAV has decided. Everything NAV said,
//! including its fault code on a rejection, is on the row that comes back; nothing is
//! flattened into "failed".

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
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
        .routes(routes!(list_all_proformas))
        .routes(routes!(list_for_order, create))
        .routes(routes!(detail))
        .routes(routes!(storno))
        .routes(routes!(annul))
        .routes(routes!(chain))
        .routes(routes!(refetch_pdf))
        .routes(routes!(list_proformas, create_proforma))
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

const BUCKETS: &[&str] = &["to_issue", "issued", "paid", "archived", "stornoed", "storno"];

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

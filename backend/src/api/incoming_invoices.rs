//! Incoming invoices: supplier bills. Drop the file in (one request per file), fill in what
//! it says, mark it paid. The list each lands on follows from those figures.
//!
//! Reading needs a login; uploading and editing need `IssueInvoices`, the office's
//! invoicing right.

use std::time::Duration;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::{Items, optional, page_limit, page_offset, patch as patch_field, patch_text};
use crate::AppState;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::media::storage::content_disposition;
use crate::repo::incoming_invoices::{
    self, BUCKETS, Fields, IncomingBucketCount, IncomingInvoice, KnownSupplier, NewFile,
};
use crate::repo::{audit, like_pattern};

const MAX_FILE_BYTES: usize = 25 * 1024 * 1024;
const URL_TTL: Duration = Duration::from_secs(3600);

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(counts))
        .routes(routes!(suppliers))
        .routes(routes!(detail, update, remove))
        .routes(routes!(file_url))
        .routes(routes!(bulk_action))
        .merge(
            OpenApiRouter::new()
                .routes(routes!(upload))
                .layer(DefaultBodyLimit::max(MAX_FILE_BYTES)),
        )
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ListQuery {
    /// Supplier, invoice number, tax number or file name.
    q: Option<String>,
    /// open_invoice | open_proforma | transferred | cash | partial | cash_receipt | booking_only
    bucket: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[utoipa::path(
    get, path = "/incoming-invoices", tag = "incoming_invoices",
    params(ListQuery),
    responses((status = 200, body = Items<IncomingInvoice>))
)]
async fn list(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> AppResult<Json<Items<IncomingInvoice>>> {
    let bucket = optional(q.bucket);
    if let Some(b) = &bucket
        && !BUCKETS.contains(&b.as_str())
    {
        return Err(AppError::validation(format!(
            "bucket must be one of {}",
            BUCKETS.join(", ")
        )));
    }
    let pattern = optional(q.q).and_then(|s| like_pattern(&s));
    Ok(Items::new(
        incoming_invoices::search(
            &state.db,
            pattern.as_deref(),
            bucket.as_deref(),
            page_limit(q.limit),
            page_offset(q.offset),
        )
        .await?,
    ))
}

#[utoipa::path(
    get, path = "/incoming-invoices/buckets", tag = "incoming_invoices",
    responses((status = 200, body = Items<IncomingBucketCount>))
)]
async fn counts(
    State(state): State<AppState>,
    Auth(_): Auth,
) -> AppResult<Json<Items<IncomingBucketCount>>> {
    Ok(Items::new(
        incoming_invoices::bucket_counts(&state.db).await?,
    ))
}

/// Suppliers seen before, for filling in the next invoice from the same one.
#[utoipa::path(
    get, path = "/incoming-invoices/suppliers", tag = "incoming_invoices",
    responses((status = 200, body = Items<KnownSupplier>))
)]
async fn suppliers(
    State(state): State<AppState>,
    Auth(_): Auth,
) -> AppResult<Json<Items<KnownSupplier>>> {
    Ok(Items::new(
        incoming_invoices::known_suppliers(&state.db).await?,
    ))
}

#[utoipa::path(
    get, path = "/incoming-invoices/{id}", tag = "incoming_invoices",
    params(("id" = i64, Path)),
    responses((status = 200, body = IncomingInvoice))
)]
async fn detail(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<IncomingInvoice>> {
    Ok(Json(
        incoming_invoices::find(&state.db, id)
            .await?
            .ok_or(AppError::NotFound("incoming invoice"))?,
    ))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct UploadQuery {
    /// The file's own name, shown in the list and used for the download.
    filename: String,
}

/// What a file is, from its first bytes; the Content-Type header is only a hint.
pub(crate) fn sniff(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(b"%PDF") {
        Some(("application/pdf", "pdf"))
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(("image/jpeg", "jpg"))
    } else if bytes.starts_with(b"\x89PNG") {
        Some(("image/png", "png"))
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(("image/webp", "webp"))
    } else if bytes.trim_ascii_start().starts_with(b"<?xml")
        || bytes.trim_ascii_start().starts_with(b"<")
    {
        // NAV online invoice XML, as suppliers' systems send it.
        Some(("application/xml", "xml"))
    } else {
        None
    }
}

/// One supplier invoice: the raw file as the body (PDF, JPEG, PNG, WebP or XML, up to
/// 25 MB), its name in `?filename=`. It lands on Nyitott számla with nothing filled in.
/// The same file twice is refused, naming the invoice that already holds it.
#[utoipa::path(
    post, path = "/incoming-invoices/upload", tag = "incoming_invoices",
    params(UploadQuery),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = 201, body = IncomingInvoice),
        (status = 409, description = "This file is already uploaded"),
    )
)]
async fn upload(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiQuery(q): ApiQuery<UploadQuery>,
    body: Bytes,
) -> AppResult<(StatusCode, Json<IncomingInvoice>)> {
    me.require(Capability::IssueInvoices)?;
    if body.is_empty() {
        return Err(AppError::validation("the request body must be the file"));
    }
    let name = super::required("filename", &q.filename)?;
    let name: String = name.chars().take(200).collect();
    let (content_type, ext) = sniff(&body)
        .ok_or_else(|| AppError::validation("only PDF, JPEG, PNG, WebP or XML files"))?;
    let hash = Sha256::digest(&body).to_vec();
    if let Some(existing) = incoming_invoices::by_hash(&state.db, &hash).await? {
        return Err(AppError::conflict(
            "duplicate",
            format!("this file is already uploaded as incoming invoice #{existing}"),
        ));
    }
    let key = format!("incoming-invoices/{}.{ext}", hex::encode(&hash));
    let size = body.len() as i64;
    state
        .storage
        .put_bytes(&key, body.to_vec(), content_type)
        .await
        .map_err(|e| AppError::internal(format!("storing incoming invoice: {e}")))?;
    let mut tx = state.db.begin().await?;
    let id = incoming_invoices::insert_with_file(
        &mut *tx,
        &NewFile {
            key: &key,
            name: &name,
            content_type,
            size,
            hash: &hash,
        },
        me.user_id,
    )
    .await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "incoming_invoice",
        id,
        "upload",
        json!({ "file_name": name }),
    )
    .await?;
    tx.commit().await?;
    let row = incoming_invoices::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("incoming invoice"))?;
    Ok((StatusCode::CREATED, Json(row)))
}

/// Every field optional: absent keeps, `null` clears.
#[derive(Deserialize, ToSchema)]
struct IncomingBody {
    /// `invoice` | `proforma` | `receipt`.
    kind: Option<String>,
    supplier_name: Option<String>,
    #[serde(default, deserialize_with = "patch_field")]
    supplier_tax_number: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    partner_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "patch_field")]
    invoice_number: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    issue_date: Option<Option<NaiveDate>>,
    #[serde(default, deserialize_with = "patch_field")]
    due_date: Option<Option<NaiveDate>>,
    /// `HUF` | `EUR`.
    currency: Option<String>,
    /// Minor units (fillér / eurocent).
    #[serde(default, deserialize_with = "patch_field")]
    net_amount: Option<Option<i64>>,
    #[serde(default, deserialize_with = "patch_field")]
    vat_amount: Option<Option<i64>>,
    #[serde(default, deserialize_with = "patch_field")]
    gross_amount: Option<Option<i64>>,
    /// `TRANSFER` | `CASH` | `CARD`.
    payment_method: Option<String>,
    /// How much of the gross has been paid, minor units.
    paid_amount: Option<i64>,
    #[serde(default, deserialize_with = "patch_field")]
    paid_on: Option<Option<NaiveDate>>,
    booking_only: Option<bool>,
    #[serde(default, deserialize_with = "patch_field")]
    notes: Option<Option<String>>,
}

fn merged(current: &IncomingInvoice, b: IncomingBody) -> AppResult<Fields> {
    let mut f = Fields::from(current);
    if let Some(kind) = b.kind {
        if !["invoice", "proforma", "receipt"].contains(&kind.as_str()) {
            return Err(AppError::validation(
                "kind must be invoice, proforma or receipt",
            ));
        }
        f.kind = kind;
    }
    if let Some(name) = b.supplier_name {
        f.supplier_name = name.trim().chars().take(200).collect();
    }
    f.supplier_tax_number = patch_text(&f.supplier_tax_number, b.supplier_tax_number);
    if let Some(p) = b.partner_id {
        f.partner_id = p;
    }
    f.invoice_number = patch_text(&f.invoice_number, b.invoice_number);
    if let Some(d) = b.issue_date {
        f.issue_date = d;
    }
    if let Some(d) = b.due_date {
        f.due_date = d;
    }
    if let Some(c) = b.currency {
        if c != "HUF" && c != "EUR" {
            return Err(AppError::validation("currency must be HUF or EUR"));
        }
        f.currency = c;
    }
    for (slot, value) in [
        (&mut f.net_amount, b.net_amount),
        (&mut f.vat_amount, b.vat_amount),
        (&mut f.gross_amount, b.gross_amount),
    ] {
        if let Some(v) = value {
            *slot = v;
        }
    }
    if f.gross_amount.is_some_and(|g| g < 0) {
        return Err(AppError::validation("gross_amount cannot be negative"));
    }
    if let Some(m) = b.payment_method {
        if !["TRANSFER", "CASH", "CARD"].contains(&m.as_str()) {
            return Err(AppError::validation(
                "payment_method must be TRANSFER, CASH or CARD",
            ));
        }
        f.payment_method = m;
    }
    if let Some(p) = b.paid_amount {
        if p < 0 {
            return Err(AppError::validation("paid_amount cannot be negative"));
        }
        f.paid_amount = p;
    }
    if let Some(d) = b.paid_on {
        f.paid_on = d;
    }
    if let Some(v) = b.booking_only {
        f.booking_only = v;
    }
    f.notes = patch_text(&f.notes, b.notes);
    Ok(f)
}

#[utoipa::path(
    patch, path = "/incoming-invoices/{id}", tag = "incoming_invoices",
    params(("id" = i64, Path)),
    request_body = IncomingBody,
    responses((status = 200, body = IncomingInvoice), (status = 409, description = "That supplier's invoice number is already recorded"))
)]
async fn update(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<IncomingBody>,
) -> AppResult<Json<IncomingInvoice>> {
    me.require(Capability::IssueInvoices)?;
    let current = incoming_invoices::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("incoming invoice"))?;
    let fields = merged(&current, b)?;
    let mut tx = state.db.begin().await?;
    incoming_invoices::update(&mut *tx, id, &fields).await?;
    let changes = audit::diff(&[
        ("kind", json!(current.kind), json!(fields.kind)),
        (
            "supplier_name",
            json!(current.supplier_name),
            json!(fields.supplier_name),
        ),
        (
            "invoice_number",
            json!(current.invoice_number),
            json!(fields.invoice_number),
        ),
        (
            "gross_amount",
            json!(current.gross_amount),
            json!(fields.gross_amount),
        ),
        (
            "paid_amount",
            json!(current.paid_amount),
            json!(fields.paid_amount),
        ),
        (
            "payment_method",
            json!(current.payment_method),
            json!(fields.payment_method),
        ),
        (
            "booking_only",
            json!(current.booking_only),
            json!(fields.booking_only),
        ),
    ]);
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "incoming_invoice",
        id,
        "update",
        changes,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(
        incoming_invoices::find(&state.db, id)
            .await?
            .ok_or(AppError::NotFound("incoming invoice"))?,
    ))
}

#[utoipa::path(
    delete, path = "/incoming-invoices/{id}", tag = "incoming_invoices",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Removed from the lists; the record and file are kept"))
)]
async fn remove(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::IssueInvoices)?;
    let mut tx = state.db.begin().await?;
    if !incoming_invoices::soft_delete(&mut *tx, id).await? {
        return Err(AppError::NotFound("incoming invoice"));
    }
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "incoming_invoice",
        id,
        "delete",
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
struct FileUrl {
    /// Presigned, expires after one hour; opens in the browser.
    url: String,
}

#[utoipa::path(
    get, path = "/incoming-invoices/{id}/file", tag = "incoming_invoices",
    params(("id" = i64, Path)),
    responses((status = 200, body = FileUrl))
)]
async fn file_url(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<FileUrl>> {
    let (key, name) = incoming_invoices::file_key(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("incoming invoice file"))?;
    let url = state
        .storage
        .presign_get(&key, URL_TTL, Some(content_disposition("inline", &name)))
        .await?;
    Ok(Json(FileUrl { url }))
}

#[cfg(test)]
mod tests {
    use super::sniff;

    #[test]
    fn files_are_told_apart_by_their_first_bytes() {
        assert_eq!(sniff(b"%PDF-1.7 ...").map(|s| s.1), Some("pdf"));
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]).map(|s| s.1), Some("jpg"));
        assert_eq!(sniff(b"\x89PNG\r\n").map(|s| s.1), Some("png"));
        assert_eq!(
            sniff(b"  <?xml version=\"1.0\"?>").map(|s| s.1),
            Some("xml")
        );
        assert_eq!(sniff(b"MZ\x90\x00"), None);
    }
}

#[derive(Deserialize, ToSchema)]
struct IncomingBulkBody {
    ids: Vec<i64>,
    #[serde(flatten)]
    action: IncomingBulkAction,
}

/// One operation over the ticked supplier invoices (0049).
#[derive(Deserialize, ToSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
enum IncomingBulkAction {
    /// Paid in full; `paid_on` defaults to today and only fills rows without a date.
    MarkPaid { paid_on: Option<NaiveDate> },
    /// Exists only in the books (or back to a normal invoice with `on: false`).
    BookingOnly { on: bool },
    /// Off the lists; records and files are kept.
    Delete,
}

#[derive(Serialize, ToSchema)]
struct IncomingBulkResult {
    changed: u64,
}

#[utoipa::path(
    post, path = "/incoming-invoices/bulk-actions", tag = "incoming_invoices",
    request_body = IncomingBulkBody,
    responses((status = 200, body = IncomingBulkResult))
)]
async fn bulk_action(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<IncomingBulkBody>,
) -> AppResult<Json<IncomingBulkResult>> {
    me.require(Capability::IssueInvoices)?;
    if b.ids.len() > 1000 {
        return Err(AppError::validation("at most 1000 invoices at once"));
    }
    let today = crate::service::business_today(state.config.business_tz);
    let mut tx = state.db.begin().await?;
    let (changed, what) = match &b.action {
        IncomingBulkAction::MarkPaid { paid_on } => (
            incoming_invoices::mark_paid_many(&mut *tx, &b.ids, paid_on.unwrap_or(today)).await?,
            "bulk_paid",
        ),
        IncomingBulkAction::BookingOnly { on } => (
            incoming_invoices::set_booking_only_many(&mut *tx, &b.ids, *on).await?,
            "bulk_booking_only",
        ),
        IncomingBulkAction::Delete => (
            incoming_invoices::soft_delete_many(&mut *tx, &b.ids).await?,
            "bulk_delete",
        ),
    };
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "incoming_invoice",
        0,
        what,
        json!({ "ids": b.ids, "changed": changed }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(IncomingBulkResult { changed }))
}

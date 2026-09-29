//! Images and documents on an order.

use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::{Items, page_limit, page_offset, patch as patch_field, patch_text};
use crate::AppState;
use crate::domain::media::{DocumentKind, ImageCategory};
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::media::storage::content_disposition;
use crate::repo::documents::{self, Document};
use crate::repo::images::{self, Image};
use crate::repo::{audit, leads, orders};
use crate::service::media::{self, Completed, UploadRequest, UploadResponse};

const URL_TTL: Duration = Duration::from_secs(3600);

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(request_upload))
        .routes(routes!(request_lead_upload))
        .routes(routes!(list_lead_documents))
        .routes(routes!(search_documents))
        .routes(routes!(update_document))
        .routes(routes!(complete_upload))
        .routes(routes!(list_images))
        .routes(routes!(original_url))
        .routes(routes!(delete_image))
        .routes(routes!(list_documents))
        .routes(routes!(document_url))
        .routes(routes!(delete_document))
}

#[utoipa::path(
    post, path = "/orders/{id}/uploads", tag = "media",
    params(("id" = i64, Path)),
    request_body = UploadRequest,
    responses((status = 200, body = UploadResponse))
)]
async fn request_upload(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(order_id): ApiPath<i64>,
    ApiJson(body): ApiJson<UploadRequest>,
) -> AppResult<Json<UploadResponse>> {
    me.require(Capability::UploadMedia)?;
    Ok(Json(
        media::request_upload(&state, &me, documents::Owner::Order(order_id), body).await?,
    ))
}

/// V2.4: a quotation PDF hangs off the lead it was sent for.
#[utoipa::path(
    post, path = "/leads/{id}/uploads", tag = "media",
    params(("id" = i64, Path)),
    request_body = UploadRequest,
    responses((status = 200, body = UploadResponse))
)]
async fn request_lead_upload(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(lead_id): ApiPath<i64>,
    ApiJson(body): ApiJson<UploadRequest>,
) -> AppResult<Json<UploadResponse>> {
    me.require(Capability::UploadMedia)?;
    Ok(Json(
        media::request_upload(&state, &me, documents::Owner::Lead(lead_id), body).await?,
    ))
}

#[derive(Deserialize, ToSchema)]
struct CompleteBody {
    ticket: String,
}

#[utoipa::path(
    post, path = "/uploads/complete", tag = "media",
    request_body = CompleteBody,
    responses(
        (status = 201, description = "Recorded", body = Completed),
        (status = 200, description = "Already recorded", body = Completed)
    )
)]
async fn complete_upload(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(body): ApiJson<CompleteBody>,
) -> AppResult<(StatusCode, Json<Completed>)> {
    me.require(Capability::UploadMedia)?;
    let completed = media::complete_upload(&state, &me, &body.ticket).await?;
    let created = match &completed {
        Completed::Image { created, .. } | Completed::Document { created, .. } => *created,
    };
    Ok((
        if created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(completed),
    ))
}

#[derive(Serialize, ToSchema)]
struct ImageView {
    #[serde(flatten)]
    image: Image,
    /// Presigned, expires after one hour; null until the thumbnail is generated.
    thumb_url: Option<String>,
    /// Presigned, expires after one hour.
    display_url: Option<String>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ImagesQuery {
    category: Option<ImageCategory>,
}

#[utoipa::path(
    get, path = "/orders/{id}/images", tag = "media",
    params(("id" = i64, Path), ImagesQuery),
    responses((status = 200, body = Items<ImageView>))
)]
async fn list_images(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(order_id): ApiPath<i64>,
    ApiQuery(q): ApiQuery<ImagesQuery>,
) -> AppResult<Json<Items<ImageView>>> {
    orders::find(&state.db, order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let rows = images::list_for_order(&state.db, order_id, q.category).await?;
    let mut views = Vec::with_capacity(rows.len());
    for image in rows {
        // Presigning is local signing, no network round trip, so doing it per image is cheap.
        let thumb_url = match &image.thumb_key {
            Some(k) => Some(state.storage.presign_get(k, URL_TTL, None).await?),
            None => None,
        };
        let display_url = match &image.display_key {
            Some(k) => Some(state.storage.presign_get(k, URL_TTL, None).await?),
            None => None,
        };
        views.push(ImageView {
            image,
            thumb_url,
            display_url,
        });
    }
    Ok(Items::new(views))
}

#[derive(Serialize, ToSchema)]
struct OriginalImageUrl {
    /// Presigned, expires after one hour.
    url: String,
    /// Hex sha256 of the original file.
    sha256: String,
}

#[utoipa::path(
    get, path = "/images/{id}/original", tag = "media",
    params(("id" = i64, Path)),
    responses((status = 200, body = OriginalImageUrl))
)]
async fn original_url(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<OriginalImageUrl>> {
    me.require(Capability::ViewOriginalImages)?;
    let image = images::find(&state.db, id)
        .await?
        .filter(|i| i.deleted_at.is_none())
        .ok_or(AppError::NotFound("image"))?;
    let filename = image
        .original_filename
        .clone()
        .unwrap_or_else(|| format!("image-{id}"));
    let url = state
        .storage
        .presign_get(
            &image.storage_key,
            URL_TTL,
            Some(content_disposition("attachment", &filename)),
        )
        .await?;
    tracing::info!(
        image_id = id,
        user_id = me.user_id,
        "original image accessed"
    );
    Ok(Json(OriginalImageUrl {
        url,
        sha256: hex::encode(&image.content_hash),
    }))
}

#[utoipa::path(
    delete, path = "/images/{id}", tag = "media",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Deleted. Intake photos answer `409 immutable`."))
)]
async fn delete_image(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::DeleteMedia)?;
    let mut tx = state.db.begin().await?;
    let image = images::find(&mut *tx, id)
        .await?
        .filter(|i| i.deleted_at.is_none())
        .ok_or(AppError::NotFound("image"))?;
    if image.immutable {
        return Err(AppError::conflict(
            "immutable",
            "intake photos are write-once evidence and cannot be deleted",
        ));
    }
    images::soft_delete(&mut *tx, id, me.user_id).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "order",
        image.order_id,
        "image_delete",
        json!({ "image_id": id, "category": image.category }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get, path = "/orders/{id}/documents", tag = "media",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<Document>))
)]
async fn list_documents(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(order_id): ApiPath<i64>,
) -> AppResult<Json<Items<Document>>> {
    orders::find(&state.db, order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    Ok(Items::new(
        documents::list_for_order(&state.db, order_id).await?,
    ))
}

/// V2.4: documents of a lead — the quotation, before any order exists.
#[utoipa::path(
    get, path = "/leads/{id}/documents", tag = "media",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<Document>))
)]
async fn list_lead_documents(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(lead_id): ApiPath<i64>,
) -> AppResult<Json<Items<Document>>> {
    leads::find(&state.db, lead_id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    Ok(Items::new(
        documents::list_for_lead(&state.db, lead_id).await?,
    ))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct DocumentSearchQuery {
    kind: Option<DocumentKind>,
    /// Certificates whose validity ends on or before this date. The question this query
    /// exists for: "which ATP certificates expire next quarter".
    expiring_before: Option<NaiveDate>,
    vehicle_id: Option<i64>,
    limit: Option<i64>,
    offset: Option<i64>,
}

/// The first cross-order document query in the system (V2.5). Without it a certificate is
/// reachable only through the one job it happened to be filed under.
#[utoipa::path(
    get, path = "/documents", tag = "media",
    params(DocumentSearchQuery),
    responses((status = 200, body = Items<Document>))
)]
async fn search_documents(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<DocumentSearchQuery>,
) -> AppResult<Json<Items<Document>>> {
    Ok(Items::new(
        documents::search(
            &state.db,
            q.kind,
            q.expiring_before,
            q.vehicle_id,
            page_limit(q.limit),
            page_offset(q.offset),
        )
        .await?,
    ))
}

#[derive(Deserialize, ToSchema)]
struct DocumentPatch {
    /// Who issued it — the ATP inspection body, the designer, the supplier.
    #[serde(default, deserialize_with = "patch_field")]
    issuer: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    valid_from: Option<Option<NaiveDate>>,
    #[serde(default, deserialize_with = "patch_field")]
    valid_until: Option<Option<NaiveDate>>,
    /// Which vehicle on a multi-vehicle order this covers.
    #[serde(default, deserialize_with = "patch_field")]
    vehicle_id: Option<Option<i64>>,
}

/// Validity and vehicle on an existing document (V2.5, V2.1). The bytes never change.
///
/// A PATCH like everywhere else: an omitted field keeps its value, `null` clears it.
/// The validity order is checked against the merged dates, not just the body.
#[utoipa::path(
    patch, path = "/documents/{id}", tag = "media",
    params(("id" = i64, Path)),
    request_body = DocumentPatch,
    responses((status = 200, body = Document))
)]
async fn update_document(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(body): ApiJson<DocumentPatch>,
) -> AppResult<Json<Document>> {
    me.require(Capability::UploadMedia)?;
    let current = documents::find(&state.db, id)
        .await?
        .filter(|d| d.deleted_at.is_none())
        .ok_or(AppError::NotFound("document"))?;
    let (validity, vehicle_id) = merge_validity(&current, body)?;
    let updated = documents::set_validity(&state.db, id, &validity, vehicle_id)
        .await?
        .ok_or(AppError::NotFound("document"))?;
    Ok(Json(updated))
}

/// Merges a document PATCH over the stored row (N10): an omitted field keeps its
/// value, `null` clears it — the API-wide PATCH contract. The validity order is
/// checked against the merged dates, so narrowing one end past the stored other end
/// is refused rather than stored.
fn merge_validity(
    current: &Document,
    body: DocumentPatch,
) -> AppResult<(documents::Validity, Option<i64>)> {
    let valid_from = body.valid_from.unwrap_or(current.valid_from);
    let valid_until = body.valid_until.unwrap_or(current.valid_until);
    if let (Some(from), Some(until)) = (valid_from, valid_until)
        && from > until
    {
        return Err(AppError::validation("valid_from is after valid_until"));
    }
    Ok((
        documents::Validity {
            issuer: patch_text(&current.issuer, body.issuer),
            valid_from,
            valid_until,
        },
        body.vehicle_id.unwrap_or(current.vehicle_id),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::media::DocumentKind;
    use chrono::{TimeZone, Utc};

    fn stored() -> Document {
        Document {
            id: 9,
            order_id: Some(7),
            lead_id: None,
            vehicle_id: Some(3),
            kind: DocumentKind::Certificate,
            filename: "atp.pdf".into(),
            content_type: "application/pdf".into(),
            storage_key: "orders/7/atp.pdf".into(),
            content_hash: vec![0u8; 32],
            byte_size: 10,
            issuer: Some("TÜV".into()),
            valid_from: NaiveDate::from_ymd_opt(2025, 1, 1),
            valid_until: NaiveDate::from_ymd_opt(2026, 1, 1),
            uploaded_at: Utc.with_ymd_and_hms(2025, 1, 2, 9, 0, 0).unwrap(),
            uploaded_by: None,
            deleted_at: None,
        }
    }

    fn patch(json: serde_json::Value) -> DocumentPatch {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn an_omitted_field_keeps_its_value_while_null_clears_it() {
        // Only the issuer is touched: everything else survives.
        let (v, vehicle) =
            merge_validity(&stored(), patch(serde_json::json!({"issuer": "DEKRA"}))).unwrap();
        assert_eq!(v.issuer.as_deref(), Some("DEKRA"));
        assert_eq!(v.valid_from, stored().valid_from);
        assert_eq!(v.valid_until, stored().valid_until);
        assert_eq!(vehicle, Some(3));

        // Explicit nulls clear.
        let (v, vehicle) = merge_validity(
            &stored(),
            patch(serde_json::json!({"issuer": null, "vehicle_id": null})),
        )
        .unwrap();
        assert_eq!(v.issuer, None);
        assert_eq!(vehicle, None);
        assert_eq!(v.valid_from, stored().valid_from);
    }

    #[test]
    fn narrowing_one_end_past_the_stored_other_end_is_refused() {
        let err = merge_validity(
            &stored(),
            patch(serde_json::json!({"valid_until": "2024-06-01"})),
        )
        .unwrap_err();
        assert!(
            matches!(&err, AppError::Validation(m) if m == "valid_from is after valid_until"),
            "got {err:?}",
        );
    }
}

#[derive(Serialize, ToSchema)]
struct DownloadUrl {
    /// Presigned, expires after one hour.
    url: String,
}

#[utoipa::path(
    get, path = "/documents/{id}/download", tag = "media",
    params(("id" = i64, Path)),
    responses((status = 200, body = DownloadUrl))
)]
async fn document_url(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<DownloadUrl>> {
    let doc = documents::find(&state.db, id)
        .await?
        .filter(|d| d.deleted_at.is_none())
        .ok_or(AppError::NotFound("document"))?;
    let url = state
        .storage
        .presign_get(
            &doc.storage_key,
            URL_TTL,
            Some(content_disposition("attachment", &doc.filename)),
        )
        .await?;
    Ok(Json(DownloadUrl { url }))
}

#[utoipa::path(
    delete, path = "/documents/{id}", tag = "media",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Deleted"))
)]
async fn delete_document(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::DeleteMedia)?;
    let mut tx = state.db.begin().await?;
    let doc = documents::find(&mut *tx, id)
        .await?
        .filter(|d| d.deleted_at.is_none())
        .ok_or(AppError::NotFound("document"))?;
    documents::soft_delete(&mut *tx, id, me.user_id).await?;
    let (entity, entity_id) = documents::Owner::from_about(doc.order_id, doc.lead_id)
        .ok_or_else(|| AppError::internal("document with no owner"))?
        .entity();
    audit::record(
        &mut *tx,
        Some(me.user_id),
        entity,
        entity_id,
        "document_delete",
        json!({ "document_id": id, "filename": doc.filename }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

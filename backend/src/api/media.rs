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
use crate::repo::images::{self, Annotations, Image, TimestampInfo};
use crate::repo::{audit, leads, orders, vehicles};
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
        .routes(routes!(update_image))
        .routes(routes!(get_annotations, put_annotations))
        .routes(routes!(timestamp_file))
        .routes(routes!(images_zip))
        .routes(routes!(document_preview))
        .routes(routes!(document_versions))
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
    /// The RFC 3161 stamp over the original, when it has one (evidence photos).
    timestamp: Option<TimestampInfo>,
    /// Whether shapes are drawn over it (`GET /images/{id}/annotations`).
    annotated: bool,
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
    let mut stamps: std::collections::HashMap<i64, TimestampInfo> =
        images::timestamps_for_order(&state.db, order_id)
            .await?
            .into_iter()
            .map(|t| (t.image_id, t))
            .collect();
    let annotated: std::collections::HashSet<i64> = images::annotated_ids(&state.db, order_id)
        .await?
        .into_iter()
        .collect();
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
            timestamp: stamps.remove(&image.id),
            annotated: annotated.contains(&image.id),
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

/// A document with its thumbnail, for lists.
#[derive(Serialize, ToSchema)]
struct DocumentView {
    #[serde(flatten)]
    document: Document,
    /// Presigned PNG, expires after one hour; null for files without one (PDFs preview in
    /// the browser instead).
    thumb_url: Option<String>,
}

async fn document_views(state: &AppState, docs: Vec<Document>) -> AppResult<Vec<DocumentView>> {
    let mut out = Vec::with_capacity(docs.len());
    for document in docs {
        let thumb_url = match &document.thumb_key {
            Some(k) => Some(state.storage.presign_get(k, URL_TTL, None).await?),
            None => None,
        };
        out.push(DocumentView {
            document,
            thumb_url,
        });
    }
    Ok(out)
}

/// The current version of every document of an order (older versions:
/// `GET /documents/{id}/versions`).
#[utoipa::path(
    get, path = "/orders/{id}/documents", tag = "media",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<DocumentView>))
)]
async fn list_documents(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(order_id): ApiPath<i64>,
) -> AppResult<Json<Items<DocumentView>>> {
    orders::find(&state.db, order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let docs = documents::list_for_order(&state.db, order_id).await?;
    Ok(Items::new(document_views(&state, docs).await?))
}

/// V2.4: documents of a lead — the quotation, before any order exists.
#[utoipa::path(
    get, path = "/leads/{id}/documents", tag = "media",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<DocumentView>))
)]
async fn list_lead_documents(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(lead_id): ApiPath<i64>,
) -> AppResult<Json<Items<DocumentView>>> {
    leads::find(&state.db, lead_id)
        .await?
        .ok_or(AppError::NotFound("lead"))?;
    let docs = documents::list_for_lead(&state.db, lead_id).await?;
    Ok(Items::new(document_views(&state, docs).await?))
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
            previous_version_id: None,
            version: 1,
            superseded_at: None,
            thumb_key: None,
            thumb_error: None,
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

// ── Captions, annotations, timestamps (0048) ────────────────────────────────

#[derive(Deserialize, ToSchema)]
struct ImagePatch {
    /// A line under the photo. Null clears it. Allowed on evidence photos too.
    #[serde(default, deserialize_with = "patch_field")]
    caption: Option<Option<String>>,
    /// Which of the order's vehicles the photo is of. Null clears it.
    #[serde(default, deserialize_with = "patch_field")]
    vehicle_id: Option<Option<i64>>,
}

#[utoipa::path(
    patch, path = "/images/{id}", tag = "media",
    params(("id" = i64, Path)),
    request_body = ImagePatch,
    responses((status = 200, body = Image))
)]
async fn update_image(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<ImagePatch>,
) -> AppResult<Json<Image>> {
    me.require(Capability::UploadMedia)?;
    let current = images::find(&state.db, id)
        .await?
        .filter(|i| i.deleted_at.is_none())
        .ok_or(AppError::NotFound("image"))?;
    let caption = patch_text(&current.caption, b.caption);
    if caption.as_deref().is_some_and(|c| c.chars().count() > 500) {
        return Err(AppError::validation("caption is at most 500 characters"));
    }
    let vehicle_id = b.vehicle_id.unwrap_or(current.vehicle_id);
    if let Some(vehicle_id) = vehicle_id
        && Some(vehicle_id) != current.vehicle_id
        && !vehicles::list_for_order(&state.db, current.order_id)
            .await?
            .iter()
            .any(|v| v.id == vehicle_id)
    {
        return Err(AppError::validation("the vehicle is not on this order"));
    }
    let updated = images::set_caption_vehicle(&state.db, id, caption.as_deref(), vehicle_id)
        .await?
        .ok_or(AppError::NotFound("image"))?;
    Ok(Json(updated))
}

#[utoipa::path(
    get, path = "/images/{id}/annotations", tag = "media",
    params(("id" = i64, Path)),
    responses((status = 200, body = Option<Annotations>))
)]
async fn get_annotations(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Option<Annotations>>> {
    images::find(&state.db, id)
        .await?
        .filter(|i| i.deleted_at.is_none())
        .ok_or(AppError::NotFound("image"))?;
    Ok(Json(images::annotations(&state.db, id).await?))
}

const SHAPE_KINDS: &[&str] = &["arrow", "line", "rect", "ellipse", "pen", "text"];
const MAX_SHAPES: usize = 200;
const MAX_SHAPES_JSON: usize = 64 * 1024;

/// Shapes are drawn by our own editor; this only keeps out what no editor sends: an
/// unknown kind, coordinates far off the picture, text that is an essay, a huge payload.
fn check_shapes(shapes: &serde_json::Value) -> AppResult<()> {
    fn numbers_in_range(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::Number(n) => n
                .as_f64()
                .is_some_and(|f| f.is_finite() && (-1.0..=2.0).contains(&f)),
            serde_json::Value::Array(a) => a.iter().all(numbers_in_range),
            _ => true,
        }
    }
    let list = shapes
        .as_array()
        .ok_or_else(|| AppError::validation("shapes must be a list"))?;
    if list.len() > MAX_SHAPES {
        return Err(AppError::validation(format!("at most {MAX_SHAPES} shapes")));
    }
    if shapes.to_string().len() > MAX_SHAPES_JSON {
        return Err(AppError::validation("the drawing is too large"));
    }
    for shape in list {
        let obj = shape
            .as_object()
            .ok_or_else(|| AppError::validation("a shape must be an object"))?;
        let kind = obj.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if !SHAPE_KINDS.contains(&kind) {
            return Err(AppError::validation(format!(
                "shape type must be one of {}",
                SHAPE_KINDS.join(", ")
            )));
        }
        for (key, value) in obj {
            if key == "text" {
                if value.as_str().is_some_and(|t| t.chars().count() > 200) {
                    return Err(AppError::validation("a label is at most 200 characters"));
                }
            } else if key == "color" {
                let ok = value.as_str().is_some_and(|c| {
                    c.len() == 7
                        && c.starts_with('#')
                        && c[1..].chars().all(|h| h.is_ascii_hexdigit())
                });
                if !ok {
                    return Err(AppError::validation("color must be #rrggbb"));
                }
            } else if !numbers_in_range(value) {
                return Err(AppError::validation(
                    "coordinates are fractions of the picture (0..1)",
                ));
            }
        }
    }
    Ok(())
}

#[derive(Deserialize, ToSchema)]
struct AnnotationsBody {
    #[schema(value_type = Vec<Object>)]
    shapes: serde_json::Value,
}

/// Replaces the shapes drawn over a photo. The photo itself never changes.
#[utoipa::path(
    put, path = "/images/{id}/annotations", tag = "media",
    params(("id" = i64, Path)),
    request_body = AnnotationsBody,
    responses((status = 200, body = Option<Annotations>))
)]
async fn put_annotations(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<AnnotationsBody>,
) -> AppResult<Json<Option<Annotations>>> {
    me.require(Capability::UploadMedia)?;
    let image = images::find(&state.db, id)
        .await?
        .filter(|i| i.deleted_at.is_none())
        .ok_or(AppError::NotFound("image"))?;
    check_shapes(&b.shapes)?;
    images::save_annotations(&state.db, id, &b.shapes, me.user_id).await?;
    audit::record(
        &state.db,
        Some(me.user_id),
        "order",
        image.order_id,
        "image_annotated",
        json!({ "image_id": id, "shapes": b.shapes.as_array().map_or(0, Vec::len) }),
    )
    .await?;
    Ok(Json(images::annotations(&state.db, id).await?))
}

/// The authority's signed answer (RFC 3161 TimeStampResp) for an evidence photo. Check it
/// with `openssl ts -verify -in kep.tsr -data original.jpg -CAfile tsa.pem`.
#[utoipa::path(
    get, path = "/images/{id}/timestamp", tag = "media",
    params(("id" = i64, Path)),
    responses((status = 200, description = "The .tsr file", content_type = "application/timestamp-reply"))
)]
async fn timestamp_file(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<axum::response::Response> {
    use axum::http::header;
    use axum::response::IntoResponse;
    let response = images::timestamp_response(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("timestamp"))?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "application/timestamp-reply".to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                content_disposition("attachment", &format!("kep-{id}.tsr")),
            ),
        ],
        response,
    )
        .into_response())
}

// ── ZIP download (0048) ─────────────────────────────────────────────────────

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ZipQuery {
    category: Option<ImageCategory>,
    /// `display` (default): the re-encoded copies, EXIF and GPS stripped, fit to send a
    /// customer. `original`: the files byte for byte (needs ViewOriginalImages).
    variant: Option<String>,
}

struct ZipEntry {
    name: String,
    key: String,
    size: u64,
    modified: chrono::NaiveDateTime,
}

/// An order's photos as one ZIP, a folder per category, streamed as it is read from
/// storage. Photos without a display copy yet are left out of the display download.
#[utoipa::path(
    get, path = "/orders/{id}/images/zip", tag = "media",
    params(("id" = i64, Path), ZipQuery),
    responses((status = 200, description = "The archive", content_type = "application/zip"))
)]
async fn images_zip(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(order_id): ApiPath<i64>,
    ApiQuery(q): ApiQuery<ZipQuery>,
) -> AppResult<axum::response::Response> {
    use crate::media::zip::{ZipStream, safe_name};
    use axum::body::{Body, Bytes};
    use axum::http::header;

    let order = orders::find(&state.db, order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    let originals = match q.variant.as_deref() {
        None | Some("display") => false,
        Some("original") => true,
        Some(_) => return Err(AppError::validation("variant must be display or original")),
    };
    if originals {
        me.require(Capability::ViewOriginalImages)?;
        tracing::info!(
            order_id,
            user_id = me.user_id,
            "original photos downloaded as ZIP"
        );
    }
    let labels: std::collections::HashMap<String, String> =
        crate::domain::lookups::image_categories()
            .into_iter()
            .map(|c| (c.key, c.label_hu))
            .collect();
    let rows = images::list_for_order(&state.db, order_id, q.category).await?;
    let mut entries = Vec::with_capacity(rows.len());
    for (i, image) in rows.into_iter().enumerate() {
        let folder = safe_name(
            labels
                .get(image.category.as_str())
                .map(String::as_str)
                .unwrap_or(image.category.as_str()),
        );
        let stem = image
            .caption
            .clone()
            .or_else(|| {
                image
                    .original_filename
                    .as_deref()
                    .map(|f| f.rsplit_once('.').map_or(f, |(stem, _)| stem).to_string())
            })
            .unwrap_or_else(|| format!("kep-{}", image.id));
        let stem: String = safe_name(&stem).chars().take(80).collect();
        let modified = image.captured_at.unwrap_or(image.uploaded_at).naive_utc();
        let (key, size, ext) = if originals {
            let ext = crate::domain::media::image_extension(&image.content_type).unwrap_or("bin");
            (image.storage_key.clone(), image.byte_size as u64, ext)
        } else {
            match &image.display_key {
                Some(k) => (k.clone(), 0, "jpg"),
                None => continue,
            }
        };
        entries.push(ZipEntry {
            name: format!("{folder}/{:03}_{stem}.{ext}", i + 1),
            key,
            size,
            modified,
        });
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(8);
    let storage = state.storage.clone();
    tokio::spawn(async move {
        let mut zip = ZipStream::new();
        for e in entries {
            let mut stream = match storage.open(&e.key).await {
                Ok(s) => s,
                Err(err) => {
                    tracing::error!(error = %err, key = %e.key, "ZIP download: could not open a photo");
                    let _ = tx.send(Err(std::io::Error::other(err.to_string()))).await;
                    return;
                }
            };
            if tx
                .send(Ok(Bytes::from(zip.begin(&e.name, e.size, e.modified))))
                .await
                .is_err()
            {
                return;
            }
            loop {
                match stream.next().await {
                    Some(Ok(chunk)) => {
                        zip.track(&chunk);
                        if tx.send(Ok(chunk)).await.is_err() {
                            return;
                        }
                    }
                    Some(Err(err)) => {
                        let _ = tx.send(Err(std::io::Error::other(err.to_string()))).await;
                        return;
                    }
                    None => break,
                }
            }
            if tx.send(Ok(Bytes::from(zip.end()))).await.is_err() {
                return;
            }
        }
        let _ = tx.send(Ok(Bytes::from(zip.finish()))).await;
    });
    let body = Body::from_stream(futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    }));
    let filename = format!(
        "{}-{}.zip",
        order.number,
        if originals { "eredeti-fotok" } else { "fotok" }
    );
    axum::response::Response::builder()
        .header(header::CONTENT_TYPE, "application/zip")
        .header(
            header::CONTENT_DISPOSITION,
            content_disposition("attachment", &filename),
        )
        .body(body)
        .map_err(|e| AppError::internal(e.to_string()))
}

// ── Document previews and versions (0048) ───────────────────────────────────

#[derive(Serialize, ToSchema)]
struct PreviewUrl {
    /// Presigned, served inline so the browser shows it; expires after one hour.
    url: String,
    content_type: String,
}

#[utoipa::path(
    get, path = "/documents/{id}/preview", tag = "media",
    params(("id" = i64, Path)),
    responses((status = 200, body = PreviewUrl))
)]
async fn document_preview(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<PreviewUrl>> {
    let doc = documents::find(&state.db, id)
        .await?
        .filter(|d| d.deleted_at.is_none())
        .ok_or(AppError::NotFound("document"))?;
    let url = state
        .storage
        .presign_get(
            &doc.storage_key,
            URL_TTL,
            Some(content_disposition("inline", &doc.filename)),
        )
        .await?;
    Ok(Json(PreviewUrl {
        url,
        content_type: doc.content_type,
    }))
}

#[utoipa::path(
    get, path = "/documents/{id}/versions", tag = "media",
    params(("id" = i64, Path)),
    responses((status = 200, body = Items<DocumentView>))
)]
async fn document_versions(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Items<DocumentView>>> {
    let all = documents::versions(&state.db, id).await?;
    if all.is_empty() {
        return Err(AppError::NotFound("document"));
    }
    Ok(Items::new(document_views(&state, all).await?))
}

#[cfg(test)]
mod shape_tests {
    use super::check_shapes;
    use serde_json::json;

    #[test]
    fn editor_shapes_pass_and_strange_ones_do_not() {
        assert!(
            check_shapes(&json!([
                {"type": "arrow", "points": [0.1, 0.2, 0.5, 0.5], "color": "#e11d48"},
                {"type": "text", "x": 0.3, "y": 0.4, "text": "karc", "color": "#000000"},
                {"type": "pen", "points": [[0.1, 0.1], [0.12, 0.13]]}
            ]))
            .is_ok()
        );
        assert!(check_shapes(&json!([{"type": "script"}])).is_err());
        assert!(check_shapes(&json!([{"type": "rect", "x": 40, "y": 0}])).is_err());
        assert!(check_shapes(&json!([{"type": "rect", "color": "red"}])).is_err());
        assert!(check_shapes(&json!({"type": "rect"})).is_err());
    }
}

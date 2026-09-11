//! Images and documents on an order.

use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use crate::AppState;
use crate::domain::media::ImageCategory;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::media::storage::content_disposition;
use crate::repo::documents::{self, Document};
use crate::repo::images::{self, Image};
use crate::repo::{audit, orders};
use crate::service::media::{self, Completed, UploadRequest, UploadResponse};

const URL_TTL: Duration = Duration::from_secs(3600);

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(request_upload))
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
        media::request_upload(&state, &me, order_id, body).await?,
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
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "order",
        doc.order_id,
        "document_delete",
        json!({ "document_id": id, "filename": doc.filename }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

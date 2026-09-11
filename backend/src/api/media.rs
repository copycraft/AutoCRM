//! Images and documents on an order.

use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

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

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/orders/{id}/uploads", post(request_upload))
        .route("/uploads/complete", post(complete_upload))
        .route("/orders/{id}/images", get(list_images))
        .route("/images/{id}/original", get(original_url))
        .route("/images/{id}", delete(delete_image))
        .route("/orders/{id}/documents", get(list_documents))
        .route("/documents/{id}/download", get(document_url))
        .route("/documents/{id}", delete(delete_document))
}

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

#[derive(Deserialize)]
struct CompleteBody {
    ticket: String,
}

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

#[derive(Serialize)]
struct ImageView {
    #[serde(flatten)]
    image: Image,
    thumb_url: Option<String>,
    display_url: Option<String>,
}

#[derive(Deserialize)]
struct ImagesQuery {
    category: Option<ImageCategory>,
}

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

async fn original_url(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<serde_json::Value>> {
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
    Ok(Json(
        json!({ "url": url, "sha256": hex::encode(&image.content_hash) }),
    ))
}

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

async fn document_url(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<serde_json::Value>> {
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
    Ok(Json(json!({ "url": url })))
}

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

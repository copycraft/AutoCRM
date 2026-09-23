//! HTTP handlers. Thin: parse → check capability → call service/repo → serialize.
//!
//! Every route is mounted through `OpenApiRouter::routes(routes!(..))`, which only accepts
//! handlers carrying a `#[utoipa::path]` annotation. A handler that is not in the OpenAPI
//! document therefore cannot be served: the published contract covers the whole API.

pub mod admin;
pub mod auth;
pub mod blockers;
pub mod configuration;
pub mod email;
pub mod extract;
pub mod inspections;
pub mod invoices;
pub mod leads;
pub mod media;
pub mod mobile;
pub mod newsletter;
pub mod openapi;
pub mod orders;
pub mod partners;
pub mod raw_import;
pub mod reports;
pub mod search;
pub mod tasks;
pub mod users;
pub mod vehicles;

use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Deserializer, Serialize};
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;

use crate::AppState;
use crate::error::{AppError, AppResult};

/// All `/api` routes, with their OpenAPI operations.
pub fn api_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .merge(auth::routes())
        .merge(users::routes())
        .merge(configuration::routes())
        .merge(partners::routes())
        .merge(raw_import::routes())
        .merge(leads::routes())
        .merge(orders::routes())
        .merge(blockers::routes())
        .merge(inspections::routes())
        .merge(media::routes())
        .merge(mobile::routes())
        .merge(newsletter::routes())
        .merge(email::routes())
        .merge(reports::routes())
        .merge(search::routes())
        .merge(tasks::routes())
        .merge(admin::routes())
        .merge(vehicles::routes())
        // Last, so adding it appends to the generated document instead of reshuffling it.
        .merge(invoices::routes())
}

pub fn router(state: AppState) -> Router {
    let (api, _) = api_routes().split_for_parts();

    Router::new()
        .route("/health", get(health))
        .nest("/api", api)
        // JSON lists compress 5-10x; the phone on the shop wifi feels it the most.
        .layer(CompressionLayer::new().gzip(true))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(CatchPanicLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .with_state(state)
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    database: &'static str,
    storage: &'static str,
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = sqlx::query_scalar!(r#"SELECT 1 AS "one!""#)
        .fetch_one(&state.db)
        .await
        .is_ok();
    let storage_ok = state.storage.check().await.is_ok();
    let status = if db_ok && storage_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let reachable = |ok: bool| if ok { "ok" } else { "unreachable" };
    (
        status,
        Json(Health {
            status: if status == StatusCode::OK {
                "ok"
            } else {
                "degraded"
            },
            database: reachable(db_ok),
            storage: reachable(storage_ok),
        }),
    )
}

/// The list envelope: `{"items": [...]}`.
#[derive(Debug, Serialize, ToSchema)]
pub struct Items<T> {
    pub items: Vec<T>,
}

impl<T> Items<T> {
    pub fn new(items: Vec<T>) -> Json<Self> {
        Json(Items { items })
    }
}

pub fn page_limit(limit: Option<i64>) -> i64 {
    limit.unwrap_or(50).clamp(1, 200)
}

pub fn page_offset(offset: Option<i64>) -> i64 {
    offset.unwrap_or(0).max(0)
}

/// Trimmed, non-empty required text.
pub fn required(field: &str, value: &str) -> AppResult<String> {
    let v = value.trim();
    if v.is_empty() {
        Err(AppError::validation(format!("{field} is required")))
    } else {
        Ok(v.to_string())
    }
}

/// Trimmed optional text; blank becomes None.
pub fn optional(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// For PATCH bodies: distinguishes an absent field (`None`) from an explicit null
/// (`Some(None)`). Use with `#[serde(default, deserialize_with = "patch")]`.
pub fn patch<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// Resolves a PATCH field against the current value: absent keeps it, null clears it.
pub fn apply_patch<T: Clone>(current: &Option<T>, patch: &Option<Option<T>>) -> Option<T> {
    match patch {
        None => current.clone(),
        Some(v) => v.clone(),
    }
}

/// Normalises an optional PATCH text field (trim, blank → null).
pub fn patch_text(current: &Option<String>, patch: Option<Option<String>>) -> Option<String> {
    match patch {
        None => current.clone(),
        Some(v) => optional(v),
    }
}

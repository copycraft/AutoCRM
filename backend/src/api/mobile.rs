//! The Android photo app needs exactly one thing beyond auth and uploads: a small order
//! picker it can cache offline. Plus where it updates itself from: sideloaded APKs get
//! no Play Store updates, so the release APK is served here for one-click self-update.

use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::{ApiQuery, Auth};
use crate::AppState;
use crate::domain::order::normalize_plate;
use crate::error::{AppError, AppResult};
use crate::repo::like_pattern;
use crate::repo::orders::{self, OrderFilter};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(order_picker))
        .routes(routes!(app_version))
        .routes(routes!(app_apk))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct PickerQuery {
    q: Option<String>,
    /// Defaults to open orders only; pass `all=true` to include finished ones.
    #[serde(default)]
    all: bool,
}

#[derive(Serialize, ToSchema)]
struct PickerOrder {
    id: i64,
    number: String,
    title: String,
    partner_name: String,
    vehicle: Option<String>,
    vehicle_plate: Option<String>,
    stage_key: String,
    stage_label: String,
}

#[utoipa::path(
    get, path = "/mobile/orders", tag = "mobile",
    params(PickerQuery),
    responses((status = 200, description = "Compact order picker; `Cache-Control: private, max-age=60`", body = Items<PickerOrder>))
)]
async fn order_picker(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<PickerQuery>,
) -> AppResult<impl IntoResponse> {
    let plate = q.q.as_deref().map(normalize_plate).unwrap_or_default();
    let filter = OrderFilter {
        pattern: q.q.as_deref().and_then(like_pattern),
        plate_pattern: if plate.is_empty() {
            String::new()
        } else {
            format!("%{plate}%")
        },
        open_only: !q.all,
        ..Default::default()
    };
    let rows = orders::search(&state.db, &filter, orders::DEFAULT_SORT, 200, 0).await?;
    let items = rows
        .into_iter()
        .map(|o| PickerOrder {
            vehicle: match (o.vehicle_make.as_deref(), o.vehicle_model.as_deref()) {
                (Some(make), Some(model)) => Some(format!("{make} {model}")),
                (Some(one), None) | (None, Some(one)) => Some(one.to_string()),
                (None, None) => None,
            },
            id: o.id,
            number: o.number,
            title: o.title,
            partner_name: o.partner_name,
            vehicle_plate: o.vehicle_plate,
            stage_key: o.stage_key,
            stage_label: o.stage_label,
        })
        .collect::<Vec<_>>();
    Ok((
        [(header::CACHE_CONTROL, "private, max-age=60")],
        Json(Items { items }),
    ))
}

/// What the phone's update dialog shows. Public on purpose: a phone with an expired
/// session and a stale build must still learn that it is stale.
#[derive(Serialize, ToSchema)]
struct AppVersion {
    /// The phone updates when this exceeds its own `versionCode`.
    version_code: i64,
    /// Shown in the dialog, e.g. "1.4.0".
    version_name: String,
    /// Where the APK downloads from: absolute path on this server, so the phone can
    /// resolve it against its stored base URL like every other call it makes.
    apk_url: String,
    /// Lowercase hex sha256 of the APK; the phone verifies before installing.
    sha256: String,
    size_bytes: i64,
    /// True: the phone refuses to work until it is updated.
    mandatory: bool,
    /// Short Hungarian release notes, one or two sentences per line. May be empty.
    changelog_hu: String,
}

#[utoipa::path(
    get, path = "/mobile/app-version", tag = "mobile",
    responses(
        (status = 200, description = "The release the phones should run; `Cache-Control: private, max-age=300`", body = AppVersion),
        (status = 404, description = "No release configured (APP_APK_PATH unset)"),
    )
)]
async fn app_version(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let update = state
        .config
        .app_update
        .as_ref()
        .ok_or(AppError::NotFound("app update"))?;
    let bytes = tokio::fs::read(&update.apk_path)
        .await
        .map_err(|e| AppError::internal(format!("reading the release APK: {e}")))?;
    Ok((
        [(header::CACHE_CONTROL, "private, max-age=300")],
        Json(AppVersion {
            version_code: update.version_code,
            version_name: update.version_name.clone(),
            apk_url: "/api/mobile/app.apk".to_string(),
            sha256: hex::encode(Sha256::digest(&bytes)),
            size_bytes: bytes.len() as i64,
            mandatory: update.mandatory,
            changelog_hu: update.changelog_hu.clone(),
        }),
    ))
}

#[utoipa::path(
    get, path = "/mobile/app.apk", tag = "mobile",
    responses(
        (status = 200, description = "The signed release APK", content_type = "application/vnd.android.package-archive", body = Vec<u8>),
        (status = 404, description = "No release configured (APP_APK_PATH unset)"),
    )
)]
async fn app_apk(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let update = state
        .config
        .app_update
        .as_ref()
        .ok_or(AppError::NotFound("app update"))?;
    let bytes = tokio::fs::read(&update.apk_path)
        .await
        .map_err(|e| AppError::internal(format!("reading the release APK: {e}")))?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "application/vnd.android.package-archive",
            ),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"autocrm.apk\"",
            ),
            (header::CACHE_CONTROL, "private, max-age=3600"),
        ],
        Body::from(bytes),
    ))
}

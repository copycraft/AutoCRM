//! Ad platforms (0049): the Facebook/Instagram lead ads webhook, and the conversion list
//! Google Ads fetches. Both are public (no staff login): Meta proves itself with the app
//! secret's signature, Google with the export key in the URL.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::ApiQuery;
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::service::ads;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(meta_verify, meta_webhook))
        .routes(routes!(google_conversions))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct VerifyQuery {
    #[serde(rename = "hub.mode")]
    #[param(rename = "hub.mode")]
    mode: Option<String>,
    #[serde(rename = "hub.verify_token")]
    #[param(rename = "hub.verify_token")]
    verify_token: Option<String>,
    #[serde(rename = "hub.challenge")]
    #[param(rename = "hub.challenge")]
    challenge: Option<String>,
}

/// Meta's subscription check: echoes the challenge when the verify token matches
/// \`META_VERIFY_TOKEN\`.
#[utoipa::path(
    get, path = "/webhooks/meta", tag = "ads", security(()),
    params(VerifyQuery),
    responses((status = 200, body = String, description = "The challenge"), (status = 403))
)]
async fn meta_verify(
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<VerifyQuery>,
) -> AppResult<String> {
    let expected = state
        .config
        .meta
        .as_ref()
        .and_then(|m| m.verify_token.as_deref())
        .ok_or(AppError::Forbidden)?;
    if q.mode.as_deref() != Some("subscribe") || q.verify_token.as_deref() != Some(expected) {
        return Err(AppError::Forbidden);
    }
    Ok(q.challenge.unwrap_or_default())
}

/// A lead form was filled in. Signed with the app secret (\`X-Hub-Signature-256\`); each
/// new submission is fetched and filed as a lead in the background.
#[utoipa::path(
    post, path = "/webhooks/meta", tag = "ads", security(()),
    request_body(content = String, content_type = "application/json"),
    responses((status = 200, description = "Received"), (status = 403, description = "Bad signature, or lead ads are off"))
)]
async fn meta_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<StatusCode> {
    let secret = state
        .config
        .meta
        .as_ref()
        .and_then(|m| m.app_secret.as_deref())
        .ok_or(AppError::Forbidden)?;
    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok());
    if !ads::signature_ok(secret, &body, signature) {
        tracing::warn!("meta webhook with a bad signature");
        return Err(AppError::Forbidden);
    }
    let ids = ads::leadgen_ids(&body);
    if !ids.is_empty() {
        let queued = ads::accept_leadgen(&state.db, &ids).await?;
        tracing::info!(announced = ids.len(), queued, "meta lead ads webhook");
    }
    Ok(StatusCode::OK)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ExportQuery {
    /// \`GOOGLE_ADS_EXPORT_KEY\`.
    key: String,
}

/// Won leads that came from a Google Ads click, in Google's offline-conversion CSV format.
/// Schedule it in Google Ads (Conversions → Uploads → Schedules → HTTPS) with this URL.
#[utoipa::path(
    get, path = "/ads/google/conversions.csv", tag = "ads", security(()),
    params(ExportQuery),
    responses((status = 200, content_type = "text/csv", body = String), (status = 403))
)]
async fn google_conversions(
    State(state): State<AppState>,
    ApiQuery(q): ApiQuery<ExportQuery>,
) -> AppResult<Response> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-export-key",
        q.key.parse().map_err(|_| AppError::Forbidden)?,
    );
    super::check_api_key(
        state.config.google_ads_export_key.as_deref(),
        &headers,
        "x-export-key",
    )?;
    let rows = ads::google_conversions(&state.db).await?;
    let csv = ads::google_csv(
        &rows,
        &state.config.google_ads_conversion_name,
        state.config.business_tz,
    );
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        csv,
    )
        .into_response())
}

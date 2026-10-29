//! EU VAT-number validation against VIES. Every VIES verdict is a 200 (INVALID,
//! UNAVAILABLE and ERROR included); only a malformed number is a 422.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::{ApiJson, ApiQuery, Auth};
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::integrations::vies::{VatCheckResult, ViesStatus};
use crate::repo::vat_checks::{self, VatCheck};
use crate::service::vat::{self, CheckRequest};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(check))
        .routes(routes!(status))
        .routes(routes!(history))
}

#[derive(Deserialize, ToSchema)]
struct VatSubject {
    /// Only `partner` for now.
    #[serde(rename = "type")]
    kind: String,
    id: i64,
}

#[derive(Deserialize, ToSchema)]
struct VatCheckBody {
    /// What the user typed: `HU12345678`, `12345678-2-06`, `DE 123.456.789`...
    input: String,
    /// The record's country (ISO or VIES code), used when the input has no prefix.
    default_country: Option<String>,
    /// When given, the verdict is filed on that record.
    subject: Option<VatSubject>,
    /// Ask VIES even when a recent verdict is on file.
    #[serde(default)]
    force: bool,
}

#[utoipa::path(
    post, path = "/vat/check", tag = "partners",
    request_body = VatCheckBody,
    responses(
        (status = 200, body = VatCheckResult, description = "Any VIES verdict, including INVALID, UNAVAILABLE and ERROR"),
        (status = 422, body = VatCheckResult, description = "FORMAT_INVALID: VIES was not asked"),
    )
)]
async fn check(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<VatCheckBody>,
) -> AppResult<Response> {
    vat::rate_limit(me.user_id)?;
    let partner_id = match b.subject {
        None => None,
        Some(VatSubject { kind, id }) if kind == "partner" => Some(id),
        Some(_) => return Err(AppError::validation("subject.type: only 'partner' is supported")),
    };
    let result = vat::check(
        &state.db,
        &state.config,
        CheckRequest {
            input: &b.input,
            default_country: b.default_country.as_deref(),
            partner_id,
            force: b.force,
            created_by: me.user_id.to_string(),
            interactive: true,
        },
    )
    .await?;
    Ok(match result {
        Ok(r) => Json(r).into_response(),
        Err(r) => (StatusCode::UNPROCESSABLE_ENTITY, Json(r)).into_response(),
    })
}

#[utoipa::path(
    get, path = "/vat/status", tag = "partners",
    responses(
        (status = 200, body = ViesStatus, description = "VIES's availability, overall and per member state (cached 5 minutes)"),
        (status = 503, description = "VIES could not be reached"),
    )
)]
async fn status(State(state): State<AppState>, Auth(_): Auth) -> Response {
    match vat::status(&state.config).await {
        Some(s) => Json(s.as_ref().clone()).into_response(),
        None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct VatHistoryQuery {
    /// Only `partner` for now.
    subject_type: String,
    subject_id: i64,
}

#[utoipa::path(
    get, path = "/vat/checks", tag = "partners",
    params(VatHistoryQuery),
    responses((status = 200, body = Items<VatCheck>))
)]
async fn history(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<VatHistoryQuery>,
) -> AppResult<Json<Items<VatCheck>>> {
    Ok(Items::new(
        vat_checks::list_for_subject(&state.db, &q.subject_type, q.subject_id).await?,
    ))
}

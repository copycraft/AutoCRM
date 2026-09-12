//! "MiniCRM eredeti adatok" — the source record behind a migrated row (V1.5).
//!
//! Its own module rather than three copies in partners/leads/orders: this is migration
//! provenance, and it should stay one thing that can be removed in one piece once the
//! MiniCRM account is finally retired.
//!
//! Deliberately a separate request instead of a field on the detail responses: the JSONB
//! is the whole source record, and most screen loads do not want it.

use axum::Json;
use axum::extract::State;
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiPath, Auth};
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::repo::raw_import;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(partner_raw_import))
        .routes(routes!(lead_raw_import))
        .routes(routes!(order_raw_import))
}

#[derive(Serialize, ToSchema)]
pub struct RawImportView {
    /// The MiniCRM id this row came from; null for records created in AutoCRM.
    minicrm_id: Option<i64>,
    /// The complete source record as MiniCRM returned it. Null for records created in
    /// AutoCRM — the screen shows nothing rather than an empty panel.
    #[schema(value_type = Option<Object>)]
    raw_import: Option<Value>,
}

fn view(
    found: Option<Option<raw_import::RawImport>>,
    what: &'static str,
) -> AppResult<Json<RawImportView>> {
    let row = found.ok_or(AppError::NotFound(what))?;
    Ok(Json(match row {
        Some(r) => RawImportView {
            minicrm_id: Some(r.minicrm_id),
            raw_import: Some(r.raw_import),
        },
        None => RawImportView {
            minicrm_id: None,
            raw_import: None,
        },
    }))
}

#[utoipa::path(
    get, path = "/partners/{id}/raw-import", tag = "migration",
    params(("id" = i64, Path)),
    responses((status = 200, body = RawImportView))
)]
async fn partner_raw_import(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<RawImportView>> {
    view(raw_import::for_partner(&state.db, id).await?, "partner")
}

#[utoipa::path(
    get, path = "/leads/{id}/raw-import", tag = "migration",
    params(("id" = i64, Path)),
    responses((status = 200, body = RawImportView))
)]
async fn lead_raw_import(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<RawImportView>> {
    view(raw_import::for_lead(&state.db, id).await?, "lead")
}

#[utoipa::path(
    get, path = "/orders/{id}/raw-import", tag = "migration",
    params(("id" = i64, Path)),
    responses((status = 200, body = RawImportView))
)]
async fn order_raw_import(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<RawImportView>> {
    view(raw_import::for_order(&state.db, id).await?, "order")
}

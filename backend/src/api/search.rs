//! Global search (`GET /api/search`).
//!
//! One input, three aggregates, one round trip: orders (number, title, partner, plate,
//! VIN), partners (name, city, tax number), leads (title, contact, partner). Each group
//! is capped so one chatty table cannot drown the others; ranking inside a group is
//! recency, cross-group ordering is the client's job. Reads are open to every
//! authenticated user, like every other list endpoint.

use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiQuery, Auth};
use crate::AppState;
use crate::error::AppResult;
use crate::repo::search::{self, LeadHit, OrderHit, PartnerHit};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(global))
}

/// Per-group cap: a header box shows a handful of each, not pages.
const PER_GROUP: i64 = 8;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct GlobalQuery {
    /// At least 2 characters; shorter input returns empty groups rather than an error,
    /// so the UI can query on every keystroke without special-casing.
    q: Option<String>,
}

#[derive(serde::Serialize, ToSchema)]
struct GlobalResults {
    orders: Vec<OrderHit>,
    partners: Vec<PartnerHit>,
    leads: Vec<LeadHit>,
}

#[utoipa::path(
    get, path = "/search", tag = "search",
    params(GlobalQuery),
    responses((status = 200, body = GlobalResults))
)]
async fn global(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<GlobalQuery>,
) -> AppResult<Json<GlobalResults>> {
    let needle = q.q.as_deref().unwrap_or("").trim();
    if needle.chars().count() < 2 {
        return Ok(Json(GlobalResults {
            orders: Vec::new(),
            partners: Vec::new(),
            leads: Vec::new(),
        }));
    }
    Ok(Json(GlobalResults {
        orders: search::orders(&state.db, needle, PER_GROUP).await?,
        partners: search::partners(&state.db, needle, PER_GROUP).await?,
        leads: search::leads(&state.db, needle, PER_GROUP).await?,
    }))
}

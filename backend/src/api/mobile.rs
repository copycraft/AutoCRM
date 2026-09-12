//! The Android photo app needs exactly one thing beyond auth and uploads: a small order
//! picker it can cache offline.

use axum::Json;
use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::{ApiQuery, Auth};
use crate::AppState;
use crate::domain::order::normalize_plate;
use crate::error::AppResult;
use crate::repo::like_pattern;
use crate::repo::orders::{self, OrderFilter};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(order_picker))
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

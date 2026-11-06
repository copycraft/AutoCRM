//! How many records sit in each status, for the status list beside a record list — the way
//! the invoice pages show their buckets: the count at a glance, a click filters the list.
//!
//! Counts are over everything, not the list's other filters: the list says where things
//! stand, and narrowing it by assignee or search must not make a stage look empty.

use axum::Json;
use axum::extract::State;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::Auth;
use crate::AppState;
use crate::domain::role::Capability;
use crate::error::AppResult;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(lead_stage_counts))
        .routes(routes!(order_stage_counts))
        .routes(routes!(email_status_counts))
}

/// One status and how many records are in it now.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct StatusCount {
    /// The stage key (`GET /config/stages`) or e-mail status.
    pub key: String,
    pub count: i64,
}

#[utoipa::path(
    get, path = "/leads/stage-counts", tag = "leads",
    responses((status = 200, description = "Leads per current stage; stages with none are absent", body = Items<StatusCount>))
)]
async fn lead_stage_counts(
    State(state): State<AppState>,
    Auth(me): Auth,
) -> AppResult<Json<Items<StatusCount>>> {
    me.require(Capability::ViewSales)?;
    let rows = sqlx::query_as::<_, StatusCount>(
        "SELECT stage_key AS key, count(*) AS count FROM lead_current_stage GROUP BY stage_key",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Items::new(rows))
}

#[utoipa::path(
    get, path = "/orders/stage-counts", tag = "orders",
    responses((status = 200, description = "Orders per current stage; stages with none are absent", body = Items<StatusCount>))
)]
async fn order_stage_counts(
    State(state): State<AppState>,
    Auth(_): Auth,
) -> AppResult<Json<Items<StatusCount>>> {
    // Every signed-in user reads orders; a count carries no price.
    let rows = sqlx::query_as::<_, StatusCount>(
        "SELECT stage_key AS key, count(*) AS count FROM order_current_stage GROUP BY stage_key",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Items::new(rows))
}

#[utoipa::path(
    get, path = "/emails/status-counts", tag = "email",
    responses((status = 200, description = "E-mails per status; statuses with none are absent", body = Items<StatusCount>))
)]
async fn email_status_counts(
    State(state): State<AppState>,
    Auth(me): Auth,
) -> AppResult<Json<Items<StatusCount>>> {
    me.require(Capability::ViewSales)?;
    let rows = sqlx::query_as::<_, StatusCount>(
        "SELECT status::text AS key, count(*) AS count FROM email_messages GROUP BY status",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Items::new(rows))
}

//! The signed-in user's notification feed. Every route is scoped to the caller: nobody reads
//! or marks anyone else's.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiQuery, Auth};
use super::page_limit;
use crate::AppState;
use crate::error::AppResult;
use crate::repo::notifications::{self, NotificationRow};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(mark_read))
        .routes(routes!(mark_all_read))
}

#[derive(Serialize, ToSchema)]
pub struct Notification {
    pub id: i64,
    /// What happened, e.g. `lead`.
    pub kind: String,
    pub title: String,
    pub body: Option<String>,
    /// App path to open, e.g. `/leads/42`.
    pub link: Option<String>,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

impl From<NotificationRow> for Notification {
    fn from(r: NotificationRow) -> Self {
        Notification {
            id: r.id,
            kind: r.kind,
            title: r.title,
            body: r.body,
            link: r.link,
            created_at: r.created_at,
            read_at: r.read_at,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct NotificationList {
    /// Newest first.
    pub items: Vec<Notification>,
    /// Unread notifications in total, not just in this page.
    pub unread: i64,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct ListQuery {
    /// Only notifications with a higher id: what a client that remembers the last one it saw
    /// asks for.
    after_id: Option<i64>,
    #[serde(default)]
    unread_only: bool,
    limit: Option<i64>,
}

#[utoipa::path(
    get, path = "/notifications", tag = "notifications",
    params(ListQuery),
    responses((status = 200, body = NotificationList))
)]
async fn list(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> AppResult<Json<NotificationList>> {
    let rows = notifications::list(
        &state.db,
        me.user_id,
        q.after_id,
        q.unread_only,
        page_limit(q.limit),
    )
    .await?;
    let unread = notifications::unread_count(&state.db, me.user_id).await?;
    Ok(Json(NotificationList {
        items: rows.into_iter().map(Notification::from).collect(),
        unread,
    }))
}

#[derive(Deserialize, ToSchema)]
struct MarkRead {
    ids: Vec<i64>,
}

#[utoipa::path(
    post, path = "/notifications/read", tag = "notifications",
    request_body = MarkRead,
    responses((status = 204, description = "Marked read (ids that are not yours are ignored)"))
)]
async fn mark_read(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<MarkRead>,
) -> AppResult<StatusCode> {
    notifications::mark_read(&state.db, me.user_id, Some(&b.ids)).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post, path = "/notifications/read-all", tag = "notifications",
    responses((status = 204, description = "Everything marked read"))
)]
async fn mark_all_read(State(state): State<AppState>, Auth(me): Auth) -> AppResult<StatusCode> {
    notifications::mark_read(&state.db, me.user_id, None).await?;
    Ok(StatusCode::NO_CONTENT)
}

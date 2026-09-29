//! Follow-up tasks (`GET /tasks`, per-record lists, done toggles, delete).
//!
//! No capability gate: any authenticated user may create and toggle reminders, and
//! `created_by` records who said so. What IS validated: the entity type is one of the
//! three, the referenced record exists (no dangling pins), and the assignee, if named,
//! is a real user — a foreign-key 500 is never the answer to a typo.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::NaiveDate;
use serde::Deserialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, Auth};
use super::{Items, required};
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::repo::tasks::Task;
use crate::repo::{leads, orders, partners, tasks, users};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(mine, create))
        .routes(routes!(for_entity))
        .routes(routes!(set_done))
        .routes(routes!(remove))
}

/// The caller's open tasks: assigned to them or created by them, most urgent first.
/// This is what the dashboard widget reads.
#[utoipa::path(
    get, path = "/tasks", tag = "tasks",
    responses((status = 200, body = Items<Task>))
)]
async fn mine(State(state): State<AppState>, Auth(me): Auth) -> AppResult<Json<Items<Task>>> {
    Ok(Items::new(
        tasks::open_for_user(&state.db, me.user_id).await?,
    ))
}

#[derive(Deserialize, ToSchema)]
struct TaskBody {
    /// `order` | `lead` | `partner`.
    entity_type: String,
    entity_id: i64,
    title: String,
    due_date: Option<NaiveDate>,
    assigned_to: Option<i64>,
}

async fn check_target(db: &sqlx::PgPool, entity_type: &str, entity_id: i64) -> AppResult<()> {
    let exists = match entity_type {
        "order" => orders::find(db, entity_id).await?.is_some(),
        "lead" => leads::find(db, entity_id).await?.is_some(),
        "partner" => partners::find(db, entity_id).await?.is_some(),
        other => {
            return Err(AppError::validation(format!(
                "entity_type must be order, lead or partner, not '{other}'"
            )));
        }
    };
    if !exists {
        return Err(AppError::validation("the pinned record does not exist"));
    }
    Ok(())
}

#[utoipa::path(
    post, path = "/tasks", tag = "tasks",
    request_body(content = TaskBody),
    responses((status = 201, body = Task))
)]
async fn create(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<TaskBody>,
) -> AppResult<(StatusCode, Json<Task>)> {
    let title = required("title", &b.title)?;
    if title.chars().count() > 200 {
        return Err(AppError::validation("title is at most 200 characters"));
    }
    check_target(&state.db, &b.entity_type, b.entity_id).await?;
    if let Some(uid) = b.assigned_to {
        if users::find(&state.db, uid).await?.is_none() {
            return Err(AppError::validation("assigned_to is not a user"));
        }
    }
    let task = tasks::create(
        &state.db,
        &tasks::NewTask {
            entity_type: b.entity_type,
            entity_id: b.entity_id,
            title,
            due_date: b.due_date,
            assigned_to: b.assigned_to,
            created_by: me.user_id,
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(task)))
}

#[utoipa::path(
    get, path = "/tasks/for/{entity}/{id}", tag = "tasks",
    params(("entity" = String, Path), ("id" = i64, Path)),
    responses((status = 200, body = Items<Task>))
)]
async fn for_entity(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath((entity, id)): ApiPath<(String, i64)>,
) -> AppResult<Json<Items<Task>>> {
    if !matches!(entity.as_str(), "order" | "lead" | "partner") {
        return Err(AppError::validation(
            "entity must be order, lead or partner",
        ));
    }
    Ok(Items::new(tasks::for_entity(&state.db, &entity, id).await?))
}

#[derive(Deserialize, ToSchema)]
struct DoneBody {
    done: bool,
}

#[utoipa::path(
    post, path = "/tasks/{id}/done", tag = "tasks",
    params(("id" = i64, Path)),
    request_body(content = DoneBody),
    responses((status = 200, body = Task))
)]
async fn set_done(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<DoneBody>,
) -> AppResult<Json<Task>> {
    tasks::set_done(&state.db, id, b.done)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound("task"))
}

#[utoipa::path(
    delete, path = "/tasks/{id}", tag = "tasks",
    params(("id" = i64, Path)),
    responses((status = 204))
)]
async fn remove(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    if !tasks::remove(&state.db, id).await? {
        return Err(AppError::NotFound("task"));
    }
    Ok(StatusCode::NO_CONTENT)
}

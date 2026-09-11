//! Blockers: "we are waiting on a thing from someone".

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::json;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::{Items, optional, page_limit, page_offset, patch as patch_field, patch_text, required};
use crate::AppState;
use crate::domain::email::normalize_address;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::blockers::{Blocker, BlockerInput};
use crate::repo::{audit, blockers, orders, partners};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/blockers", get(list_open))
        .route("/orders/{id}/blockers", get(list_for_order).post(create))
        .route("/blockers/{id}", patch(update))
        .route("/blockers/{id}/resolve", post(resolve))
        .route("/blockers/{id}/reopen", post(reopen))
}

#[derive(Deserialize)]
struct OpenQuery {
    responsible_partner_id: Option<i64>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn list_open(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiQuery(q): ApiQuery<OpenQuery>,
) -> AppResult<Json<Items<Blocker>>> {
    let rows = blockers::list_open(
        &state.db,
        q.responsible_partner_id,
        page_limit(q.limit),
        page_offset(q.offset),
    )
    .await?;
    Ok(Items::new(rows))
}

async fn list_for_order(
    State(state): State<AppState>,
    Auth(_): Auth,
    ApiPath(order_id): ApiPath<i64>,
) -> AppResult<Json<Items<Blocker>>> {
    Ok(Items::new(
        blockers::list_for_order(&state.db, order_id).await?,
    ))
}

#[derive(Deserialize)]
struct BlockerBody {
    what: Option<String>,
    #[serde(default, deserialize_with = "patch_field")]
    responsible_partner_id: Option<Option<i64>>,
    #[serde(default, deserialize_with = "patch_field")]
    responsible_email: Option<Option<String>>,
    #[serde(default, deserialize_with = "patch_field")]
    due_date: Option<Option<NaiveDate>>,
    #[serde(default, deserialize_with = "patch_field")]
    notes: Option<Option<String>>,
    nudge_enabled: Option<bool>,
}

fn merge(current: Option<&Blocker>, b: BlockerBody) -> AppResult<BlockerInput> {
    let what = b
        .what
        .or_else(|| current.map(|c| c.what.clone()))
        .unwrap_or_default();
    let email = patch_text(
        &current.and_then(|c| c.responsible_email.clone()),
        b.responsible_email,
    );
    let responsible_email = match email {
        Some(e) => Some(
            normalize_address(&e)
                .ok_or_else(|| AppError::validation("responsible_email is not a valid address"))?,
        ),
        None => None,
    };
    Ok(BlockerInput {
        what: required("what", &what)?,
        responsible_partner_id: b
            .responsible_partner_id
            .unwrap_or(current.and_then(|c| c.responsible_partner_id)),
        responsible_email,
        due_date: b.due_date.unwrap_or(current.and_then(|c| c.due_date)),
        notes: patch_text(&current.and_then(|c| c.notes.clone()), b.notes),
        nudge_enabled: b
            .nudge_enabled
            .or(current.map(|c| c.nudge_enabled))
            .unwrap_or(true),
    })
}

async fn create(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(order_id): ApiPath<i64>,
    ApiJson(b): ApiJson<BlockerBody>,
) -> AppResult<(StatusCode, Json<Blocker>)> {
    me.require(Capability::ManageBlockers)?;
    let input = merge(None, b)?;
    let mut tx = state.db.begin().await?;
    orders::lock(&mut *tx, order_id)
        .await?
        .ok_or(AppError::NotFound("order"))?;
    if let Some(pid) = input.responsible_partner_id {
        partners::find(&mut *tx, pid)
            .await?
            .ok_or_else(|| AppError::validation("responsible partner does not exist"))?;
    }
    let id = blockers::insert(&mut *tx, order_id, &input, me.user_id).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "order",
        order_id,
        "blocker_add",
        json!({ "blocker_id": id, "what": input.what }),
    )
    .await?;
    let blocker = blockers::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("blocker"))?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(blocker)))
}

async fn update(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<BlockerBody>,
) -> AppResult<Json<Blocker>> {
    me.require(Capability::ManageBlockers)?;
    let mut tx = state.db.begin().await?;
    let current = blockers::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("blocker"))?;
    let input = merge(Some(&current), b)?;
    if let Some(pid) = input.responsible_partner_id {
        partners::find(&mut *tx, pid)
            .await?
            .ok_or_else(|| AppError::validation("responsible partner does not exist"))?;
    }
    blockers::update(&mut *tx, id, &input).await?;
    let updated = blockers::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("blocker"))?;
    let changes = audit::diff(&[
        ("what", json!(current.what), json!(updated.what)),
        (
            "responsible_partner_id",
            json!(current.responsible_partner_id),
            json!(updated.responsible_partner_id),
        ),
        (
            "responsible_email",
            json!(current.responsible_email),
            json!(updated.responsible_email),
        ),
        ("due_date", json!(current.due_date), json!(updated.due_date)),
        ("notes", json!(current.notes), json!(updated.notes)),
        (
            "nudge_enabled",
            json!(current.nudge_enabled),
            json!(updated.nudge_enabled),
        ),
    ]);
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "order",
        current.order_id,
        "blocker_update",
        json!({ "blocker_id": id, "changes": changes }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(updated))
}

#[derive(Deserialize)]
struct ResolveBody {
    note: Option<String>,
}

async fn resolve(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<ResolveBody>,
) -> AppResult<Json<Blocker>> {
    me.require(Capability::ManageBlockers)?;
    let note = optional(b.note);
    let mut tx = state.db.begin().await?;
    let current = blockers::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("blocker"))?;
    if !blockers::resolve(&mut *tx, id, me.user_id, note.as_deref()).await? {
        return Err(AppError::conflict(
            "already_resolved",
            "blocker is already resolved",
        ));
    }
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "order",
        current.order_id,
        "blocker_resolve",
        json!({ "blocker_id": id, "note": note }),
    )
    .await?;
    let updated = blockers::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("blocker"))?;
    tx.commit().await?;
    Ok(Json(updated))
}

async fn reopen(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<Blocker>> {
    me.require(Capability::ManageBlockers)?;
    let mut tx = state.db.begin().await?;
    let current = blockers::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("blocker"))?;
    if !blockers::reopen(&mut *tx, id).await? {
        return Err(AppError::conflict(
            "not_resolved",
            "blocker is not resolved",
        ));
    }
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "order",
        current.order_id,
        "blocker_reopen",
        json!({ "blocker_id": id }),
    )
    .await?;
    let updated = blockers::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("blocker"))?;
    tx.commit().await?;
    Ok(Json(updated))
}

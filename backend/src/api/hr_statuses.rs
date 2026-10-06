//! HR status lists: where each employee stands (absent, waiting to be filed, active and
//! which papers are missing, left). Everything here needs `AccessHr`, like the rest of HR.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, ApiQuery, Auth};
use super::hr::{Employee, present};
use super::{Items, required};
use crate::AppState;
use crate::domain::lead_tag::normalize_color;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::employee_statuses::{self, EmployeeStatus, StatusInput};
use crate::repo::{audit, employees};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_statuses, create_status))
        .routes(routes!(update_status))
        .routes(routes!(reorder_statuses))
        .routes(routes!(set_employee_status))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
struct StatusesQuery {
    #[serde(default)]
    archived: bool,
}

#[utoipa::path(
    get, path = "/hr/statuses", tag = "hr",
    params(StatusesQuery),
    responses((status = 200, body = Items<EmployeeStatus>))
)]
async fn list_statuses(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiQuery(q): ApiQuery<StatusesQuery>,
) -> AppResult<Json<Items<EmployeeStatus>>> {
    me.require(Capability::AccessHr)?;
    Ok(Items::new(employee_statuses::list(&state.db, q.archived).await?))
}

#[derive(Deserialize, ToSchema)]
struct CreateStatus {
    section: String,
    label: String,
    color: Option<String>,
}

#[derive(Deserialize, ToSchema)]
struct UpdateStatus {
    section: Option<String>,
    label: Option<String>,
    color: Option<String>,
    archived: Option<bool>,
}

fn input(section: &str, label: &str, color: &str) -> AppResult<StatusInput> {
    let section = required("section", section)?;
    let label = required("label", label)?;
    if section.chars().count() > 60 || label.chars().count() > 100 {
        return Err(AppError::validation("section or label is too long"));
    }
    let color = normalize_color(color)
        .ok_or_else(|| AppError::validation("color must look like #a33122"))?;
    Ok(StatusInput {
        section,
        label,
        color,
    })
}

#[utoipa::path(
    post, path = "/hr/statuses", tag = "hr",
    request_body = CreateStatus,
    responses((status = 201, body = EmployeeStatus))
)]
async fn create_status(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<CreateStatus>,
) -> AppResult<(StatusCode, Json<EmployeeStatus>)> {
    me.require(Capability::AccessHr)?;
    let s = input(&b.section, &b.label, b.color.as_deref().unwrap_or("#dde1e6"))?;
    let id = employee_statuses::insert(&state.db, &s).await?;
    audit::record(&state.db, Some(me.user_id), "employee_status", id, "create",
        json!({ "section": s.section, "label": s.label })).await?;
    let row = employee_statuses::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("employee status"))?;
    Ok((StatusCode::CREATED, Json(row)))
}

#[utoipa::path(
    patch, path = "/hr/statuses/{id}", tag = "hr",
    params(("id" = i64, Path)),
    request_body = UpdateStatus,
    responses((status = 200, body = EmployeeStatus), (status = 422, description = "The default or the ended status cannot be archived"))
)]
async fn update_status(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<UpdateStatus>,
) -> AppResult<Json<EmployeeStatus>> {
    me.require(Capability::AccessHr)?;
    let current = employee_statuses::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("employee status"))?;
    let archived = b.archived.unwrap_or(current.archived_at.is_some());
    if archived && (current.is_default || current.ends_employment) {
        return Err(AppError::rule(
            "status_required",
            "new employees start on this status and leavers land on that one: rename it instead",
        ));
    }
    let s = input(
        b.section.as_deref().unwrap_or(&current.section),
        b.label.as_deref().unwrap_or(&current.label),
        b.color.as_deref().unwrap_or(&current.color),
    )?;
    employee_statuses::update(&state.db, id, &s, archived).await?;
    let changes = audit::diff(&[
        ("section", json!(current.section), json!(s.section)),
        ("label", json!(current.label), json!(s.label)),
        ("color", json!(current.color), json!(s.color)),
        ("archived", json!(current.archived_at.is_some()), json!(archived)),
    ]);
    audit::record(&state.db, Some(me.user_id), "employee_status", id, "update", changes).await?;
    let row = employee_statuses::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("employee status"))?;
    Ok(Json(row))
}

#[derive(Deserialize, ToSchema)]
struct ReorderStatuses {
    section: String,
    ids: Vec<i64>,
}

#[utoipa::path(
    put, path = "/hr/statuses/order", tag = "hr",
    request_body = ReorderStatuses,
    responses((status = 204, description = "Reordered"))
)]
async fn reorder_statuses(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<ReorderStatuses>,
) -> AppResult<StatusCode> {
    me.require(Capability::AccessHr)?;
    employee_statuses::reorder(&state.db, b.section.trim(), &b.ids).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
struct EmployeeStatusBody {
    status_id: i64,
}

/// Moves the employee to a status. Megszűnt jogviszony (a status that ends employment)
/// archives them; any other brings an archived employee back.
#[utoipa::path(
    put, path = "/hr/employees/{id}/status", tag = "hr",
    params(("id" = i64, Path)),
    request_body = EmployeeStatusBody,
    responses((status = 200, body = Employee))
)]
async fn set_employee_status(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<EmployeeStatusBody>,
) -> AppResult<Json<Employee>> {
    me.require(Capability::AccessHr)?;
    let status = employee_statuses::find(&state.db, b.status_id)
        .await?
        .filter(|s| s.archived_at.is_none())
        .ok_or_else(|| AppError::validation("the status does not exist or is archived"))?;
    let mut tx = state.db.begin().await?;
    let before = employees::lock(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    let row = employees::set_status(&mut *tx, id, status.id)
        .await?
        .ok_or(AppError::NotFound("employee"))?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "employee",
        id,
        "status",
        json!({ "from": before.status_id, "to": status.id, "label": status.label }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(present(&state, row).await))
}

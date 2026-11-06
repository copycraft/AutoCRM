//! Departments and the technical content split by them (0059).
//!
//! Who may do what is decided in `service::modules`: everyone reads, the office writes
//! every module, a department's members write their own. Departments themselves are
//! configuration (`ManageConfiguration`); who works where is user administration
//! (`ManageUsers`).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::{ApiJson, ApiPath, Auth};
use crate::AppState;
use crate::domain::role::Capability;
use crate::error::{AppError, AppResult};
use crate::repo::departments::{self, Department, Membership};
use crate::repo::order_modules::{self, ModuleFile, OrderModule};
use crate::repo::{audit, orders};
use crate::service::auth::AuthUser;
use crate::service::modules;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_departments, create_department))
        .routes(routes!(update_department))
        .routes(routes!(list_memberships))
        .routes(routes!(set_user_departments))
        .routes(routes!(list_modules, create_module))
        .routes(routes!(update_module, delete_module))
        .routes(routes!(release_module))
        .routes(routes!(add_module_file))
        .routes(routes!(remove_module_file))
}

fn valid_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

// ── Departments ─────────────────────────────────────────────────────────────

/// Every department, active or not, in their order.
#[utoipa::path(
    get, path = "/departments", tag = "modules",
    responses((status = 200, body = Items<Department>))
)]
async fn list_departments(
    State(state): State<AppState>,
    Auth(_): Auth,
) -> AppResult<Json<Items<Department>>> {
    Ok(Items::new(departments::list(&state.db).await?))
}

#[derive(Deserialize, ToSchema)]
struct NewDepartment {
    /// Lower-case letters, digits and `_`; never changes.
    key: String,
    name: String,
    /// `#rrggbb`.
    color: String,
    #[serde(default)]
    position: i32,
}

#[utoipa::path(
    post, path = "/departments", tag = "modules",
    request_body = NewDepartment,
    responses((status = 201, body = Department))
)]
async fn create_department(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(b): ApiJson<NewDepartment>,
) -> AppResult<(StatusCode, Json<Department>)> {
    me.require(Capability::ManageConfiguration)?;
    let key = b.key.trim();
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(AppError::validation(
            "key must be lower-case letters, digits and _",
        ));
    }
    if b.name.trim().is_empty() {
        return Err(AppError::validation("name is required"));
    }
    if !valid_color(&b.color) {
        return Err(AppError::validation("color must be #rrggbb"));
    }
    let created = departments::insert(&state.db, key, b.name.trim(), &b.color, b.position).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[derive(Deserialize, ToSchema)]
struct DepartmentPatch {
    name: Option<String>,
    color: Option<String>,
    position: Option<i32>,
    is_active: Option<bool>,
}

#[utoipa::path(
    patch, path = "/departments/{id}", tag = "modules",
    params(("id" = i64, Path)),
    request_body = DepartmentPatch,
    responses((status = 200, body = Department))
)]
async fn update_department(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<DepartmentPatch>,
) -> AppResult<Json<Department>> {
    me.require(Capability::ManageConfiguration)?;
    let mut d = departments::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("department"))?;
    if let Some(name) = b.name {
        if name.trim().is_empty() {
            return Err(AppError::validation("name is required"));
        }
        d.name = name.trim().to_string();
    }
    if let Some(color) = b.color {
        if !valid_color(&color) {
            return Err(AppError::validation("color must be #rrggbb"));
        }
        d.color = color;
    }
    if let Some(position) = b.position {
        d.position = position;
    }
    if let Some(active) = b.is_active {
        d.is_active = active;
    }
    departments::update(&state.db, &d)
        .await?
        .map(Json)
        .ok_or(AppError::NotFound("department"))
}

/// Who works in which department, and who heads it.
#[utoipa::path(
    get, path = "/departments/members", tag = "modules",
    responses((status = 200, body = Items<Membership>))
)]
async fn list_memberships(
    State(state): State<AppState>,
    Auth(_): Auth,
) -> AppResult<Json<Items<Membership>>> {
    Ok(Items::new(departments::memberships(&state.db).await?))
}

#[derive(Deserialize, ToSchema)]
struct DepartmentSeat {
    department_id: i64,
    #[serde(default)]
    is_head: bool,
}

#[derive(Deserialize, ToSchema)]
struct UserDepartments {
    /// Every department the person works in; departments left out are left.
    departments: Vec<DepartmentSeat>,
}

/// Set the departments one person works in, and which of them they head.
#[utoipa::path(
    put, path = "/users/{id}/departments", tag = "modules",
    params(("id" = i64, Path)),
    request_body = UserDepartments,
    responses((status = 204))
)]
async fn set_user_departments(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<UserDepartments>,
) -> AppResult<StatusCode> {
    me.require(Capability::ManageUsers)?;
    let known: Vec<i64> = departments::list(&state.db)
        .await?
        .into_iter()
        .map(|d| d.id)
        .collect();
    let mut seats: Vec<(i64, bool)> = Vec::new();
    for seat in &b.departments {
        if !known.contains(&seat.department_id) {
            return Err(AppError::validation("a department does not exist"));
        }
        if seats.iter().any(|(d, _)| *d == seat.department_id) {
            return Err(AppError::validation("a department is listed twice"));
        }
        seats.push((seat.department_id, seat.is_head));
    }
    let mut tx = state.db.begin().await?;
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE id = $1)")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    if !exists {
        return Err(AppError::NotFound("user"));
    }
    departments::set_for_user(&mut tx, id, &seats).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "user",
        id,
        "departments",
        json!({ "departments": seats }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Modules ─────────────────────────────────────────────────────────────────

/// A module as the screen shows it: the text, its files, and what this person may do.
#[derive(Serialize, ToSchema)]
pub struct ModuleView {
    #[serde(flatten)]
    pub module: OrderModule,
    pub files: Vec<ModuleFile>,
    /// Released, then written to again: the department has an older version.
    pub changed_since_release: bool,
    /// Whether the caller may write this module (office, or a member of the department).
    pub can_edit: bool,
}

/// The order's technical content, plus which departments the caller may start a module for.
#[derive(Serialize, ToSchema)]
pub struct OrderModules {
    pub items: Vec<ModuleView>,
    pub editable_department_ids: Vec<i64>,
}

async fn views(state: &AppState, me: &AuthUser, order_id: i64) -> AppResult<OrderModules> {
    let mut conn = state.db.acquire().await?;
    let mut mods = order_modules::list_for_order(&mut *conn, order_id).await?;
    // Without `ViewAllModules` a person sees only their own departments' parts: the
    // welder gets the metalwork, not the refrigeration or the body (0061).
    if !me.can(Capability::ViewAllModules) {
        let mine = departments::of_user(&mut *conn, me.user_id).await?;
        mods.retain(|m| mine.contains(&m.department_id));
    }
    let files = order_modules::files_for_order(&mut *conn, order_id).await?;
    let all = departments::list(&mut *conn).await?;
    let mut editable = Vec::new();
    for d in all.iter().filter(|d| d.is_active) {
        if modules::can_edit(&mut conn, me, d.id).await? {
            editable.push(d.id);
        }
    }
    let mut items = Vec::with_capacity(mods.len());
    for module in mods {
        let can_edit = modules::can_edit(&mut conn, me, module.department_id).await?;
        items.push(ModuleView {
            files: files
                .iter()
                .filter(|f| f.module_id == module.id)
                .cloned()
                .collect(),
            changed_since_release: module.changed_since_release(),
            can_edit,
            module,
        });
    }
    Ok(OrderModules {
        items,
        editable_department_ids: editable,
    })
}

async fn view(state: &AppState, me: &AuthUser, module_id: i64) -> AppResult<ModuleView> {
    let module = order_modules::find(&state.db, module_id)
        .await?
        .ok_or(AppError::NotFound("module"))?;
    views(state, me, module.order_id)
        .await?
        .items
        .into_iter()
        .find(|v| v.module.id == module_id)
        .ok_or(AppError::NotFound("module"))
}

#[utoipa::path(
    get, path = "/orders/{id}/modules", tag = "modules",
    params(("id" = i64, Path)),
    responses((status = 200, body = OrderModules))
)]
async fn list_modules(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<OrderModules>> {
    if orders::find(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound("order"));
    }
    Ok(Json(views(&state, &me, id).await?))
}

#[derive(Deserialize, ToSchema)]
struct NewModule {
    department_id: i64,
    #[serde(default)]
    content: String,
}

/// Start a department's part of the order's technical content (or get the one it has).
#[utoipa::path(
    post, path = "/orders/{id}/modules", tag = "modules",
    params(("id" = i64, Path)),
    request_body = NewModule,
    responses((status = 201, body = ModuleView))
)]
async fn create_module(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<NewModule>,
) -> AppResult<(StatusCode, Json<ModuleView>)> {
    let mut tx = state.db.begin().await?;
    if orders::find(&mut *tx, id).await?.is_none() {
        return Err(AppError::NotFound("order"));
    }
    let department = departments::find(&mut *tx, b.department_id)
        .await?
        .filter(|d| d.is_active)
        .ok_or_else(|| AppError::validation("the department does not exist"))?;
    modules::require_edit(&mut tx, &me, department.id).await?;
    let module_id = order_modules::insert(
        &mut *tx,
        id,
        department.id,
        b.content.trim_end(),
        me.user_id,
    )
    .await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "order",
        id,
        "module_add",
        json!({ "module_id": module_id, "department": department.name }),
    )
    .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(view(&state, &me, module_id).await?),
    ))
}

#[derive(Deserialize, ToSchema)]
struct ModulePatch {
    content: Option<String>,
    /// `draft` or `done`. Releasing is `POST /modules/{id}/release`, which tells the heads.
    status: Option<String>,
}

#[utoipa::path(
    patch, path = "/modules/{id}", tag = "modules",
    params(("id" = i64, Path)),
    request_body = ModulePatch,
    responses((status = 200, body = ModuleView))
)]
async fn update_module(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<ModulePatch>,
) -> AppResult<Json<ModuleView>> {
    let mut tx = state.db.begin().await?;
    let module = order_modules::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("module"))?;
    modules::require_edit(&mut tx, &me, module.department_id).await?;
    if let Some(content) = &b.content {
        order_modules::set_content(&mut *tx, id, content.trim_end(), me.user_id).await?;
    }
    if let Some(status) = &b.status {
        if !matches!(status.as_str(), "draft" | "done") {
            return Err(AppError::validation(
                "status must be draft or done; release with POST /modules/{id}/release",
            ));
        }
        order_modules::set_status(&mut *tx, id, status, me.user_id).await?;
    }
    if b.content.is_some() || b.status.is_some() {
        audit::record(
            &mut *tx,
            Some(me.user_id),
            "order",
            module.order_id,
            "module_update",
            json!({
                "module_id": id,
                "department": module.department_name,
                "status": b.status,
                "content_changed": b.content.as_deref().is_some_and(|c| c.trim_end() != module.content),
            }),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Json(view(&state, &me, id).await?))
}

/// Remove a department's part from the order. Office only: it may be what a department
/// is building from.
#[utoipa::path(
    delete, path = "/modules/{id}", tag = "modules",
    params(("id" = i64, Path)),
    responses((status = 204))
)]
async fn delete_module(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::EditOrders)?;
    let mut tx = state.db.begin().await?;
    let module = order_modules::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("module"))?;
    order_modules::delete(&mut *tx, id).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "order",
        module.order_id,
        "module_remove",
        json!({ "module_id": id, "department": module.department_name }),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Hand the module to its department: the heads get a notification and a letter. Released
/// and written to since, it goes out again as a change.
#[utoipa::path(
    post, path = "/modules/{id}/release", tag = "modules",
    params(("id" = i64, Path)),
    responses((status = 200, body = ModuleView))
)]
async fn release_module(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<ModuleView>> {
    modules::release(&state, &me, id).await?;
    Ok(Json(view(&state, &me, id).await?))
}

#[derive(Deserialize, ToSchema)]
struct ModuleFileBody {
    /// One of the order's photos, or…
    image_id: Option<i64>,
    /// …one of the order's documents. Exactly one of the two.
    document_id: Option<i64>,
}

/// Put one of the order's drawings or photos on the module's sheet.
#[utoipa::path(
    post, path = "/modules/{id}/files", tag = "modules",
    params(("id" = i64, Path)),
    request_body = ModuleFileBody,
    responses((status = 200, body = ModuleView))
)]
async fn add_module_file(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(b): ApiJson<ModuleFileBody>,
) -> AppResult<Json<ModuleView>> {
    let mut tx = state.db.begin().await?;
    let module = order_modules::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("module"))?;
    modules::require_edit(&mut tx, &me, module.department_id).await?;
    let owner: Option<Option<i64>> = match (b.image_id, b.document_id) {
        (Some(image), None) => {
            sqlx::query_scalar("SELECT order_id FROM images WHERE id = $1 AND deleted_at IS NULL")
                .bind(image)
                .fetch_optional(&mut *tx)
                .await?
                .map(Some)
        }
        (None, Some(doc)) => {
            sqlx::query_scalar(
                "SELECT order_id FROM documents WHERE id = $1 AND deleted_at IS NULL",
            )
            .bind(doc)
            .fetch_optional(&mut *tx)
            .await?
        }
        _ => {
            return Err(AppError::validation(
                "give exactly one of image_id and document_id",
            ));
        }
    };
    if owner.flatten() != Some(module.order_id) {
        return Err(AppError::validation(
            "the file must be one of this order's files",
        ));
    }
    order_modules::add_file(&mut *tx, id, b.image_id, b.document_id).await?;
    tx.commit().await?;
    Ok(Json(view(&state, &me, id).await?))
}

#[utoipa::path(
    delete, path = "/modules/{id}/files/{file_id}", tag = "modules",
    params(("id" = i64, Path), ("file_id" = i64, Path)),
    responses((status = 200, body = ModuleView))
)]
async fn remove_module_file(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath((id, file_id)): ApiPath<(i64, i64)>,
) -> AppResult<Json<ModuleView>> {
    let mut tx = state.db.begin().await?;
    let module = order_modules::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("module"))?;
    modules::require_edit(&mut tx, &me, module.department_id).await?;
    if !order_modules::remove_file(&mut *tx, id, file_id).await? {
        return Err(AppError::NotFound("module file"));
    }
    tx.commit().await?;
    Ok(Json(view(&state, &me, id).await?))
}

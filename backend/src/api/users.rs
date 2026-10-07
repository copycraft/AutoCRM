//! Staff accounts. Admin only. There is no self-registration: an admin creates the account
//! with a temporary password that must be changed at first login.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, Auth};
use super::{Items, required};
use crate::AppState;
use crate::domain::email::normalize_address;
use crate::domain::role::{Capability, Role};
use crate::error::{AppError, AppResult};
use crate::repo::users::User;
use crate::repo::{audit, sessions, users};
use crate::service::auth;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(update))
        .routes(routes!(reset_password))
        .routes(routes!(revoke_sessions))
        .routes(routes!(reset_two_factor))
}

#[utoipa::path(
    get, path = "/users", tag = "users",
    responses((status = 200, body = Items<User>))
)]
async fn list(State(state): State<AppState>, Auth(me): Auth) -> AppResult<Json<Items<User>>> {
    me.require(Capability::ManageUsers)?;
    Ok(Items::new(users::list(&state.db).await?))
}

#[derive(Deserialize, ToSchema)]
struct CreateUser {
    email: String,
    display_name: String,
    role: Role,
    temporary_password: String,
}

#[utoipa::path(
    post, path = "/users", tag = "users",
    request_body = CreateUser,
    responses((status = 201, body = User))
)]
async fn create(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(body): ApiJson<CreateUser>,
) -> AppResult<(StatusCode, Json<User>)> {
    me.require(Capability::ManageUsers)?;
    let email = normalize_address(&body.email)
        .ok_or_else(|| AppError::validation("email is not a valid address"))?;
    let display_name = required("display_name", &body.display_name)?;
    auth::validate_new_password(&body.temporary_password)?;
    let hash = auth::hash_password_async(body.temporary_password).await?;

    let mut tx = state.db.begin().await?;
    let user = users::insert(&mut *tx, &email, &display_name, body.role, &hash, true).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "user",
        user.id,
        "create",
        json!({ "email": email, "role": body.role }),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(user)))
}

#[derive(Deserialize, ToSchema)]
struct UpdateUser {
    display_name: Option<String>,
    role: Option<Role>,
    is_active: Option<bool>,
    /// Unlocks the HR module for a non-admin. Admins have it regardless.
    hr_access: Option<bool>,
    /// Replaces the capabilities granted on top of the role. `manage_users` and
    /// `access_hr` cannot be granted this way.
    permissions: Option<Vec<Capability>>,
}

#[utoipa::path(
    patch, path = "/users/{id}", tag = "users",
    params(("id" = i64, Path)),
    request_body = UpdateUser,
    responses((status = 200, body = User))
)]
async fn update(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(body): ApiJson<UpdateUser>,
) -> AppResult<Json<User>> {
    me.require(Capability::ManageUsers)?;
    let display_name = body
        .display_name
        .as_deref()
        .map(|n| required("display_name", n))
        .transpose()?;

    let permissions = body
        .permissions
        .map(|caps| {
            if let Some(c) = caps.iter().find(|c| !c.grantable()) {
                return Err(AppError::validation(format!(
                    "{} cannot be granted per user",
                    c.key()
                )));
            }
            let mut keys: Vec<String> = caps.iter().map(|c| c.key()).collect();
            keys.sort();
            keys.dedup();
            Ok(keys)
        })
        .transpose()?;

    let mut tx = state.db.begin().await?;
    let before = users::find(&mut *tx, id)
        .await?
        .ok_or(AppError::NotFound("user"))?;

    // Never let the system end up with nobody able to administer it.
    let loses_admin = before.role == Role::Admin
        && before.is_active
        && (body.role.is_some_and(|r| r != Role::Admin) || body.is_active == Some(false));
    if loses_admin && users::count_active_admins(&mut *tx).await? <= 1 {
        return Err(AppError::rule(
            "last_admin",
            "cannot demote or deactivate the last active admin",
        ));
    }

    let after = users::update(
        &mut *tx,
        id,
        display_name.as_deref(),
        body.role,
        body.is_active,
        body.hr_access,
        permissions.as_deref(),
    )
    .await?
    .ok_or(AppError::NotFound("user"))?;
    if !after.is_active || after.role != before.role {
        // Sessions carry the role at authentication time only via lookup, but a
        // deactivated or re-roled user should start fresh everywhere.
        sessions::revoke_all(&mut *tx, id, None).await?;
    }
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "user",
        id,
        "update",
        audit::diff(&[
            (
                "display_name",
                json!(before.display_name),
                json!(after.display_name),
            ),
            ("role", json!(before.role), json!(after.role)),
            ("is_active", json!(before.is_active), json!(after.is_active)),
            ("hr_access", json!(before.hr_access), json!(after.hr_access)),
            (
                "permissions",
                json!(before.permissions),
                json!(after.permissions),
            ),
        ]),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(after))
}

#[derive(Deserialize, ToSchema)]
struct ResetPassword {
    temporary_password: String,
}

#[utoipa::path(
    post, path = "/users/{id}/password", tag = "users",
    params(("id" = i64, Path)),
    request_body = ResetPassword,
    responses((status = 204, description = "Password reset; the user must change it at next login"))
)]
async fn reset_password(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
    ApiJson(body): ApiJson<ResetPassword>,
) -> AppResult<StatusCode> {
    me.require(Capability::ManageUsers)?;
    auth::validate_new_password(&body.temporary_password)?;
    let hash = auth::hash_password_async(body.temporary_password).await?;

    let mut tx = state.db.begin().await?;
    if !users::set_password(&mut *tx, id, &hash, true).await? {
        return Err(AppError::NotFound("user"));
    }
    sessions::revoke_all(&mut *tx, id, None).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "user",
        id,
        "reset_password",
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
struct RevokedSessions {
    revoked: u64,
}

#[utoipa::path(
    post, path = "/users/{id}/revoke-sessions", tag = "users",
    params(("id" = i64, Path)),
    responses((status = 200, body = RevokedSessions))
)]
async fn revoke_sessions(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<RevokedSessions>> {
    me.require(Capability::ManageUsers)?;
    let revoked = sessions::revoke_all(&state.db, id, None).await?;
    Ok(Json(RevokedSessions { revoked }))
}

#[utoipa::path(
    delete, path = "/users/{id}/two-factor", tag = "users",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Two-factor sign-in switched off (a lost phone); the person can set it up again"))
)]
async fn reset_two_factor(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    me.require(Capability::ManageUsers)?;
    let mut tx = state.db.begin().await?;
    if users::find(&mut *tx, id).await?.is_none() {
        return Err(AppError::NotFound("user"));
    }
    users::disable_totp(&mut *tx, id).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "user",
        id,
        "reset_two_factor",
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

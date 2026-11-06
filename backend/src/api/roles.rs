//! The role grid (0061): what each role may see and do, as an admin sets it. Admins are not
//! in it — they always have everything — and neither is `AccessHr`, which is a per-person
//! flag. A change applies to everyone in the role on their next request.

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::Items;
use super::extract::{ApiJson, ApiPath, Auth};
use crate::AppState;
use crate::domain::role::{Capability, Role};
use crate::error::{AppError, AppResult};
use crate::repo::audit;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_roles))
        .routes(routes!(set_role))
}

/// One role and what it may do now.
#[derive(Serialize, ToSchema)]
pub struct RoleGrid {
    pub role: Role,
    /// Admin: always everything, and not editable.
    pub editable: bool,
    pub capabilities: Vec<Capability>,
    /// The role's out-of-the-box set, to show what "restore defaults" would do.
    pub defaults: Vec<Capability>,
}

async fn grid_of(db: &sqlx::PgPool, role: Role) -> AppResult<Vec<Capability>> {
    if role == Role::Admin {
        return Ok(Capability::ALL.to_vec());
    }
    let keys: Vec<String> =
        sqlx::query_scalar("SELECT capability FROM role_permissions WHERE role = $1")
            .bind(role)
            .fetch_all(db)
            .await?;
    // In the order of the enum, not of the table: the screen lists them the same each time.
    Ok(Capability::ALL
        .into_iter()
        .filter(|c| keys.contains(&c.key()))
        .collect())
}

fn defaults_of(role: Role) -> Vec<Capability> {
    Capability::ALL
        .into_iter()
        .filter(|c| role.can(*c) && (role == Role::Admin || c.grantable()))
        .collect()
}

/// Every role with what it may see and do.
#[utoipa::path(
    get, path = "/roles", tag = "users",
    responses((status = 200, body = Items<RoleGrid>))
)]
async fn list_roles(
    State(state): State<AppState>,
    Auth(me): Auth,
) -> AppResult<Json<Items<RoleGrid>>> {
    me.require(Capability::ManageUsers)?;
    let mut items = Vec::new();
    for role in Role::ALL {
        items.push(RoleGrid {
            role,
            editable: role != Role::Admin,
            capabilities: grid_of(&state.db, role).await?,
            defaults: defaults_of(role),
        });
    }
    Ok(Items::new(items))
}

#[derive(Deserialize, ToSchema)]
struct RoleCapabilities {
    /// The role's whole set; what is left out is taken away.
    capabilities: Vec<Capability>,
}

/// Set what a role may see and do. Not the admin role, and not the capabilities only an
/// admin can have (user management, HR's own flag).
#[utoipa::path(
    put, path = "/roles/{role}", tag = "users",
    params(("role" = String, Path, description = "office, sales, accountant, designer or viewer")),
    request_body = RoleCapabilities,
    responses((status = 200, body = RoleGrid))
)]
async fn set_role(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(role): ApiPath<String>,
    ApiJson(b): ApiJson<RoleCapabilities>,
) -> AppResult<Json<RoleGrid>> {
    me.require(Capability::ManageUsers)?;
    let role = Role::from_key(&role).ok_or(AppError::NotFound("role"))?;
    if role == Role::Admin {
        return Err(AppError::validation("the admin role always has everything"));
    }
    if let Some(c) = b.capabilities.iter().find(|c| !c.grantable()) {
        return Err(AppError::validation(format!(
            "{} cannot be given to a role",
            c.key()
        )));
    }
    let before = grid_of(&state.db, role).await?;
    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM role_permissions WHERE role = $1")
        .bind(role)
        .execute(&mut *tx)
        .await?;
    for c in &b.capabilities {
        sqlx::query(
            "INSERT INTO role_permissions (role, capability) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(role)
        .bind(c.key())
        .execute(&mut *tx)
        .await?;
    }
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "role",
        0,
        "role_permissions",
        json!({
            "role": role.key(),
            "before": before.iter().map(|c| c.key()).collect::<Vec<_>>(),
            "after": b.capabilities.iter().map(|c| c.key()).collect::<Vec<_>>(),
        }),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(RoleGrid {
        role,
        editable: true,
        capabilities: grid_of(&state.db, role).await?,
        defaults: defaults_of(role),
    }))
}

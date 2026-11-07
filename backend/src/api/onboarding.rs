//! Onboarding by invitation (0066): an admin invites a colleague by email, and the
//! colleague sets up their own account from the letter's link. See `service::onboarding`.
//!
//! - `POST /users/invite`: a new account and its letter (`ManageUsers`).
//! - `POST /users/{id}/invite`: the letter again, for a link that expired or a forgotten
//!   password (`ManageUsers`).
//! - `GET` / `POST /onboarding/{token}`: the page behind the link. No sign-in: the link is
//!   the proof. Posting picks the look and the password, and signs the browser in.

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum_extra::extract::cookie::CookieJar;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::{SessionUser, session_cookie, session_user};
use super::extract::{ApiJson, ApiPath, Auth, check_origin, client_ip, user_agent};
use super::required;
use crate::AppState;
use crate::domain::email::normalize_address;
use crate::domain::role::{Capability, Role};
use crate::error::{AppError, AppResult};
use crate::repo::audit;
use crate::repo::sessions::SessionKind;
use crate::repo::users::{self, User};
use crate::service::{auth, onboarding};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(invite_user))
        .routes(routes!(resend_invite))
        .routes(routes!(invite_info, accept_invite))
}

#[derive(Deserialize, ToSchema)]
struct InviteUser {
    email: String,
    display_name: String,
    role: Role,
}

#[derive(Serialize, ToSchema)]
struct Invited {
    user: User,
    /// When the letter's link stops working.
    expires_at: DateTime<Utc>,
}

/// A new account, set up by its owner: the letter goes out at once with a one-time link
/// (7 days). The account has no usable password until the link is used.
#[utoipa::path(
    post, path = "/users/invite", tag = "users",
    request_body = InviteUser,
    responses((status = 201, body = Invited), (status = 409, description = "The address already has an account"))
)]
async fn invite_user(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiJson(body): ApiJson<InviteUser>,
) -> AppResult<(StatusCode, Json<Invited>)> {
    me.require(Capability::ManageUsers)?;
    let email = normalize_address(&body.email)
        .ok_or_else(|| AppError::validation("email is not a valid address"))?;
    let display_name = required("display_name", &body.display_name)?;
    // Nobody knows this password, so nobody can sign in until the link is used.
    let unusable = auth::hash_password_async(auth::generate_token()).await?;

    let mut tx = state.db.begin().await?;
    let user = users::insert(&mut *tx, &email, &display_name, body.role, &unusable, true).await?;
    audit::record(
        &mut *tx,
        Some(me.user_id),
        "user",
        user.id,
        "create",
        json!({ "email": email, "role": body.role, "invited": true }),
    )
    .await?;
    let expires_at =
        onboarding::send_invite(&mut *tx, &state, &user, (me.user_id, &me.display_name)).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(Invited { user, expires_at })))
}

#[derive(Serialize, ToSchema)]
struct InviteSent {
    expires_at: DateTime<Utc>,
}

/// The letter again, with a new link; older links stop working. For an account that never
/// got set up, or one whose owner forgot the password. Their password stays until the new
/// link is used.
#[utoipa::path(
    post, path = "/users/{id}/invite", tag = "users",
    params(("id" = i64, Path)),
    responses((status = 200, body = InviteSent))
)]
async fn resend_invite(
    State(state): State<AppState>,
    Auth(me): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<Json<InviteSent>> {
    me.require(Capability::ManageUsers)?;
    let user = users::find(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("user"))?;
    let mut tx = state.db.begin().await?;
    let expires_at =
        onboarding::send_invite(&mut *tx, &state, &user, (me.user_id, &me.display_name)).await?;
    tx.commit().await?;
    Ok(Json(InviteSent { expires_at }))
}

/// Who the link is for, while it can still be used.
#[derive(Serialize, ToSchema)]
struct InviteInfo {
    email: String,
    display_name: String,
    expires_at: DateTime<Utc>,
}

#[utoipa::path(
    get, path = "/onboarding/{token}", tag = "auth", security(()),
    params(("token" = String, Path)),
    responses(
        (status = 200, body = InviteInfo),
        (status = 404, description = "No such link, or a newer one replaced it"),
        (status = 422, description = "`invite_used` or `invite_expired`"),
    )
)]
async fn invite_info(
    State(state): State<AppState>,
    ApiPath(token): ApiPath<String>,
) -> AppResult<Json<InviteInfo>> {
    let invite = onboarding::open_invite(&state.db, &token).await?;
    Ok(Json(InviteInfo {
        email: invite.email,
        display_name: invite.display_name,
        expires_at: invite.expires_at,
    }))
}

#[derive(Deserialize, ToSchema)]
struct AcceptInvite {
    password: String,
    /// `system`, `light` or `dark`.
    theme: String,
}

#[derive(Serialize, ToSchema)]
struct Welcome {
    user: SessionUser,
}

/// Sets the password and the look, spends the link and signs this browser in (the httpOnly
/// session cookie, as at login). Other sessions of the account end.
#[utoipa::path(
    post, path = "/onboarding/{token}", tag = "auth", security(()),
    params(("token" = String, Path)),
    request_body = AcceptInvite,
    responses((status = 200, body = Welcome))
)]
async fn accept_invite(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    ApiPath(token): ApiPath<String>,
    ApiJson(body): ApiJson<AcceptInvite>,
) -> AppResult<(CookieJar, Json<Welcome>)> {
    // Like the login: a hostile page must not sign a browser into an account.
    if headers.contains_key(header::ORIGIN) {
        check_origin(&headers, &state.config)?;
    }
    let user_id = onboarding::accept(&state.db, &token, body.password, &body.theme).await?;
    let ua = user_agent(&headers);
    let ip = client_ip(&headers);
    let outcome = auth::open_session(
        &state.db,
        user_id,
        SessionKind::Web,
        None,
        ua.as_deref(),
        ip.as_deref(),
    )
    .await?;
    tracing::info!(
        user_id,
        session_id = outcome.session_id,
        "login via invitation"
    );
    let user = session_user(&state, &outcome.user, SessionKind::Web).await?;
    Ok((
        jar.add(session_cookie(&state, outcome.token)),
        Json(Welcome { user }),
    ))
}

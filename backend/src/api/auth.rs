use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiPath, Auth, SESSION_COOKIE, check_origin, client_ip, user_agent};
use super::{Items, optional};
use crate::AppState;
use crate::domain::role::Role;
use crate::error::{AppError, AppResult};
use crate::repo::config::{self, UserSettings};
use crate::repo::sessions::{self, SessionInfo, SessionKind};
use crate::repo::users::User;
use crate::service::auth::{self, AuthUser, LoginRequest};

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(login))
        .routes(routes!(logout))
        .routes(routes!(me))
        .routes(routes!(change_password))
        .routes(routes!(list_sessions))
        .routes(routes!(revoke_session))
        .routes(routes!(get_preferences))
        .routes(routes!(put_preferences))
}

fn default_client() -> SessionKind {
    SessionKind::Web
}

#[derive(Deserialize, ToSchema)]
struct LoginBody {
    email: String,
    password: String,
    /// `web` (default) sets the session cookie; `mobile` returns a bearer token.
    #[serde(default = "default_client")]
    client: SessionKind,
    device_label: Option<String>,
}

/// The signed-in user as clients see it. Login and `/auth/me` return the same shape.
#[derive(Serialize, ToSchema)]
pub struct SessionUser {
    pub id: i64,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    /// While true, every endpoint except `/auth/me`, `/auth/password` and `/auth/logout`
    /// answers `422 password_change_required`.
    pub must_change_password: bool,
    pub session_kind: SessionKind,
}

impl SessionUser {
    fn from_user(user: &User, session_kind: SessionKind) -> Self {
        SessionUser {
            id: user.id,
            email: user.email.clone(),
            display_name: user.display_name.clone(),
            role: user.role,
            must_change_password: user.must_change_password,
            session_kind,
        }
    }
}

impl From<AuthUser> for SessionUser {
    fn from(user: AuthUser) -> Self {
        SessionUser {
            id: user.user_id,
            email: user.email,
            display_name: user.display_name,
            role: user.role,
            must_change_password: user.must_change_password,
            session_kind: user.session_kind,
        }
    }
}

#[derive(Serialize, ToSchema)]
struct LoginResponse {
    user: SessionUser,
    /// Mobile clients only: send as `Authorization: Bearer <token>`.
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    /// Mobile clients only.
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, ToSchema)]
struct MeResponse {
    user: SessionUser,
}

#[utoipa::path(
    post, path = "/auth/login", tag = "auth", security(()),
    request_body = LoginBody,
    responses((status = 200, description = "Signed in. Web clients also receive the httpOnly `autocrm_session` cookie.", body = LoginResponse))
)]
async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    ApiJson(body): ApiJson<LoginBody>,
) -> AppResult<(CookieJar, Json<LoginResponse>)> {
    // Login CSRF: a hostile page must not be able to sign a browser into another account.
    if body.client == SessionKind::Web && headers.contains_key(header::ORIGIN) {
        check_origin(&headers, &state.config)?;
    }
    let outcome = auth::login(
        &state.db,
        LoginRequest {
            email: body.email,
            password: body.password,
            kind: body.client,
            device_label: optional(body.device_label).map(|d| d.chars().take(100).collect()),
            user_agent: user_agent(&headers),
            ip: client_ip(&headers),
        },
    )
    .await?;
    tracing::info!(user_id = outcome.user.id, session_id = outcome.session_id, kind = ?body.client, "login");

    let user = SessionUser::from_user(&outcome.user, body.client);
    match body.client {
        SessionKind::Web => {
            // Until the absolute cap, not `expires_at`: the cookie is never re-issued, and
            // the server's sliding `expires_at` already enforces the idle timeout.
            let now = chrono::Utc::now();
            let max_age = auth::cookie_max_age_secs(SessionKind::Web, now, now);
            let cookie = Cookie::build((SESSION_COOKIE, outcome.token))
                .http_only(true)
                .secure(state.config.cookie_secure)
                .same_site(SameSite::Lax)
                .path("/")
                .max_age(cookie::time::Duration::seconds(max_age))
                .build();
            Ok((
                jar.add(cookie),
                Json(LoginResponse {
                    user,
                    token: None,
                    expires_at: None,
                }),
            ))
        }
        SessionKind::Mobile => Ok((
            jar,
            Json(LoginResponse {
                user,
                token: Some(outcome.token),
                expires_at: Some(outcome.expires_at),
            }),
        )),
    }
}

#[utoipa::path(
    post, path = "/auth/logout", tag = "auth",
    responses((status = 204, description = "Current session revoked"))
)]
async fn logout(
    State(state): State<AppState>,
    Auth(user): Auth,
    jar: CookieJar,
) -> AppResult<(StatusCode, CookieJar)> {
    sessions::revoke(&state.db, user.session_id, user.user_id).await?;
    let jar = jar.remove(Cookie::build(SESSION_COOKIE).path("/"));
    Ok((StatusCode::NO_CONTENT, jar))
}

#[utoipa::path(
    get, path = "/auth/me", tag = "auth",
    responses((status = 200, body = MeResponse))
)]
async fn me(Auth(user): Auth) -> Json<MeResponse> {
    Json(MeResponse { user: user.into() })
}

#[derive(Deserialize, ToSchema)]
struct ChangePasswordBody {
    current_password: String,
    new_password: String,
}

#[utoipa::path(
    post, path = "/auth/password", tag = "auth",
    request_body = ChangePasswordBody,
    responses((status = 204, description = "Password changed; other sessions revoked"))
)]
async fn change_password(
    State(state): State<AppState>,
    Auth(user): Auth,
    ApiJson(body): ApiJson<ChangePasswordBody>,
) -> AppResult<StatusCode> {
    auth::change_password(&state.db, &user, body.current_password, body.new_password).await?;
    tracing::info!(
        user_id = user.user_id,
        "password changed; other sessions revoked"
    );
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
struct SessionView {
    #[serde(flatten)]
    session: SessionInfo,
    current: bool,
}

#[utoipa::path(
    get, path = "/auth/sessions", tag = "auth",
    responses((status = 200, body = Items<SessionView>))
)]
async fn list_sessions(
    State(state): State<AppState>,
    Auth(user): Auth,
) -> AppResult<Json<Items<SessionView>>> {
    let sessions = sessions::list_active(&state.db, user.user_id).await?;
    Ok(Items::new(
        sessions
            .into_iter()
            .map(|s| SessionView {
                current: s.id == user.session_id,
                session: s,
            })
            .collect(),
    ))
}

#[utoipa::path(
    delete, path = "/auth/sessions/{id}", tag = "auth",
    params(("id" = i64, Path)),
    responses((status = 204, description = "Session revoked"))
)]
async fn revoke_session(
    State(state): State<AppState>,
    Auth(user): Auth,
    ApiPath(id): ApiPath<i64>,
) -> AppResult<StatusCode> {
    if sessions::revoke(&state.db, id, user.user_id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound("session"))
    }
}

fn default_preferences(user_id: i64) -> UserSettings {
    UserSettings {
        user_id,
        density: "comfortable".into(),
        page_size: 50,
        // Not "now": no row exists, so there is no update time to report.
        updated_at: DateTime::default(),
    }
}

#[utoipa::path(
    get, path = "/auth/preferences", tag = "auth",
    responses((status = 200, body = UserSettings, description = "Own preferences; defaults when never saved"))
)]
async fn get_preferences(
    State(state): State<AppState>,
    Auth(user): Auth,
) -> AppResult<Json<UserSettings>> {
    let prefs = config::user_settings(&state.db, user.user_id)
        .await?
        .unwrap_or_else(|| default_preferences(user.user_id));
    Ok(Json(prefs))
}

#[derive(Deserialize, ToSchema)]
struct PreferencesBody {
    density: Option<String>,
    page_size: Option<i32>,
}

#[utoipa::path(
    put, path = "/auth/preferences", tag = "auth",
    request_body = PreferencesBody,
    responses((status = 200, body = UserSettings))
)]
async fn put_preferences(
    State(state): State<AppState>,
    Auth(user): Auth,
    ApiJson(b): ApiJson<PreferencesBody>,
) -> AppResult<Json<UserSettings>> {
    let current = config::user_settings(&state.db, user.user_id)
        .await?
        .unwrap_or_else(|| default_preferences(user.user_id));
    let density = b.density.unwrap_or(current.density);
    if density != "comfortable" && density != "compact" {
        return Err(AppError::validation(
            "density: expected comfortable or compact",
        ));
    }
    let page_size = b.page_size.unwrap_or(current.page_size);
    if ![25, 50, 100].contains(&page_size) {
        return Err(AppError::validation("page_size: expected 25, 50 or 100"));
    }
    let saved = config::upsert_user_settings(&state.db, user.user_id, &density, page_size).await?;
    Ok(Json(saved))
}

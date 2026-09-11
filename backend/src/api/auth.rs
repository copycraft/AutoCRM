use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::extract::{ApiJson, ApiPath, Auth, SESSION_COOKIE, check_origin, client_ip, user_agent};
use super::{Items, optional};
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::repo::sessions::{self, SessionInfo, SessionKind};
use crate::service::auth::{self, LoginRequest};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .route("/auth/password", post(change_password))
        .route("/auth/sessions", get(list_sessions))
        .route("/auth/sessions/{id}", delete(revoke_session))
}

fn default_client() -> SessionKind {
    SessionKind::Web
}

#[derive(Deserialize)]
struct LoginBody {
    email: String,
    password: String,
    #[serde(default = "default_client")]
    client: SessionKind,
    device_label: Option<String>,
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    ApiJson(body): ApiJson<LoginBody>,
) -> AppResult<(CookieJar, Json<Value>)> {
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

    match body.client {
        SessionKind::Web => {
            let max_age = (outcome.expires_at - chrono::Utc::now())
                .num_seconds()
                .max(0);
            let cookie = Cookie::build((SESSION_COOKIE, outcome.token))
                .http_only(true)
                .secure(state.config.cookie_secure)
                .same_site(SameSite::Lax)
                .path("/")
                .max_age(cookie::time::Duration::seconds(max_age))
                .build();
            Ok((jar.add(cookie), Json(json!({ "user": outcome.user }))))
        }
        SessionKind::Mobile => Ok((
            jar,
            Json(
                json!({ "token": outcome.token, "expires_at": outcome.expires_at, "user": outcome.user }),
            ),
        )),
    }
}

async fn logout(
    State(state): State<AppState>,
    Auth(user): Auth,
    jar: CookieJar,
) -> AppResult<(StatusCode, CookieJar)> {
    sessions::revoke(&state.db, user.session_id, user.user_id).await?;
    let jar = jar.remove(Cookie::build(SESSION_COOKIE).path("/"));
    Ok((StatusCode::NO_CONTENT, jar))
}

async fn me(Auth(user): Auth) -> Json<Value> {
    Json(json!({ "user": user }))
}

#[derive(Deserialize)]
struct ChangePasswordBody {
    current_password: String,
    new_password: String,
}

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

#[derive(Serialize)]
struct SessionView {
    #[serde(flatten)]
    session: SessionInfo,
    current: bool,
}

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

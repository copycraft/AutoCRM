//! Request extractors: authentication, CSRF origin check, and JSON/query/path extractors
//! whose rejections use the standard error body.

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, OriginalUri};
use axum::http::request::Parts;
use axum::http::{HeaderMap, Method, header};
use axum_extra::extract::cookie::CookieJar;

use crate::AppState;
use crate::config::Config;
use crate::error::AppError;
use crate::service::auth::{self, AuthUser};

pub const SESSION_COOKIE: &str = "autocrm_session";

/// Endpoints a user may reach while their password must be changed.
const PASSWORD_CHANGE_ALLOWED: [&str; 3] =
    ["/api/auth/me", "/api/auth/password", "/api/auth/logout"];

/// An authenticated user. Mobile clients send `Authorization: Bearer <token>`;
/// the web app uses the httpOnly session cookie.
pub struct Auth(pub AuthUser);

impl FromRequestParts<AppState> for Auth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let (token, via_cookie) = match bearer_token(&parts.headers) {
            Some(token) => (token, false),
            None => {
                let jar = CookieJar::from_headers(&parts.headers);
                let token = jar
                    .get(SESSION_COOKIE)
                    .map(|c| c.value().to_string())
                    .ok_or(AppError::Unauthenticated)?;
                (token, true)
            }
        };

        // Cookies ride along on cross-site requests; bearer tokens don't. So only
        // cookie-authenticated, state-changing requests need the origin check.
        if via_cookie && !matches!(parts.method, Method::GET | Method::HEAD | Method::OPTIONS) {
            check_origin(&parts.headers, &state.config)?;
        }

        let user = auth::authenticate(&state.db, &token).await?;

        if user.must_change_password {
            let path = parts
                .extensions
                .get::<OriginalUri>()
                .map(|u| u.0.path().to_string())
                .unwrap_or_else(|| parts.uri.path().to_string());
            if !PASSWORD_CHANGE_ALLOWED.contains(&path.as_str()) {
                return Err(AppError::rule(
                    "password_change_required",
                    "you must change your password before continuing",
                ));
            }
        }
        Ok(Auth(user))
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let token = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))?;
    Some(token.trim().to_string())
}

/// Rejects state-changing cookie requests whose Origin isn't ours. Browsers always send
/// Origin on cross-origin and on same-origin POST/PUT/PATCH/DELETE fetches, so a missing
/// header is treated as a failure.
pub fn check_origin(headers: &HeaderMap, config: &Config) -> Result<(), AppError> {
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(|o| o.trim_end_matches('/'));
    match origin {
        Some(o) if config.allowed_origins.iter().any(|a| a == o) || config.public_base_url == o => {
            Ok(())
        }
        _ => Err(AppError::Forbidden),
    }
}

pub fn client_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|ip| ip.trim().chars().take(64).collect())
}

pub fn user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|ua| ua.chars().take(300).collect())
}

#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
pub struct ApiJson<T>(pub T);

#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(AppError))]
pub struct ApiQuery<T>(pub T);

#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(AppError))]
pub struct ApiPath<T>(pub T);

impl From<JsonRejection> for AppError {
    fn from(r: JsonRejection) -> Self {
        AppError::Validation(r.body_text())
    }
}

impl From<QueryRejection> for AppError {
    fn from(r: QueryRejection) -> Self {
        AppError::Validation(r.body_text())
    }
}

impl From<PathRejection> for AppError {
    fn from(r: PathRejection) -> Self {
        AppError::Validation(r.body_text())
    }
}

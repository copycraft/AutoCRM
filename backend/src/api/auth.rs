use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{
    ApiJson, ApiPath, ApiQuery, Auth, SESSION_COOKIE, check_origin, client_ip, user_agent,
};
use super::{Items, optional};
use crate::AppState;
use crate::domain::role::{Capability, Role};
use crate::domain::{ics, totp};
use crate::error::{AppError, AppResult};
use crate::integrations::google;
use crate::repo::config::{self, UserSettings};
use crate::repo::sessions::{self, SessionInfo, SessionKind};
use crate::repo::users::{self, User};
use crate::service::auth::{self, AuthUser, LoginOutcome, LoginRequest};
use crate::service::secrets;

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
        // 0049
        .routes(routes!(providers))
        .routes(routes!(google_start))
        .routes(routes!(google_callback))
        .routes(routes!(two_factor_status))
        .routes(routes!(two_factor_setup))
        .routes(routes!(two_factor_enable))
        .routes(routes!(two_factor_disable))
        .routes(routes!(get_calendar, create_calendar, delete_calendar))
        .routes(routes!(calendar_feed))
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
    /// The authenticator app's six digits, for an account with two-factor sign-in. Without
    /// it such an account answers `422 totp_required` (the password was right).
    totp_code: Option<String>,
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
    /// Whether the HR module is open to this user: admins, and users an admin granted it.
    pub hr_access: bool,
    /// Everything this user may do: the role's defaults plus what an admin granted.
    /// Clients gate their UI on this rather than on the role.
    pub capabilities: Vec<Capability>,
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
            hr_access: user.role == Role::Admin || user.hr_access,
            capabilities: {
                let grants = auth::parse_permissions(&user.permissions);
                Capability::ALL
                    .into_iter()
                    .filter(|c| {
                        user.role.can(*c)
                            || (*c == Capability::AccessHr && user.hr_access)
                            || grants.contains(c)
                    })
                    .collect()
            },
        }
    }
}

impl From<AuthUser> for SessionUser {
    fn from(user: AuthUser) -> Self {
        let hr_access = user.can(Capability::AccessHr);
        let capabilities = user.capabilities();
        SessionUser {
            id: user.user_id,
            email: user.email,
            display_name: user.display_name,
            role: user.role,
            must_change_password: user.must_change_password,
            session_kind: user.session_kind,
            hr_access,
            capabilities,
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
    let device_label: Option<String> =
        optional(body.device_label).map(|d| d.chars().take(100).collect());
    let ua = user_agent(&headers);
    let ip = client_ip(&headers);
    let outcome = auth::login(
        &state.db,
        &state.config,
        LoginRequest {
            email: body.email,
            password: body.password,
            kind: body.client,
            device_label: device_label.clone(),
            user_agent: ua.clone(),
            ip: ip.clone(),
            totp_code: body.totp_code,
        },
    )
    .await?;
    tracing::info!(user_id = outcome.user.id, session_id = outcome.session_id, kind = ?body.client, "login");
    alert_new_device(
        &state,
        &outcome,
        device_label.as_deref(),
        ua.as_deref(),
        ip.as_deref(),
    )
    .await;

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
        email_signature: None,
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
    /// Absent keeps it; empty removes it.
    email_signature: Option<String>,
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
    let signature = match b.email_signature {
        None => current.email_signature,
        Some(s) => {
            let s = s.trim_end().trim_start_matches(['\r', '\n']).to_string();
            if s.chars().count() > 2000 {
                return Err(AppError::validation(
                    "email_signature: at most 2000 characters",
                ));
            }
            (!s.trim().is_empty()).then_some(s)
        }
    };
    let saved = config::upsert_user_settings(
        &state.db,
        user.user_id,
        &density,
        page_size,
        signature.as_deref(),
    )
    .await?;
    Ok(Json(saved))
}

/// Mails the account's owner when it was signed in to from a device it never used. Logged,
/// never fatal: the sign-in already happened.
async fn alert_new_device(
    state: &AppState,
    outcome: &LoginOutcome,
    device_label: Option<&str>,
    ua: Option<&str>,
    ip: Option<&str>,
) {
    if !outcome.new_device {
        return;
    }
    let device = auth::describe_device(device_label, ua);
    if let Err(e) = crate::service::email::new_device_alert(
        state,
        &outcome.user,
        outcome.session_id,
        &device,
        ip,
    )
    .await
    {
        tracing::error!(error = %e, user_id = outcome.user.id, "new-device alert not queued");
    }
}

// --- Sign in with Google (0049) ---

#[derive(Serialize, ToSchema)]
struct Providers {
    /// "Sign in with Google" is configured.
    google: bool,
}

#[utoipa::path(
    get, path = "/auth/providers", tag = "auth", security(()),
    responses((status = 200, body = Providers, description = "Which sign-in methods the login page offers"))
)]
async fn providers(State(state): State<AppState>) -> Json<Providers> {
    Json(Providers {
        google: state.config.google.is_some(),
    })
}

const OAUTH_COOKIE: &str = "autocrm_oauth";
const OAUTH_COOKIE_PATH: &str = "/api/auth/google";

fn google_redirect_uri(state: &AppState) -> String {
    format!(
        "{}/api/auth/google/callback",
        state.config.public_base_url.trim_end_matches('/')
    )
}

fn login_page(state: &AppState, error: Option<&str>) -> Redirect {
    let base = state.config.public_base_url.trim_end_matches('/');
    match error {
        Some(e) => Redirect::to(&format!("{base}/hu/login?error={e}")),
        None => Redirect::to(&format!("{base}/hu")),
    }
}

#[utoipa::path(
    get, path = "/auth/google/start", tag = "auth", security(()),
    responses((status = 303, description = "To Google's account chooser"))
)]
async fn google_start(State(state): State<AppState>, jar: CookieJar) -> Response {
    let Some(cfg) = state.config.google.as_ref() else {
        return login_page(&state, Some("google_off")).into_response();
    };
    let csrf = auth::generate_token();
    let nonce = auth::generate_token();
    let cookie = Cookie::build((OAUTH_COOKIE, format!("{csrf}.{nonce}")))
        .http_only(true)
        .secure(state.config.cookie_secure)
        .same_site(SameSite::Lax)
        .path(OAUTH_COOKIE_PATH)
        .max_age(cookie::time::Duration::minutes(10))
        .build();
    let url = google::authorize_url(cfg, &google_redirect_uri(&state), &csrf, &nonce);
    (jar.add(cookie), Redirect::to(&url)).into_response()
}

#[derive(Deserialize, IntoParams)]
struct GoogleCallback {
    code: Option<String>,
    state: Option<String>,
    /// Set by Google when the person cancelled.
    error: Option<String>,
}

#[utoipa::path(
    get, path = "/auth/google/callback", tag = "auth", security(()),
    params(GoogleCallback),
    responses((status = 303, description = "Signed in (session cookie set) and on to the app, or back to the login page with `?error=`"))
)]
async fn google_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    ApiQuery(q): ApiQuery<GoogleCallback>,
) -> Response {
    let stored = jar.get(OAUTH_COOKIE).map(|c| c.value().to_string());
    let jar = jar.remove(Cookie::build(OAUTH_COOKIE).path(OAUTH_COOKIE_PATH));
    let fail = |jar: CookieJar, code: &str| (jar, login_page(&state, Some(code))).into_response();
    let Some(cfg) = state.config.google.as_ref() else {
        return fail(jar, "google_off");
    };
    if q.error.is_some() {
        return fail(jar, "google_cancelled");
    }
    let (Some(code), Some(given_state), Some(stored)) = (q.code, q.state, stored) else {
        return fail(jar, "google_failed");
    };
    let Some((csrf, nonce)) = stored.split_once('.') else {
        return fail(jar, "google_failed");
    };
    if csrf != given_state {
        return fail(jar, "google_failed");
    }
    let redirect_uri = google_redirect_uri(&state);
    let email =
        match google::exchange(cfg, &redirect_uri, &code, nonce, Utc::now().timestamp()).await {
            Ok(email) => email,
            Err(e) => {
                tracing::warn!(error = %e, "google sign-in refused");
                return fail(jar, "google_failed");
            }
        };
    let user_id = match users::find_id_by_email(&state.db, &email).await {
        Ok(Some((id, true))) => id,
        Ok(_) => {
            tracing::info!(%email, "google sign-in for an address with no active account");
            return fail(jar, "google_unknown");
        }
        Err(e) => {
            tracing::error!(error = %e, "google sign-in lookup failed");
            return fail(jar, "google_failed");
        }
    };
    let ua = user_agent(&headers);
    let ip = client_ip(&headers);
    // Google's own two-step verification stands in for ours here: the Workspace account is
    // the stronger identity, and its admin can enforce 2SV for everyone.
    let outcome = match auth::open_session(
        &state.db,
        user_id,
        SessionKind::Web,
        None,
        ua.as_deref(),
        ip.as_deref(),
    )
    .await
    {
        Ok(o) => o,
        Err(e) => {
            tracing::error!(error = %e, "google sign-in session failed");
            return fail(jar, "google_failed");
        }
    };
    tracing::info!(user_id, session_id = outcome.session_id, "login via google");
    alert_new_device(&state, &outcome, None, ua.as_deref(), ip.as_deref()).await;
    let now = Utc::now();
    let max_age = auth::cookie_max_age_secs(SessionKind::Web, now, now);
    let cookie = Cookie::build((SESSION_COOKIE, outcome.token))
        .http_only(true)
        .secure(state.config.cookie_secure)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(cookie::time::Duration::seconds(max_age))
        .build();
    (jar.add(cookie), login_page(&state, None)).into_response()
}

// --- Two-factor sign-in (0049) ---

#[derive(Serialize, ToSchema)]
struct TwoFactorStatus {
    enabled: bool,
    /// A secret was handed out but not yet confirmed with a code.
    pending: bool,
}

#[utoipa::path(
    get, path = "/auth/two-factor", tag = "auth",
    responses((status = 200, body = TwoFactorStatus))
)]
async fn two_factor_status(
    State(state): State<AppState>,
    Auth(user): Auth,
) -> AppResult<Json<TwoFactorStatus>> {
    let s = users::totp_state(&state.db, user.user_id)
        .await?
        .ok_or(AppError::Unauthenticated)?;
    Ok(Json(TwoFactorStatus {
        enabled: s.enabled,
        pending: !s.enabled && s.secret.is_some(),
    }))
}

#[derive(Deserialize, ToSchema)]
struct PasswordConfirm {
    password: String,
}

#[derive(Serialize, ToSchema)]
struct TwoFactorSetup {
    /// Base32, for typing into the app by hand.
    secret: String,
    /// `otpauth://` link: shown as a QR code, or tapped on the phone.
    otpauth_url: String,
}

async fn confirm_password(state: &AppState, user: &AuthUser, password: String) -> AppResult<()> {
    let creds = users::find_credentials(&state.db, user.user_id)
        .await?
        .ok_or(AppError::Unauthenticated)?;
    if !auth::verify_password_async(password, creds.password_hash).await? {
        return Err(AppError::rule(
            "wrong_password",
            "current password is incorrect",
        ));
    }
    Ok(())
}

#[utoipa::path(
    post, path = "/auth/two-factor/setup", tag = "auth",
    request_body = PasswordConfirm,
    responses((status = 200, body = TwoFactorSetup, description = "A new secret; sign-in asks for codes only once it is confirmed (`/auth/two-factor/enable`)"))
)]
async fn two_factor_setup(
    State(state): State<AppState>,
    Auth(user): Auth,
    ApiJson(b): ApiJson<PasswordConfirm>,
) -> AppResult<Json<TwoFactorSetup>> {
    confirm_password(&state, &user, b.password).await?;
    let current = users::totp_state(&state.db, user.user_id)
        .await?
        .ok_or(AppError::Unauthenticated)?;
    if current.enabled {
        return Err(AppError::conflict(
            "duplicate",
            "two-factor sign-in is already on",
        ));
    }
    let raw: [u8; 20] = rand::random();
    let secret = totp::base32_encode(&raw);
    users::set_pending_totp(
        &state.db,
        user.user_id,
        &secrets::seal(&state.config, &secret),
    )
    .await?;
    Ok(Json(TwoFactorSetup {
        otpauth_url: totp::otpauth_url("AutoCRM", &user.email, &secret),
        secret,
    }))
}

#[derive(Deserialize, ToSchema)]
struct TwoFactorCode {
    code: String,
}

#[utoipa::path(
    post, path = "/auth/two-factor/enable", tag = "auth",
    request_body = TwoFactorCode,
    responses((status = 204, description = "On: every later sign-in asks for a code"))
)]
async fn two_factor_enable(
    State(state): State<AppState>,
    Auth(user): Auth,
    ApiJson(b): ApiJson<TwoFactorCode>,
) -> AppResult<StatusCode> {
    let s = users::totp_state(&state.db, user.user_id)
        .await?
        .ok_or(AppError::Unauthenticated)?;
    let secret = s
        .secret
        .as_deref()
        .and_then(|v| secrets::open(&state.config, v))
        .and_then(|b32| totp::base32_decode(&b32))
        .ok_or_else(|| AppError::validation("start the setup first"))?;
    let step = totp::verify(&secret, &b.code, Utc::now().timestamp(), None)
        .ok_or_else(|| AppError::rule("totp_invalid", "the one-time code is wrong"))?;
    users::enable_totp(&state.db, user.user_id, step).await?;
    tracing::info!(user_id = user.user_id, "two-factor sign-in enabled");
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post, path = "/auth/two-factor/disable", tag = "auth",
    request_body = PasswordConfirm,
    responses((status = 204, description = "Off"))
)]
async fn two_factor_disable(
    State(state): State<AppState>,
    Auth(user): Auth,
    ApiJson(b): ApiJson<PasswordConfirm>,
) -> AppResult<StatusCode> {
    confirm_password(&state, &user, b.password).await?;
    users::disable_totp(&state.db, user.user_id).await?;
    tracing::info!(user_id = user.user_id, "two-factor sign-in disabled");
    Ok(StatusCode::NO_CONTENT)
}

// --- Calendar feed (0049) ---

#[derive(Serialize, ToSchema)]
struct CalendarFeed {
    /// The secret subscription link (Google Calendar: "From URL"; Outlook: "Subscribe from
    /// web"). None until one is made.
    url: Option<String>,
}

fn feed_url(state: &AppState, token: &str) -> String {
    format!(
        "{}/api/calendar/{token}.ics",
        state.config.public_base_url.trim_end_matches('/')
    )
}

#[utoipa::path(
    get, path = "/auth/calendar", tag = "auth",
    responses((status = 200, body = CalendarFeed))
)]
async fn get_calendar(
    State(state): State<AppState>,
    Auth(user): Auth,
) -> AppResult<Json<CalendarFeed>> {
    let token = config::calendar_token(&state.db, user.user_id).await?;
    Ok(Json(CalendarFeed {
        url: token.map(|t| feed_url(&state, &t)),
    }))
}

#[utoipa::path(
    post, path = "/auth/calendar", tag = "auth",
    responses((status = 200, body = CalendarFeed, description = "A new link; the old one stops working"))
)]
async fn create_calendar(
    State(state): State<AppState>,
    Auth(user): Auth,
) -> AppResult<Json<CalendarFeed>> {
    let token = auth::generate_token();
    config::set_calendar_token(&state.db, user.user_id, Some(&token)).await?;
    Ok(Json(CalendarFeed {
        url: Some(feed_url(&state, &token)),
    }))
}

#[utoipa::path(
    delete, path = "/auth/calendar", tag = "auth",
    responses((status = 204, description = "The link stops working"))
)]
async fn delete_calendar(State(state): State<AppState>, Auth(user): Auth) -> AppResult<StatusCode> {
    config::set_calendar_token(&state.db, user.user_id, None).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get, path = "/calendar/{file}", tag = "auth", security(()),
    params(("file" = String, Path, description = "`<token>.ics`")),
    responses((status = 200, content_type = "text/calendar", body = String, description = "The person's tasks, order deadlines and leave as all-day events"))
)]
async fn calendar_feed(
    State(state): State<AppState>,
    ApiPath(file): ApiPath<String>,
) -> AppResult<Response> {
    let token = file.strip_suffix(".ics").unwrap_or(&file);
    if token.is_empty() || token.len() > 128 {
        return Err(AppError::NotFound("calendar"));
    }
    let (user_id, name) = config::calendar_owner(&state.db, token)
        .await?
        .ok_or(AppError::NotFound("calendar"))?;
    let base = state.config.public_base_url.trim_end_matches('/');
    let events: Vec<ics::Event> = crate::repo::calendar::feed(&state.db, user_id)
        .await?
        .into_iter()
        .map(|r| ics::Event {
            uid: r.uid,
            start: r.start_date,
            last_day: r.end_date,
            summary: r.summary,
            description: None,
            url: r.path.map(|p| format!("{base}/hu/{p}")),
        })
        .collect();
    let body = ics::calendar(
        &format!("AutoCRM – {name}"),
        &state.config.email.message_id_domain,
        Utc::now(),
        &events,
    );
    Ok((
        [
            (header::CONTENT_TYPE, "text/calendar; charset=utf-8"),
            (header::CACHE_CONTROL, "private, max-age=300"),
        ],
        body,
    )
        .into_response())
}

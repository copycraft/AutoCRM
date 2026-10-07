//! Custom auth flow over vetted primitives: Argon2id for passwords, 256-bit random opaque
//! tokens, sha256 of the token stored server-side.

use std::sync::OnceLock;

use anyhow::anyhow;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, TimeDelta, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::config::Config;
use crate::domain::role::{Capability, Role};
use crate::domain::totp;
use crate::error::{AppError, AppResult};
use crate::repo::sessions::{self, NewSession, SessionKind};
use crate::repo::users::{self, User};

pub const MIN_PASSWORD_CHARS: usize = 12;
const MAX_PASSWORD_CHARS: usize = 256;
/// Only re-write last_seen_at/expires_at this often, not on every request.
const TOUCH_INTERVAL: TimeDelta = TimeDelta::minutes(5);

/// (idle timeout, absolute lifetime). Mobile sessions live longer: re-typing a password on
/// a workshop phone is friction nobody will tolerate.
fn lifetimes(kind: SessionKind) -> (TimeDelta, TimeDelta) {
    match kind {
        SessionKind::Web => (TimeDelta::days(7), TimeDelta::days(30)),
        SessionKind::Mobile => (TimeDelta::days(60), TimeDelta::days(365)),
    }
}

pub fn session_expiry(
    kind: SessionKind,
    created_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> DateTime<Utc> {
    let (idle, _) = lifetimes(kind);
    (now + idle).min(session_absolute_expiry(kind, created_at))
}

/// The hard end of a session, however active it is.
pub fn session_absolute_expiry(kind: SessionKind, created_at: DateTime<Utc>) -> DateTime<Utc> {
    let (_, absolute) = lifetimes(kind);
    created_at + absolute
}

/// `Max-Age` for the web session cookie, in seconds. The cookie is set once, at login, and
/// never re-issued, so it must outlive every server-side renewal: it lasts until the
/// absolute cap and the server's `expires_at` enforces the idle timeout. Tying it to the
/// first idle window would make the browser drop an active session after 7 days.
pub fn cookie_max_age_secs(
    kind: SessionKind,
    created_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> i64 {
    (session_absolute_expiry(kind, created_at) - now)
        .num_seconds()
        .max(0)
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthUser {
    pub user_id: i64,
    pub session_id: i64,
    pub session_kind: SessionKind,
    pub email: String,
    pub display_name: String,
    pub role: Role,
    pub must_change_password: bool,
    /// Granted by an admin; unlocks the HR module on top of the role.
    pub hr_access: bool,
    /// Capabilities an admin granted this user on top of the role.
    pub permissions: Vec<Capability>,
}

impl AuthUser {
    pub fn require(&self, capability: Capability) -> AppResult<()> {
        if self.can(capability) {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }

    pub fn can(&self, capability: Capability) -> bool {
        self.role.can(capability)
            || (capability == Capability::AccessHr && self.hr_access)
            || (capability.grantable() && self.permissions.contains(&capability))
    }

    /// Everything this user may do: the role's set plus the grants.
    pub fn capabilities(&self) -> Vec<Capability> {
        Capability::ALL
            .into_iter()
            .filter(|c| self.can(*c))
            .collect()
    }
}

/// Stored grant keys to capabilities; unknown or no-longer-grantable keys are ignored.
pub fn parse_permissions(keys: &[String]) -> Vec<Capability> {
    keys.iter()
        .filter_map(|k| Capability::from_key(k))
        .filter(|c| c.grantable())
        .collect()
}

pub fn validate_new_password(password: &str) -> AppResult<()> {
    let n = password.chars().count();
    if n < MIN_PASSWORD_CHARS {
        return Err(AppError::validation(format!(
            "password must be at least {MIN_PASSWORD_CHARS} characters"
        )));
    }
    if n > MAX_PASSWORD_CHARS {
        return Err(AppError::validation("password is too long"));
    }
    Ok(())
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt_bytes: [u8; 16] = rand::random();
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|e| anyhow!("salt encoding: {e}"))?;
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow!("password hashing: {e}"))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, phc_hash: &str) -> bool {
    PasswordHash::new(phc_hash).is_ok_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}

/// Argon2 is deliberately expensive; keep it off the async executor threads.
pub async fn hash_password_async(password: String) -> anyhow::Result<String> {
    tokio::task::spawn_blocking(move || hash_password(&password)).await?
}

pub async fn verify_password_async(password: String, phc_hash: String) -> anyhow::Result<bool> {
    Ok(tokio::task::spawn_blocking(move || verify_password(&password, &phc_hash)).await?)
}

/// Verifying against this when the email doesn't exist keeps response timing the same,
/// so login can't be used to discover which addresses have accounts.
fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        hash_password("timing-equaliser-not-a-real-password").expect("hashing works")
    })
}

pub fn generate_token() -> String {
    let bytes: [u8; 32] = rand::random();
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub struct LoginRequest {
    pub email: String,
    pub password: String,
    pub kind: SessionKind,
    pub device_label: Option<String>,
    pub user_agent: Option<String>,
    pub ip: Option<String>,
    /// The authenticator app's code, for an account with two-factor sign-in (0049).
    pub totp_code: Option<String>,
}

pub struct LoginOutcome {
    pub token: String,
    pub session_id: i64,
    pub expires_at: DateTime<Utc>,
    pub user: User,
    /// The account had signed in before, but never from this device: the caller tells the
    /// owner by email.
    pub new_device: bool,
}

/// Checks a typed code against the account's secret and burns its time step. Errors are the
/// login's: `totp_required` when no code came, `totp_invalid` for a wrong or reused one.
pub async fn check_totp(
    db: &PgPool,
    config: &Config,
    user_id: i64,
    code: Option<&str>,
) -> AppResult<()> {
    let Some(state) = users::totp_state(db, user_id).await? else {
        return Err(AppError::Unauthenticated);
    };
    if !state.enabled {
        return Ok(());
    }
    let Some(code) = code.map(str::trim).filter(|c| !c.is_empty()) else {
        return Err(AppError::rule(
            "totp_required",
            "this account signs in with a one-time code too",
        ));
    };
    let secret = state
        .secret
        .as_deref()
        .and_then(|s| crate::service::secrets::open(config, s))
        .and_then(|b32| totp::base32_decode(&b32))
        .ok_or_else(|| {
            AppError::internal(format!("two-factor secret of user {user_id} is unreadable"))
        })?;
    let step = totp::verify(&secret, code, Utc::now().timestamp(), state.last_step);
    match step {
        Some(step) if users::use_totp_step(db, user_id, step).await? => Ok(()),
        _ => {
            users::record_failed_login(db, user_id).await?;
            Err(AppError::rule(
                "totp_invalid",
                "the one-time code is wrong or was already used",
            ))
        }
    }
}

/// Opens a session for a user whose identity is already proven (password and code, or
/// Google). Reports whether the device is new to the account.
pub async fn open_session(
    db: &PgPool,
    user_id: i64,
    kind: SessionKind,
    device_label: Option<&str>,
    user_agent: Option<&str>,
    ip: Option<&str>,
) -> AppResult<LoginOutcome> {
    let (signed_in_before, seen) =
        sessions::device_seen(db, user_id, device_label, user_agent).await?;
    let token = generate_token();
    let hash = token_hash(&token);
    let now = Utc::now();
    let expires_at = session_expiry(kind, now, now);
    let session_id = sessions::insert(
        db,
        NewSession {
            user_id,
            token_hash: &hash,
            kind,
            device_label,
            user_agent,
            ip,
            expires_at,
        },
    )
    .await?;
    let user = users::find(db, user_id)
        .await?
        .ok_or(AppError::Unauthenticated)?;
    Ok(LoginOutcome {
        token,
        session_id,
        expires_at,
        user,
        new_device: signed_in_before && !seen,
    })
}

pub async fn login(db: &PgPool, config: &Config, req: LoginRequest) -> AppResult<LoginOutcome> {
    let Some(creds) = users::find_credentials_by_email(db, &req.email).await? else {
        verify_password_async(req.password, dummy_hash().to_string()).await?;
        return Err(AppError::Unauthenticated);
    };
    // Verify before looking at the lock, and answer a locked account with a wrong password
    // exactly like an unknown address: same Argon2 time, same 401. Otherwise locking an
    // address (ten bad guesses) and seeing 429 would tell a stranger it has an account.
    // Only the right password on a locked account learns about the lock, and it is refused.
    let valid = verify_password_async(req.password, creds.password_hash).await?;
    if creds.locked_until.is_some_and(|until| until > Utc::now()) {
        return Err(if valid && creds.is_active {
            AppError::TooManyRequests
        } else {
            AppError::Unauthenticated
        });
    }
    if !valid {
        users::record_failed_login(db, creds.id).await?;
        return Err(AppError::Unauthenticated);
    }
    if !creds.is_active {
        return Err(AppError::Unauthenticated);
    }
    // The password was right; a wrong code counts as a failed login like a wrong password,
    // so guessing codes runs into the same lock.
    check_totp(db, config, creds.id, req.totp_code.as_deref()).await?;
    users::record_login_success(db, creds.id).await?;
    open_session(
        db,
        creds.id,
        req.kind,
        req.device_label.as_deref(),
        req.user_agent.as_deref(),
        req.ip.as_deref(),
    )
    .await
}

pub async fn authenticate(db: &PgPool, token: &str) -> AppResult<AuthUser> {
    if token.is_empty() || token.len() > 128 {
        return Err(AppError::Unauthenticated);
    }
    let session = sessions::find_active(db, &token_hash(token))
        .await?
        .ok_or(AppError::Unauthenticated)?;
    let now = Utc::now();
    // A password-blocked session must not be kept alive by hitting blocked endpoints:
    // without this, a user who never changes their password extends it forever and the
    // idle timeout never fires. Blocked sessions expire naturally instead.
    if !session.must_change_password && now - session.last_seen_at > TOUCH_INTERVAL {
        sessions::touch(
            db,
            session.session_id,
            session_expiry(session.kind, session.created_at, now),
        )
        .await?;
    }
    Ok(AuthUser {
        user_id: session.user_id,
        session_id: session.session_id,
        session_kind: session.kind,
        email: session.email,
        display_name: session.display_name,
        role: session.role,
        must_change_password: session.must_change_password,
        hr_access: session.hr_access,
        permissions: parse_permissions(&session.permissions),
    })
}

/// Changes the caller's own password and signs out every other device.
pub async fn change_password(
    db: &PgPool,
    auth: &AuthUser,
    current: String,
    new: String,
) -> AppResult<()> {
    validate_new_password(&new)?;
    let creds = users::find_credentials(db, auth.user_id)
        .await?
        .ok_or(AppError::Unauthenticated)?;
    if !verify_password_async(current, creds.password_hash).await? {
        return Err(AppError::rule(
            "wrong_password",
            "current password is incorrect",
        ));
    }
    let hash = hash_password_async(new).await?;
    let mut tx = db.begin().await?;
    users::set_password(&mut *tx, auth.user_id, &hash, false).await?;
    sessions::revoke_all(&mut *tx, auth.user_id, Some(auth.session_id)).await?;
    tx.commit().await?;
    Ok(())
}

/// A device as a person recognises it: the phone's own label, or "Chrome, Windows" from the
/// browser string. Never empty.
pub fn describe_device(device_label: Option<&str>, user_agent: Option<&str>) -> String {
    if let Some(label) = device_label.map(str::trim).filter(|l| !l.is_empty()) {
        return format!("{label} (AutoCRM mobilalkalmazás)");
    }
    let Some(ua) = user_agent.filter(|u| !u.trim().is_empty()) else {
        return "ismeretlen eszköz".into();
    };
    let browser = if ua.contains("Edg/") {
        "Edge"
    } else if ua.contains("OPR/") || ua.contains("Opera") {
        "Opera"
    } else if ua.contains("Firefox/") {
        "Firefox"
    } else if ua.contains("Chrome/") || ua.contains("CriOS/") {
        "Chrome"
    } else if ua.contains("Safari/") {
        "Safari"
    } else if ua.contains("okhttp") {
        "AutoCRM mobilalkalmazás"
    } else {
        "ismeretlen böngésző"
    };
    let os = if ua.contains("Android") {
        "Android"
    } else if ua.contains("iPhone") || ua.contains("iPad") {
        "iOS"
    } else if ua.contains("Windows") {
        "Windows"
    } else if ua.contains("Mac OS X") || ua.contains("Macintosh") {
        "macOS"
    } else if ua.contains("Linux") {
        "Linux"
    } else {
        "ismeretlen rendszer"
    };
    format!("{browser}, {os}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_read_like_people_say_them() {
        let chrome = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0 Safari/537.36";
        assert_eq!(describe_device(None, Some(chrome)), "Chrome, Windows");
        let edge =
            "Mozilla/5.0 (Windows NT 10.0) AppleWebKit/537.36 Chrome/129.0 Safari/537.36 Edg/129.0";
        assert_eq!(describe_device(None, Some(edge)), "Edge, Windows");
        assert_eq!(
            describe_device(Some("Pixel 8"), Some("okhttp/4.12")),
            "Pixel 8 (AutoCRM mobilalkalmazás)"
        );
        assert_eq!(describe_device(None, None), "ismeretlen eszköz");
    }

    #[test]
    fn password_round_trip() {
        let hash = hash_password("correct horse battery").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password("correct horse battery", &hash));
        assert!(!verify_password("wrong horse battery", &hash));
        assert!(!verify_password("anything", "not a phc string"));
    }

    #[test]
    fn tokens_are_unique_and_hash_stably() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert_eq!(token_hash(&a), token_hash(&a));
        assert_eq!(token_hash(&a).len(), 32);
    }

    #[test]
    fn expiry_is_capped_by_absolute_lifetime() {
        let created: DateTime<Utc> = "2026-01-01T00:00:00Z".parse().unwrap();
        let early = created + TimeDelta::days(1);
        assert_eq!(
            session_expiry(SessionKind::Web, created, early),
            early + TimeDelta::days(7)
        );
        let late = created + TimeDelta::days(28);
        assert_eq!(
            session_expiry(SessionKind::Web, created, late),
            created + TimeDelta::days(30)
        );
    }

    #[test]
    fn web_cookie_outlives_the_idle_window_until_the_absolute_cap() {
        // DECISIONS.md: web sessions are 7-day idle / 30-day absolute. The cookie is only
        // set at login, so an active user must keep it for 30 days, not 7.
        let now: DateTime<Utc> = "2026-01-01T00:00:00Z".parse().unwrap();
        let idle_expiry = session_expiry(SessionKind::Web, now, now);
        assert_eq!(idle_expiry, now + TimeDelta::days(7));
        assert_eq!(
            cookie_max_age_secs(SessionKind::Web, now, now),
            TimeDelta::days(30).num_seconds()
        );
        assert_eq!(
            cookie_max_age_secs(SessionKind::Web, now, now + TimeDelta::days(31)),
            0
        );
    }

    #[test]
    fn password_policy() {
        assert!(validate_new_password("short").is_err());
        assert!(validate_new_password("twelve chars").is_ok());
    }
}

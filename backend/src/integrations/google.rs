//! "Sign in with Google" for the company's Workspace accounts: OpenID Connect, the
//! authorization-code flow.
//!
//! The ID token is taken from Google's token endpoint over TLS, in exchange for a code only
//! our client secret can redeem, so its signature is not checked again (OIDC Core 3.1.3.7
//! allows this for tokens received directly from the token endpoint). Its claims still are:
//! audience, issuer, expiry, nonce, verified email, and the Workspace domain.

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;

use crate::config::GoogleConfig;

const AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Where the browser goes to choose an account.
pub fn authorize_url(cfg: &GoogleConfig, redirect_uri: &str, state: &str, nonce: &str) -> String {
    let mut url = format!(
        "{AUTHORIZE_URL}?client_id={}&response_type=code&scope={}&redirect_uri={}&state={}&nonce={}&prompt=select_account",
        enc(&cfg.client_id),
        enc("openid email profile"),
        enc(redirect_uri),
        enc(state),
        enc(nonce),
    );
    if let Some(domain) = &cfg.domain {
        // A hint only (preselects the domain); the `hd` claim is what is enforced.
        url.push_str(&format!("&hd={}", enc(domain)));
    }
    url
}

#[derive(Debug, Deserialize)]
pub struct Claims {
    pub iss: String,
    pub aud: String,
    pub exp: i64,
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: bool,
    pub hd: Option<String>,
    pub nonce: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum GoogleError {
    #[error("the token exchange failed: {0}")]
    Exchange(String),
    #[error("the identity token is not acceptable: {0}")]
    Claims(&'static str),
}

#[derive(Deserialize)]
struct TokenResponse {
    id_token: Option<String>,
}

/// Redeems the code and returns the verified email address of the account.
pub async fn exchange(
    cfg: &GoogleConfig,
    redirect_uri: &str,
    code: &str,
    nonce: &str,
    now_unix: i64,
) -> Result<String, GoogleError> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| GoogleError::Exchange(e.to_string()))?;
    let response = http
        .post(TOKEN_URL)
        .form(&[
            ("code", code),
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| GoogleError::Exchange(e.to_string()))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(GoogleError::Exchange(format!(
            "{status}: {}",
            body.chars().take(300).collect::<String>()
        )));
    }
    let token: TokenResponse = response
        .json()
        .await
        .map_err(|e| GoogleError::Exchange(e.to_string()))?;
    let id_token = token.id_token.ok_or(GoogleError::Claims("no id_token"))?;
    let claims = decode_claims(&id_token)?;
    check_claims(cfg, &claims, nonce, now_unix)
}

pub fn decode_claims(id_token: &str) -> Result<Claims, GoogleError> {
    let payload = id_token
        .split('.')
        .nth(1)
        .ok_or(GoogleError::Claims("malformed"))?;
    let raw = URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .map_err(|_| GoogleError::Claims("malformed"))?;
    serde_json::from_slice(&raw).map_err(|_| GoogleError::Claims("malformed"))
}

/// The address, when every claim checks out. Pure, so it is tested without Google.
pub fn check_claims(
    cfg: &GoogleConfig,
    claims: &Claims,
    nonce: &str,
    now_unix: i64,
) -> Result<String, GoogleError> {
    if claims.iss != "https://accounts.google.com" && claims.iss != "accounts.google.com" {
        return Err(GoogleError::Claims("issuer"));
    }
    if claims.aud != cfg.client_id {
        return Err(GoogleError::Claims("audience"));
    }
    if claims.exp + 60 < now_unix {
        return Err(GoogleError::Claims("expired"));
    }
    if claims.nonce.as_deref() != Some(nonce) {
        return Err(GoogleError::Claims("nonce"));
    }
    if !claims.email_verified {
        return Err(GoogleError::Claims("email not verified"));
    }
    if let Some(domain) = &cfg.domain
        && claims.hd.as_deref().map(str::to_ascii_lowercase).as_deref() != Some(domain.as_str())
    {
        return Err(GoogleError::Claims("workspace domain"));
    }
    claims
        .email
        .clone()
        .filter(|e| e.contains('@'))
        .ok_or(GoogleError::Claims("no email"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> GoogleConfig {
        GoogleConfig {
            client_id: "client.apps.googleusercontent.com".into(),
            client_secret: "secret".into(),
            domain: Some("autotherm.hu".into()),
        }
    }

    fn claims() -> Claims {
        Claims {
            iss: "https://accounts.google.com".into(),
            aud: "client.apps.googleusercontent.com".into(),
            exp: 2_000,
            email: Some("kovacs@autotherm.hu".into()),
            email_verified: true,
            hd: Some("autotherm.hu".into()),
            nonce: Some("n1".into()),
        }
    }

    #[test]
    fn good_claims_give_the_address() {
        assert_eq!(
            check_claims(&cfg(), &claims(), "n1", 1_000).unwrap(),
            "kovacs@autotherm.hu"
        );
    }

    #[test]
    fn every_claim_is_enforced() {
        let c = cfg();
        let mut x = claims();
        x.aud = "other".into();
        assert!(check_claims(&c, &x, "n1", 1_000).is_err());
        let mut x = claims();
        x.hd = None;
        assert!(
            check_claims(&c, &x, "n1", 1_000).is_err(),
            "a private gmail account"
        );
        let mut x = claims();
        x.email_verified = false;
        assert!(check_claims(&c, &x, "n1", 1_000).is_err());
        assert!(check_claims(&c, &claims(), "n2", 1_000).is_err(), "nonce");
        assert!(check_claims(&c, &claims(), "n1", 5_000).is_err(), "expired");
    }

    #[test]
    fn the_payload_decodes() {
        let payload = URL_SAFE_NO_PAD.encode(
            br#"{"iss":"accounts.google.com","aud":"a","exp":1,"email":"x@y.hu","email_verified":true}"#,
        );
        let c = decode_claims(&format!("h.{payload}.s")).unwrap();
        assert_eq!(c.email.as_deref(), Some("x@y.hu"));
        assert!(c.hd.is_none());
    }

    #[test]
    fn the_authorize_url_carries_state_and_domain_hint() {
        let url = authorize_url(
            &cfg(),
            "https://crm.example/api/auth/google/callback",
            "s1",
            "n1",
        );
        assert!(url.contains("state=s1"));
        assert!(url.contains("hd=autotherm.hu"));
        assert!(
            url.contains("redirect_uri=https%3A%2F%2Fcrm.example%2Fapi%2Fauth%2Fgoogle%2Fcallback")
        );
    }
}

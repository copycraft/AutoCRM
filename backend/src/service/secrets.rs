//! Secrets kept in the database (the SMTP password in the settings row, two-factor seeds)
//! are sealed with AES-256-GCM. A database dump or backup then holds ciphertext; the key
//! lives in the process environment, next to the database URL.
//!
//! The key is derived (HKDF-SHA256) from `SECRETS_KEY`, or from `UPLOAD_SIGNING_KEY` when
//! that is unset, so existing installations need no new variable. Set `SECRETS_KEY` before
//! rotating the upload key, or the sealed values become unreadable.
//!
//! Sealed values read `v1:` + base64(nonce ‖ ciphertext ‖ tag). Anything else is treated as a
//! legacy plain value and sealed on the next write.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};

use sqlx::PgPool;

use crate::config::Config;

const PREFIX: &str = "v1:";

fn key_bytes(config: &Config) -> [u8; 32] {
    let ikm = config
        .secrets_key
        .as_deref()
        .map(str::as_bytes)
        .unwrap_or(&config.upload_signing_key);
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(Some(b"autocrm-secrets-v1"), ikm);
    let mut out = [0u8; 32];
    hk.expand(b"settings and two-factor seeds", &mut out)
        .expect("32 bytes is a valid HKDF output length");
    out
}

fn key(config: &Config) -> LessSafeKey {
    LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &key_bytes(config)).expect("a 32-byte key"))
}

pub fn is_sealed(value: &str) -> bool {
    value.starts_with(PREFIX)
}

pub fn seal(config: &Config, plain: &str) -> String {
    let nonce_bytes: [u8; NONCE_LEN] = rand::random();
    let mut data = plain.as_bytes().to_vec();
    key(config)
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::empty(),
            &mut data,
        )
        .expect("sealing cannot fail for in-memory data");
    let mut out = nonce_bytes.to_vec();
    out.extend(data);
    format!("{PREFIX}{}", STANDARD.encode(out))
}

/// The plain value of a sealed one; a legacy plain value is returned as it is. None when a
/// sealed value cannot be opened (wrong key, damaged).
pub fn open(config: &Config, stored: &str) -> Option<String> {
    let Some(encoded) = stored.strip_prefix(PREFIX) else {
        return Some(stored.to_string());
    };
    let raw = STANDARD.decode(encoded).ok()?;
    if raw.len() < NONCE_LEN + 16 {
        return None;
    }
    let (nonce, rest) = raw.split_at(NONCE_LEN);
    let mut data = rest.to_vec();
    let plain = key(config)
        .open_in_place(
            Nonce::try_assume_unique_for_key(nonce).ok()?,
            Aad::empty(),
            &mut data,
        )
        .ok()?;
    String::from_utf8(plain.to_vec()).ok()
}

/// Seals what was stored in the clear before sealing existed: the SMTP password and any
/// two-factor secret. Idempotent.
pub async fn seal_legacy(db: &PgPool, config: &Config) -> sqlx::Result<()> {
    if let Some(stored) = crate::repo::config::email_secret(db).await?
        && !is_sealed(&stored)
    {
        crate::repo::config::set_email_secret(db, &seal(config, &stored)).await?;
        tracing::info!("sealed the stored SMTP password");
    }
    for (user_id, plain) in crate::repo::users::plain_totp_secrets(db).await? {
        crate::repo::users::reseal_totp(db, user_id, &seal(config, &plain)).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        // Only the two key fields matter here; the rest is borrowed from the env-free test
        // defaults of `Config` via a minimal environment.
        let mut c = crate::config::test_config();
        c.upload_signing_key = b"0123456789abcdef0123456789abcdef".to_vec();
        c
    }

    #[test]
    fn a_sealed_value_opens_and_differs_each_time() {
        let c = config();
        let a = seal(&c, "titkos-jelszo");
        let b = seal(&c, "titkos-jelszo");
        assert!(is_sealed(&a));
        assert_ne!(a, b, "a fresh nonce every time");
        assert_eq!(open(&c, &a).as_deref(), Some("titkos-jelszo"));
    }

    #[test]
    fn a_legacy_plain_value_reads_as_itself_and_a_wrong_key_reads_nothing() {
        let c = config();
        assert_eq!(open(&c, "regi-jelszo").as_deref(), Some("regi-jelszo"));
        let sealed = seal(&c, "x");
        let mut other = config();
        other.secrets_key = Some("another key entirely, 32+ chars long".into());
        assert_eq!(open(&other, &sealed), None);
    }
}

//! Stateless upload tickets. The server signs what it agreed to accept (order, category,
//! key, hash, size); the client uploads straight to object storage, then hands the ticket
//! back to finalize. No pending-uploads table needed, and a ticket cannot be altered to
//! point at another order or a different file.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::domain::media::{DocumentKind, ImageCategory};

type HmacSha256 = Hmac<Sha256>;

/// Domain separation: this key signs upload tickets and nothing else.
const CONTEXT: &[u8] = b"autocrm-upload-ticket-v1.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum UploadTarget {
    Image { category: ImageCategory },
    Document { kind: DocumentKind },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadClaims {
    pub target: UploadTarget,
    /// Exactly one of these is set. Images are always order-scoped; only documents can
    /// belong to a lead (V2.4).
    pub order_id: Option<i64>,
    pub lead_id: Option<i64>,
    pub user_id: i64,
    pub storage_key: String,
    pub sha256_hex: String,
    pub byte_size: i64,
    pub content_type: String,
    pub original_filename: Option<String>,
    /// Unix seconds.
    pub expires_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TicketError {
    #[error("malformed upload ticket")]
    Malformed,
    #[error("upload ticket signature is invalid")]
    BadSignature,
    #[error("upload ticket has expired; request a new upload")]
    Expired,
}

fn mac(key: &[u8], payload: &str) -> HmacSha256 {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(CONTEXT);
    mac.update(payload.as_bytes());
    mac
}

pub fn sign(key: &[u8], claims: &UploadClaims) -> String {
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).expect("claims serialize"));
    let signature = URL_SAFE_NO_PAD.encode(mac(key, &payload).finalize().into_bytes());
    format!("{payload}.{signature}")
}

pub fn verify(key: &[u8], ticket: &str, now_unix: i64) -> Result<UploadClaims, TicketError> {
    let (payload, signature) = ticket.split_once('.').ok_or(TicketError::Malformed)?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| TicketError::Malformed)?;
    // verify_slice is constant-time.
    mac(key, payload)
        .verify_slice(&signature)
        .map_err(|_| TicketError::BadSignature)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| TicketError::Malformed)?;
    let claims: UploadClaims =
        serde_json::from_slice(&bytes).map_err(|_| TicketError::Malformed)?;
    if claims.expires_at < now_unix {
        return Err(TicketError::Expired);
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &[u8] = b"test-key-test-key-test-key-test-key!";

    fn claims() -> UploadClaims {
        UploadClaims {
            target: UploadTarget::Image {
                category: ImageCategory::Intake,
            },
            order_id: Some(7),
            lead_id: None,
            user_id: 3,
            storage_key: "orders/7/intake/ab.jpg".into(),
            sha256_hex: "ab".into(),
            byte_size: 1234,
            content_type: "image/jpeg".into(),
            original_filename: Some("IMG_0001.jpg".into()),
            expires_at: 1_000,
        }
    }

    #[test]
    fn round_trip() {
        let t = sign(KEY, &claims());
        assert_eq!(verify(KEY, &t, 999), Ok(claims()));
    }

    #[test]
    fn expired_rejected() {
        let t = sign(KEY, &claims());
        assert_eq!(verify(KEY, &t, 1_001), Err(TicketError::Expired));
    }

    #[test]
    fn tampering_rejected() {
        let t = sign(KEY, &claims());
        let mut other = claims();
        other.order_id = Some(8);
        let forged_payload = sign(KEY, &other).split_once('.').unwrap().0.to_string();
        let original_sig = t.split_once('.').unwrap().1;
        assert_eq!(
            verify(KEY, &format!("{forged_payload}.{original_sig}"), 0),
            Err(TicketError::BadSignature)
        );
        assert_eq!(
            verify(b"another-key-another-key-another-key", &t, 0),
            Err(TicketError::BadSignature)
        );
        assert_eq!(verify(KEY, "garbage", 0), Err(TicketError::Malformed));
    }
}

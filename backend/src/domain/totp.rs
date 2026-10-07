//! One-time codes from an authenticator app (RFC 6238 TOTP: HMAC-SHA1, 30-second steps,
//! 6 digits), and the base32 the secret is shown in. Pure: no clock, no storage.

use hmac::{Hmac, Mac};
use sha1::Sha1;

pub const STEP_SECONDS: i64 = 30;
pub const DIGITS: u32 = 6;
/// Codes one step either side of now are accepted: phone clocks drift.
pub const WINDOW: i64 = 1;

const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

pub fn base32_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for &b in bytes {
        buffer = (buffer << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            out.push(ALPHABET[((buffer >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}

pub fn base32_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for c in text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '=' && *c != '-')
    {
        let v = ALPHABET
            .iter()
            .position(|&a| a as char == c.to_ascii_uppercase())? as u32;
        buffer = (buffer << 5) | v;
        bits += 5;
        if bits >= 8 {
            out.push(((buffer >> (bits - 8)) & 0xff) as u8);
            bits -= 8;
        }
    }
    Some(out)
}

/// The code for time step `step` (unix seconds / 30).
pub fn code_at(secret: &[u8], step: i64) -> String {
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).expect("HMAC takes any key length");
    mac.update(&step.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let binary = (u32::from(digest[offset] & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);
    format!(
        "{:0width$}",
        binary % 10u32.pow(DIGITS),
        width = DIGITS as usize
    )
}

/// The step a typed code matches, if any, within the window around `unix_seconds` and
/// after `last_step` (a code is good once).
pub fn verify(secret: &[u8], code: &str, unix_seconds: i64, last_step: Option<i64>) -> Option<i64> {
    let code: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
    if code.len() != DIGITS as usize {
        return None;
    }
    let now = unix_seconds.div_euclid(STEP_SECONDS);
    (now - WINDOW..=now + WINDOW)
        .filter(|step| last_step.is_none_or(|last| *step > last))
        .find(|step| code_at(secret, *step) == code)
}

/// The link an authenticator app reads (as a QR code, or tapped on the phone).
pub fn otpauth_url(issuer: &str, account: &str, secret_b32: &str) -> String {
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
    format!(
        "otpauth://totp/{}:{}?secret={secret_b32}&issuer={}&algorithm=SHA1&digits={DIGITS}&period={STEP_SECONDS}",
        enc(issuer),
        enc(account),
        enc(issuer)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 6238 appendix B, SHA1, 8 digits truncated to our 6.
    #[test]
    fn rfc6238_vectors() {
        let secret = b"12345678901234567890";
        assert_eq!(code_at(secret, 59 / 30), "287082");
        assert_eq!(code_at(secret, 1_111_111_109 / 30), "081804");
        assert_eq!(code_at(secret, 1_234_567_890 / 30), "005924");
    }

    #[test]
    fn base32_round_trips() {
        let raw = b"\x01\x02autocrm\xff";
        assert_eq!(base32_decode(&base32_encode(raw)).unwrap(), raw);
        assert_eq!(base32_encode(b"foobar"), "MZXW6YTBOI");
        assert_eq!(base32_decode("mzxw 6ytb-oi").unwrap(), b"foobar");
        assert!(base32_decode("not base32!").is_none());
    }

    #[test]
    fn a_code_is_good_within_the_window_and_only_once() {
        let secret = b"12345678901234567890";
        let now = 1_234_567_890;
        let code = code_at(secret, now / 30);
        let step = verify(secret, &code, now + 25, None).unwrap();
        assert_eq!(step, now / 30);
        assert!(
            verify(secret, &code, now + 25, Some(step)).is_none(),
            "used once"
        );
        assert!(verify(secret, &code, now + 120, None).is_none(), "too late");
        assert!(verify(secret, "12345", now, None).is_none());
    }

    #[test]
    fn the_app_link_names_issuer_and_account() {
        let url = otpauth_url("AutoCRM", "kovacs@autotherm.hu", "MZXW6YTBOI");
        assert!(url.starts_with("otpauth://totp/AutoCRM:kovacs%40autotherm.hu?secret=MZXW6YTBOI"));
    }
}

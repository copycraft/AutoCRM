//! Partners: businesses and people, unified (mirrors MiniCRM's Business/Person).

use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[sqlx(type_name = "partner_kind", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum PartnerKind {
    Business,
    Person,
}

/// Hungarian tax numbers are 11 digits, conventionally written 12345678-1-23.
/// Accepts any spacing/dash variant; returns the canonical form.
pub fn normalize_hu_tax_number(raw: &str) -> Result<String, &'static str> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    let only_separators = raw
        .chars()
        .all(|c| c.is_ascii_digit() || c == '-' || c == ' ');
    if digits.len() != 11 || !only_separators {
        return Err("Hungarian tax number must be 11 digits (12345678-1-23)");
    }
    Ok(format!(
        "{}-{}-{}",
        &digits[..8],
        &digits[8..9],
        &digits[9..]
    ))
}

pub fn normalize_country(raw: &str) -> Option<String> {
    let c = raw.trim().to_ascii_uppercase();
    (c.len() == 2 && c.chars().all(|ch| ch.is_ascii_uppercase())).then_some(c)
}

/// Phone numbers are matched on digits with Hungarian prefixes unified, so a
/// caller reading "+36 30 ..." off a phone display finds "06-30-..." as stored:
/// "+36 30 123 4567", "06 30 123 4567" and "0036 30 123 4567" all become
/// "36301234567". A prefix-less "30 123 4567" stays as-is and still
/// substring-matches. Foreign numbers pass through untouched.
/// The SQL side mirrors this exactly (strip non-digits, strip a leading 00,
/// rewrite a leading 06 to 36) in each phone predicate — keep them in sync.
/// Must stay in sync with the phone predicates in repo::{partners, leads, search}.
pub fn normalize_phone(raw: &str) -> String {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    let no_trunk = digits.strip_prefix("00").unwrap_or(&digits);
    if let Some(rest) = no_trunk.strip_prefix("06").filter(|r| !r.is_empty()) {
        format!("36{rest}")
    } else {
        no_trunk.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tax_number_formats() {
        assert_eq!(
            normalize_hu_tax_number("12345678-1-23").unwrap(),
            "12345678-1-23"
        );
        assert_eq!(
            normalize_hu_tax_number("12345678123").unwrap(),
            "12345678-1-23"
        );
        assert_eq!(
            normalize_hu_tax_number(" 12345678 1 23 ").unwrap(),
            "12345678-1-23"
        );
        assert!(normalize_hu_tax_number("1234").is_err());
        assert!(normalize_hu_tax_number("ATU12345678").is_err());
    }

    #[test]
    fn countries() {
        assert_eq!(normalize_country("at").as_deref(), Some("AT"));
        assert_eq!(normalize_country("DEU"), None);
    }

    #[test]
    fn phones_normalise_hungarian_prefixes() {
        assert_eq!(normalize_phone("+36 30 123 4567"), "36301234567");
        assert_eq!(normalize_phone("06-30-123-4567"), "36301234567");
        assert_eq!(normalize_phone("0036 30 123 4567"), "36301234567");
        // Prefix-less local form stays as-is and substring-matches.
        assert_eq!(normalize_phone("30 123 4567"), "301234567");
        assert_eq!(normalize_phone("+43 664 123456"), "43664123456");
        assert_eq!(normalize_phone("0049 30 1234"), "49301234");
        assert_eq!(normalize_phone("  "), "");
        // Degenerate prefixes don't fabricate a country code.
        assert_eq!(normalize_phone("06"), "06");
    }
}

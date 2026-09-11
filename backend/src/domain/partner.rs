//! Partners: businesses and people, unified (mirrors MiniCRM's Business/Person).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
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
}

//! Turning whatever the user typed into the (country, number) pair VIES expects.
//! Only format checks happen here; VIES is the authority on validity.

use serde::Serialize;

/// VIES member-state codes. Greece is EL, Northern Ireland is XI.
pub const MEMBER_STATES: &[&str] = &[
    "AT", "BE", "BG", "CY", "CZ", "DE", "DK", "EE", "EL", "ES", "FI", "FR", "HR", "HU", "IE", "IT",
    "LT", "LU", "LV", "MT", "NL", "PL", "PT", "RO", "SE", "SI", "SK", "XI",
];

pub const HINT_HU_VAT_GROUP_MEMBER: &str = "HU_VAT_GROUP_MEMBER";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NormalizedVat {
    pub country_code: String,
    /// Without the country prefix.
    pub vat_number: String,
    /// `HU12345678`; the cache and dedupe key.
    pub formatted: String,
    /// The full 11-digit adószám when one was entered (display and NAV).
    pub hu_domestic_tax_number: Option<String>,
    pub hints: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatError {
    MissingCountry,
    UnsupportedCountry,
    FormatInvalid,
}

impl FormatError {
    pub fn code(self) -> &'static str {
        match self {
            FormatError::MissingCountry => "MISSING_COUNTRY",
            FormatError::UnsupportedCountry => "UNSUPPORTED_COUNTRY",
            FormatError::FormatInvalid => "FORMAT_INVALID",
        }
    }
}

/// ISO uses GR for Greece; VIES uses EL.
fn vies_country(code: &str) -> String {
    let upper = code.trim().to_ascii_uppercase();
    if upper == "GR" { "EL".into() } else { upper }
}

/// `default_country` is the record's country field (ISO or VIES code), used when the input
/// has no two-letter prefix.
pub fn normalize_vat(
    input: &str,
    default_country: Option<&str>,
) -> Result<NormalizedVat, FormatError> {
    let cleaned: String = input
        .trim()
        .to_uppercase()
        .chars()
        .filter(|c| !matches!(c, ' ' | '.' | '-' | '/' | '_' | ','))
        .collect();

    let bytes = cleaned.as_bytes();
    let (country, number) =
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1].is_ascii_alphabetic() {
            (vies_country(&cleaned[..2]), cleaned[2..].to_string())
        } else {
            let default = default_country
                .filter(|c| !c.trim().is_empty())
                .ok_or(FormatError::MissingCountry)?;
            (vies_country(default), cleaned)
        };
    if !MEMBER_STATES.contains(&country.as_str()) {
        return Err(FormatError::UnsupportedCountry);
    }

    let allowed = |c: char| c.is_ascii_digit() || c.is_ascii_uppercase() || c == '+' || c == '*';
    if !(2..=12).contains(&number.len()) || !number.chars().all(allowed) {
        return Err(FormatError::FormatInvalid);
    }

    let mut hints = Vec::new();
    let mut domestic = None;
    let mut number = number;
    if country == "HU" {
        if number.len() == 11 && number.chars().all(|c| c.is_ascii_digit()) {
            // 12345678-2-06: the first 8 digits (törzsszám) are the közösségi adószám;
            // ÁFA-kód 4 marks a VAT group member.
            if number.as_bytes()[8] == b'4' {
                hints.push(HINT_HU_VAT_GROUP_MEMBER.to_string());
            }
            domestic = Some(number.clone());
            number.truncate(8);
        }
        if number.len() != 8 || !number.chars().all(|c| c.is_ascii_digit()) {
            return Err(FormatError::FormatInvalid);
        }
    }

    Ok(NormalizedVat {
        formatted: format!("{country}{number}"),
        country_code: country,
        vat_number: number,
        hu_domestic_tax_number: domestic,
        hints,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(input: &str, default: Option<&str>) -> NormalizedVat {
        normalize_vat(input, default).unwrap_or_else(|e| panic!("{input}: {e:?}"))
    }

    #[test]
    fn spec_table() {
        let n = ok("hu 1234 5678", None);
        assert_eq!(
            (n.country_code.as_str(), n.vat_number.as_str()),
            ("HU", "12345678")
        );
        assert_eq!(n.formatted, "HU12345678");
        assert_eq!(n.hu_domestic_tax_number, None);

        for (input, default) in [("12345678-2-06", Some("HU")), ("HU12345678-2-06", None)] {
            let n = ok(input, default);
            assert_eq!(
                (n.country_code.as_str(), n.vat_number.as_str()),
                ("HU", "12345678")
            );
            assert_eq!(n.hu_domestic_tax_number.as_deref(), Some("12345678206"));
            assert!(n.hints.is_empty());
        }

        let n = ok("12345678-4-06", Some("HU"));
        assert_eq!(n.vat_number, "12345678");
        assert_eq!(n.hints, vec![HINT_HU_VAT_GROUP_MEMBER]);

        let n = ok("GR123456789", None);
        assert_eq!(
            (n.country_code.as_str(), n.vat_number.as_str()),
            ("EL", "123456789")
        );
        let n = ok("DE 123.456.789", None);
        assert_eq!(
            (n.country_code.as_str(), n.vat_number.as_str()),
            ("DE", "123456789")
        );
        let n = ok("ATU12345678", None);
        assert_eq!(
            (n.country_code.as_str(), n.vat_number.as_str()),
            ("AT", "U12345678")
        );
        let n = ok("NL123456789B01", None);
        assert_eq!(
            (n.country_code.as_str(), n.vat_number.as_str()),
            ("NL", "123456789B01")
        );

        assert_eq!(
            normalize_vat("12345678", None),
            Err(FormatError::MissingCountry)
        );
        assert_eq!(
            normalize_vat("US123456789", None),
            Err(FormatError::UnsupportedCountry)
        );
        assert_eq!(
            normalize_vat("HU1234567", None),
            Err(FormatError::FormatInvalid)
        );
    }

    #[test]
    fn iso_default_country_maps_to_vies() {
        assert_eq!(ok("123456789", Some("gr")).formatted, "EL123456789");
    }

    #[test]
    fn rejects_bad_characters_and_lengths() {
        assert_eq!(
            normalize_vat("DE12345678901234", None),
            Err(FormatError::FormatInvalid)
        );
        assert_eq!(normalize_vat("DE1", None), Err(FormatError::FormatInvalid));
        assert_eq!(
            normalize_vat("DE12#45", None),
            Err(FormatError::FormatInvalid)
        );
        assert_eq!(normalize_vat("", None), Err(FormatError::MissingCountry));
    }
}

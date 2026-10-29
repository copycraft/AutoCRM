//! VIES hands back the registered address as free text, laid out the way each member state
//! likes ("Stephansplatz 1\nAT-1010 Wien", "1055 BUDAPEST, KOSSUTH TÉR 1.", ...). This
//! splits it into the partner's postal code / city / street fields as a best effort; the
//! UI always shows the original next to the split, and the user decides.

use std::sync::OnceLock;

use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct AddressParts {
    pub postal_code: Option<String>,
    pub city: Option<String>,
    pub address_line: Option<String>,
}

const POSTAL: &str = r"\d{2}-\d{3}|\d{4}\s?[A-Z]{2}|\d{3}\s\d{2}|\d{4,5}";

fn postal_then_city() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Optional country prefix (AT-1010, D-80331), postal code, then the city.
    RE.get_or_init(|| Regex::new(&format!(r"^(?:[A-Z]{{1,2}}-)?({POSTAL})\s+(\D.*)$")).unwrap())
}

fn postal_only() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(&format!(r"^(?:[A-Z]{{1,2}}-)?({POSTAL})$")).unwrap())
}

fn city_then_postal() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(&format!(r"^(\D+?)\s+({POSTAL})$")).unwrap())
}

pub fn split_address(address: &str) -> AddressParts {
    let mut parts: Vec<String> = address
        .split(['\n', '\r'])
        .flat_map(|line| line.split(','))
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return AddressParts::default();
    }

    let mut postal_code = None;
    let mut city = None;
    if let Some((i, caps)) = parts
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, p)| postal_then_city().captures(p).map(|c| (i, c)))
    {
        postal_code = Some(caps[1].to_string());
        city = Some(caps[2].trim().to_string());
        parts.remove(i);
    } else if let Some((i, caps)) = parts
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, p)| city_then_postal().captures(p).map(|c| (i, c)))
    {
        city = Some(caps[1].trim().to_string());
        postal_code = Some(caps[2].to_string());
        parts.remove(i);
    } else if let Some(i) = parts.iter().position(|p| postal_only().is_match(p)) {
        let caps = postal_only().captures(&parts[i]).unwrap();
        postal_code = Some(caps[1].to_string());
        parts.remove(i);
        // The city is then the line without digits.
        if let Some(j) = parts.iter().position(|p| !p.chars().any(|c| c.is_ascii_digit())) {
            city = Some(parts.remove(j));
        }
    } else if parts.len() >= 2
        && parts.last().is_some_and(|p| p.chars().next().is_some_and(char::is_alphabetic))
    {
        // No postal code at all (Irish addresses often end "..., DUBLIN 4"): the last part
        // is the town.
        city = parts.pop();
    }

    AddressParts {
        postal_code,
        city,
        address_line: (!parts.is_empty()).then(|| parts.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(postal: Option<&str>, city: Option<&str>, line: Option<&str>) -> AddressParts {
        AddressParts {
            postal_code: postal.map(Into::into),
            city: city.map(Into::into),
            address_line: line.map(Into::into),
        }
    }

    #[test]
    fn member_state_layouts() {
        assert_eq!(
            split_address("Stephansplatz 1\nAT-1010 Wien"),
            s(Some("1010"), Some("Wien"), Some("Stephansplatz 1"))
        );
        assert_eq!(
            split_address("1055 BUDAPEST, KOSSUTH LAJOS TÉR 1-3. "),
            s(Some("1055"), Some("BUDAPEST"), Some("KOSSUTH LAJOS TÉR 1-3."))
        );
        assert_eq!(
            split_address("KOSSUTH LAJOS TÉR 1-3.\nBUDAPEST\n1055"),
            s(Some("1055"), Some("BUDAPEST"), Some("KOSSUTH LAJOS TÉR 1-3."))
        );
        assert_eq!(
            split_address("VIA ROMA 1 \n00100 ROMA RM\n"),
            s(Some("00100"), Some("ROMA RM"), Some("VIA ROMA 1"))
        );
        assert_eq!(
            split_address("STRAAT 00001\n1234AB AMSTERDAM"),
            s(Some("1234AB"), Some("AMSTERDAM"), Some("STRAAT 00001"))
        );
        assert_eq!(
            split_address("UL. DŁUGA 1, 00-001 WARSZAWA"),
            s(Some("00-001"), Some("WARSZAWA"), Some("UL. DŁUGA 1"))
        );
    }

    #[test]
    fn without_a_postal_code_the_last_part_is_the_town() {
        assert_eq!(
            split_address("3RD FLOOR, GORDON HOUSE, BARROW STREET, DUBLIN 4"),
            s(None, Some("DUBLIN 4"), Some("3RD FLOOR, GORDON HOUSE, BARROW STREET"))
        );
    }

    #[test]
    fn unrecognised_layouts_keep_everything_as_the_street() {
        assert_eq!(split_address("SOMEWHERE"), s(None, None, Some("SOMEWHERE")));
        assert_eq!(split_address("  "), AddressParts::default());
    }
}

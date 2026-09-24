//! Invoicing rules that are decidable without a database or a tax authority.
//!
//! Two things live here. The lifecycle of an invoice, as an enum rather than a set of
//! booleans; and the arithmetic and address handling that turn an order into something NAV
//! will accept. Both are pure, and both are tested below — a rounding rule that disagrees
//! with the one the invoice is printed with is a rejected report, and finding that out from
//! NAV rather than from a unit test is expensive.

use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::domain::money::{Currency, Money, MoneyError};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[sqlx(type_name = "invoice_kind", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum InvoiceKind {
    Invoice,
    Storno,
}

/// Where an invoice is in its life.
///
/// `Submitting` is a real state, not a placeholder: NAV validates asynchronously, so there
/// is a period in which the report has been sent and no verdict exists. `Rejected` is
/// terminal for that document — the number is spent, and a corrected invoice is a new one.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[sqlx(type_name = "invoice_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum InvoiceStatus {
    Submitting,
    Issued,
    Rejected,
    Stornoed,
    Annulled,
}

impl InvoiceStatus {
    /// Whether NAV holds this document as a valid report.
    pub fn is_live(self) -> bool {
        matches!(self, InvoiceStatus::Issued)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            InvoiceStatus::Submitting => "submitting",
            InvoiceStatus::Issued => "issued",
            InvoiceStatus::Rejected => "rejected",
            InvoiceStatus::Stornoed => "stornoed",
            InvoiceStatus::Annulled => "annulled",
        }
    }
}

/// How the customer pays. Cash or bank transfer, nothing else: an invoice is reported
/// with one of these, so anything else is refused at issue time rather than rejected
/// by NAV after drawing a number (Q-INV-14).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "UPPERCASE")]
pub enum PaymentMethod {
    Transfer,
    Cash,
}

impl PaymentMethod {
    /// The wire values, in the order the UI offers them.
    pub const ALL: [&'static str; 2] = ["TRANSFER", "CASH"];

    /// The default when the office does not choose: a bank transfer.
    pub const DEFAULT: PaymentMethod = PaymentMethod::Transfer;

    pub fn as_str(self) -> &'static str {
        match self {
            PaymentMethod::Transfer => "TRANSFER",
            PaymentMethod::Cash => "CASH",
        }
    }

    /// What the customer reads on the letter.
    pub fn hu_label(self) -> &'static str {
        match self {
            PaymentMethod::Transfer => "Átutalás",
            PaymentMethod::Cash => "Készpénz",
        }
    }

    /// Case-insensitive: `"cash"` and `" Cash "` are what they mean.
    pub fn parse(raw: &str) -> Result<PaymentMethod, UnknownPaymentMethod> {
        match raw.trim().to_uppercase().as_str() {
            "TRANSFER" => Ok(PaymentMethod::Transfer),
            "CASH" => Ok(PaymentMethod::Cash),
            _ => Err(UnknownPaymentMethod(raw.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("payment method must be TRANSFER or CASH, not '{0}'")]
pub struct UnknownPaymentMethod(pub String);

/// A NAV-shaped address: the parts, not a line of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub postal_code: String,
    pub city: String,
    pub street_name: String,
    pub public_place_category: String,
    pub number: String,
}

/// Public-place categories NAV sees on Hungarian invoices. Longest first, so "körút" wins
/// over "út" in "Nagykörút" — the whole point of scanning for a category is to split the
/// name from it, and matching the shorter one would cut the name in half.
const PLACE_CATEGORIES: &[&str] = &[
    "sugárút",
    "körút",
    "rakpart",
    "sétány",
    "fasor",
    "lejtő",
    "lépcső",
    "dűlő",
    "liget",
    "köz",
    "sor",
    "park",
    "part",
    "udvar",
    "tér",
    "utca",
    "út",
    "u.",
    "krt.",
];

/// Splits "Kossuth Lajos utca 12." into its name, category and number.
///
/// Partners carry one free-text `address_line`, because that is what an address is on a
/// letter. NAV wants it in three pieces. This finds the category word and cuts there,
/// which handles the forms that actually occur ("Fő út 5/A", "Váci utca 12.", "Hősök tere
/// 3" — the last via the `tere`/`útja` inflections). Anything it cannot read is refused
/// rather than guessed at: an invented street category is a defective invoice.
pub fn split_address_line(line: &str) -> Option<(String, String, String)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let lower = line.to_lowercase();

    // Inflected forms that appear in real addresses: "Hősök tere", "Bartók Béla útja".
    let normalise = |category: &str| -> &'static str {
        match category {
            "tere" => "tér",
            "útja" => "út",
            "u." => "utca",
            "krt." => "körút",
            other => match PLACE_CATEGORIES.iter().find(|c| **c == other) {
                Some(c) => c,
                None => "utca",
            },
        }
    };

    let candidates: Vec<&str> = PLACE_CATEGORIES
        .iter()
        .copied()
        .chain(["tere", "útja"])
        .collect();

    // The last category word wins: "Ipar utca" inside a city name would otherwise cut too
    // early, and the street's own category is always the last one before the number.
    let mut best: Option<(usize, usize, &str)> = None;
    for category in candidates {
        let mut from = 0;
        while let Some(found) = lower[from..].find(category) {
            let start = from + found;
            let end = start + category.len();
            let before_ok = start == 0 || lower[..start].ends_with(' ');
            let after = &lower[end..];
            let after_ok = after.is_empty() || after.starts_with(' ');
            if before_ok && after_ok && start > 0 && best.is_none_or(|(s, _, _)| start > s) {
                best = Some((start, end, category));
            }
            from = end.max(start + 1);
            if from >= lower.len() {
                break;
            }
        }
    }

    let (start, end, category) = best?;
    let street_name = line[..start].trim().trim_end_matches(',').trim().to_string();
    let number = line[end..]
        .trim()
        .trim_start_matches(',')
        .trim()
        .to_string();
    if street_name.is_empty() || number.is_empty() {
        return None;
    }
    Some((street_name, normalise(category).to_string(), number))
}

/// Minor units as the decimal string the sidecar takes: 139_700 → "1397.00".
///
/// A string rather than a JSON number on purpose. Money that has travelled through a
/// double is money you cannot reconcile, and every amount on an invoice is reconciled.
pub fn minor_to_decimal_string(minor: i64, currency: Currency) -> String {
    let scale = currency.exponent();
    Decimal::new(minor, scale).to_string()
}

/// Net and VAT for one line, in minor units.
///
/// Rounds half away from zero at each step, matching `Money::times_quantity` and Postgres
/// `round()` — the same rule the reporting views use, so an invoice total and an order
/// total never differ by a fillér for reasons nobody can explain.
pub fn line_amounts(
    unit_price: Money,
    quantity: Decimal,
    vat_rate: Decimal,
) -> Result<(Money, Money), MoneyError> {
    let net = unit_price.times_quantity(quantity)?;
    let vat_exact = Decimal::from(net.minor()) * vat_rate;
    let rounded = vat_exact.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
    let vat_minor = i64::try_from(rounded).map_err(|_| MoneyError::Overflow)?;
    Ok((net, Money::new(vat_minor, net.currency())))
}

/// `AT2026-0001`. The year is in the number because an invoice series runs by year, and a
/// human reading a number should be able to say when it was issued.
pub fn format_invoice_number(prefix: &str, year: i32, sequence: i32) -> String {
    format!("{prefix}{year}-{sequence:04}")
}

/// A VAT rate the sidecar will accept: a fraction, never a percentage.
pub fn validate_vat_rate(rate: Decimal) -> Result<Decimal, String> {
    if rate.is_sign_negative() || rate > Decimal::ONE {
        return Err(format!(
            "VAT rate is a fraction between 0 and 1 (0.27 for 27%), got {rate}"
        ));
    }
    Ok(rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    #[test]
    fn splits_the_addresses_that_actually_occur() {
        assert_eq!(
            split_address_line("Kossuth Lajos utca 12."),
            Some(("Kossuth Lajos".into(), "utca".into(), "12.".into()))
        );
        assert_eq!(
            split_address_line("Fő út 5/A"),
            Some(("Fő".into(), "út".into(), "5/A".into()))
        );
        assert_eq!(
            split_address_line("Hősök tere 3"),
            Some(("Hősök".into(), "tér".into(), "3".into()))
        );
        assert_eq!(
            split_address_line("Bartók Béla útja 17/b"),
            Some(("Bartók Béla".into(), "út".into(), "17/b".into()))
        );
        assert_eq!(
            split_address_line("Váci u. 8"),
            Some(("Váci".into(), "utca".into(), "8".into()))
        );
    }

    #[test]
    fn the_last_category_word_wins() {
        // "Ipar" is not a category, but "Ipar utca" inside a longer name must not cut at
        // the first thing that looks like one.
        assert_eq!(
            split_address_line("Régi Ipar utca 4"),
            Some(("Régi Ipar".into(), "utca".into(), "4".into()))
        );
    }

    #[test]
    fn refuses_what_it_cannot_read_rather_than_guessing() {
        assert_eq!(split_address_line(""), None);
        assert_eq!(split_address_line("utca 12"), None); // no street name
        assert_eq!(split_address_line("Kossuth utca"), None); // no number
        assert_eq!(split_address_line("hrsz 0123/4"), None); // topographical lot number
    }

    #[test]
    fn line_amounts_round_half_away_from_zero() {
        let price = Money::new(100_000, Currency::HUF); // 1000.00
        let (net, vat) = line_amounts(price, dec("1"), dec("0.27")).unwrap();
        assert_eq!(net.minor(), 100_000);
        assert_eq!(vat.minor(), 27_000);

        // 2.5 × 4000.00 = 10000.00 net, 2700.00 VAT
        let (net, vat) = line_amounts(Money::new(400_000, Currency::HUF), dec("2.5"), dec("0.27"))
            .unwrap();
        assert_eq!(net.minor(), 1_000_000);
        assert_eq!(vat.minor(), 270_000);

        // A discount line stays negative in both.
        let (net, vat) =
            line_amounts(Money::new(-1_000, Currency::HUF), dec("1"), dec("0.27")).unwrap();
        assert_eq!(net.minor(), -1_000);
        assert_eq!(vat.minor(), -270);
    }

    #[test]
    fn zero_rate_is_allowed_and_a_percentage_is_not() {
        assert!(validate_vat_rate(dec("0")).is_ok());
        assert!(validate_vat_rate(dec("0.27")).is_ok());
        assert!(validate_vat_rate(dec("27")).is_err());
        assert!(validate_vat_rate(dec("-0.1")).is_err());
    }

    #[test]
    fn minor_units_render_with_the_currency_exponent() {
        assert_eq!(
            minor_to_decimal_string(139_700, Currency::HUF),
            "1397.00".to_string()
        );
        assert_eq!(
            minor_to_decimal_string(-139_700, Currency::EUR),
            "-1397.00".to_string()
        );
    }

    #[test]
    fn invoice_numbers_are_padded() {
        assert_eq!(format_invoice_number("AT", 2026, 1), "AT2026-0001");
        assert_eq!(format_invoice_number("DB", 2026, 1234), "DB2026-1234");
    }

    #[test]
    fn payment_methods_parse_leniently_and_render_canonically() {
        assert_eq!(PaymentMethod::parse("CASH"), Ok(PaymentMethod::Cash));
        assert_eq!(PaymentMethod::parse(" cash "), Ok(PaymentMethod::Cash));
        assert_eq!(PaymentMethod::parse("transfer"), Ok(PaymentMethod::Transfer));
        assert_eq!(PaymentMethod::Cash.as_str(), "CASH");
        assert_eq!(PaymentMethod::Cash.hu_label(), "Készpénz");
        assert_eq!(PaymentMethod::Transfer.hu_label(), "Átutalás");
        assert_eq!(PaymentMethod::DEFAULT, PaymentMethod::Transfer);
        assert!(PaymentMethod::parse("CHEQUE").is_err());
        assert!(PaymentMethod::parse("").is_err());
    }
}

//! Order-level rules that don't need a database.

use rust_decimal::Decimal;

use super::money::{Currency, Money, MoneyError};

/// New orders are numbered `YYYY-NNNN` per calendar year. Imported orders keep whatever
/// number they had; only numbers in this shape participate in the sequence.
pub fn format_order_number(year: i32, sequence: i32) -> String {
    format!("{year}-{sequence:04}")
}

pub fn parse_order_number(number: &str) -> Option<(i32, i32)> {
    let (year, seq) = number.split_once('-')?;
    if year.len() != 4
        || seq.len() < 4
        || !year.chars().chain(seq.chars()).all(|c| c.is_ascii_digit())
    {
        return None;
    }
    Some((year.parse().ok()?, seq.parse().ok()?))
}

/// Plates are matched ignoring spaces, dashes and case: "abc-123" finds "ABC 123".
/// Must stay in sync with the `orders_plate_idx` expression in 0003_orders.sql.
pub fn normalize_plate(plate: &str) -> String {
    plate
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase()
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LineItemError {
    #[error("description is required")]
    EmptyDescription,
    #[error("quantity must be positive with at most 3 decimal places")]
    InvalidQuantity,
    #[error("line item currency {item} does not match order currency {order}")]
    CurrencyMismatch { item: Currency, order: Currency },
    #[error(transparent)]
    Money(#[from] MoneyError),
}

pub fn validate_line_item(
    description: &str,
    quantity: Decimal,
    unit_price: Money,
    order_currency: Currency,
) -> Result<Money, LineItemError> {
    if description.trim().is_empty() {
        return Err(LineItemError::EmptyDescription);
    }
    if quantity <= Decimal::ZERO || quantity.scale() > 3 || quantity >= Decimal::from(1_000_000_000)
    {
        return Err(LineItemError::InvalidQuantity);
    }
    if unit_price.currency() != order_currency {
        return Err(LineItemError::CurrencyMismatch {
            item: unit_price.currency(),
            order: order_currency,
        });
    }
    Ok(unit_price.times_quantity(quantity)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_numbers_round_trip() {
        assert_eq!(format_order_number(2026, 42), "2026-0042");
        assert_eq!(parse_order_number("2026-0042"), Some((2026, 42)));
        assert_eq!(parse_order_number("2026-12345"), Some((2026, 12345)));
        assert_eq!(parse_order_number("MC-2019/17"), None);
        assert_eq!(parse_order_number("2026-42"), None);
    }

    #[test]
    fn plates_normalise() {
        assert_eq!(normalize_plate("abc-123"), "ABC123");
        assert_eq!(normalize_plate("AA BB-123"), "AABB123");
    }

    #[test]
    fn line_item_validation() {
        let price = Money::new(10_000, Currency::EUR);
        let q: Decimal = "2.5".parse().unwrap();
        assert_eq!(
            validate_line_item("Hűtőgép", q, price, Currency::EUR),
            Ok(Money::new(25_000, Currency::EUR))
        );
        assert_eq!(
            validate_line_item(" ", q, price, Currency::EUR),
            Err(LineItemError::EmptyDescription)
        );
        assert_eq!(
            validate_line_item("x", "0.0001".parse().unwrap(), price, Currency::EUR),
            Err(LineItemError::InvalidQuantity)
        );
        assert_eq!(
            validate_line_item("x", Decimal::ZERO, price, Currency::EUR),
            Err(LineItemError::InvalidQuantity)
        );
        assert!(matches!(
            validate_line_item("x", q, price, Currency::HUF),
            Err(LineItemError::CurrencyMismatch { .. })
        ));
    }
}

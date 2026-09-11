//! Currency-safe money. Integer minor units, explicit currency, no floats anywhere.
//!
//! There is deliberately no `impl Add for Money`: every combination goes through a
//! checked method that refuses to mix currencies, so "added EUR to HUF" is an error
//! you must handle rather than a number you silently get.

use std::fmt;
use std::str::FromStr;

use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Currency {
    HUF,
    EUR,
}

impl Currency {
    pub const ALL: [Currency; 2] = [Currency::HUF, Currency::EUR];

    pub fn code(self) -> &'static str {
        match self {
            Currency::HUF => "HUF",
            Currency::EUR => "EUR",
        }
    }

    /// ISO 4217 minor-unit exponent. Both supported currencies use 2, which is what
    /// lets FX conversion multiply minor units by the published rate directly.
    pub fn exponent(self) -> u32 {
        2
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unsupported currency '{0}'")]
pub struct UnknownCurrency(pub String);

impl FromStr for Currency {
    type Err = UnknownCurrency;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "HUF" => Ok(Currency::HUF),
            "EUR" => Ok(Currency::EUR),
            other => Err(UnknownCurrency(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MoneyError {
    #[error("cannot combine {0} with {1}")]
    CurrencyMismatch(Currency, Currency),
    #[error("amount out of range")]
    Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Money {
    minor: i64,
    currency: Currency,
}

impl Money {
    pub const fn new(minor: i64, currency: Currency) -> Self {
        Money { minor, currency }
    }

    pub const fn zero(currency: Currency) -> Self {
        Money { minor: 0, currency }
    }

    pub fn minor(self) -> i64 {
        self.minor
    }

    pub fn currency(self) -> Currency {
        self.currency
    }

    pub fn checked_add(self, other: Money) -> Result<Money, MoneyError> {
        self.same_currency(other)?;
        let minor = self
            .minor
            .checked_add(other.minor)
            .ok_or(MoneyError::Overflow)?;
        Ok(Money::new(minor, self.currency))
    }

    pub fn checked_sub(self, other: Money) -> Result<Money, MoneyError> {
        self.same_currency(other)?;
        let minor = self
            .minor
            .checked_sub(other.minor)
            .ok_or(MoneyError::Overflow)?;
        Ok(Money::new(minor, self.currency))
    }

    /// Sums amounts that must all be in `currency`. An empty iterator is zero.
    pub fn sum(
        currency: Currency,
        amounts: impl IntoIterator<Item = Money>,
    ) -> Result<Money, MoneyError> {
        amounts
            .into_iter()
            .try_fold(Money::zero(currency), |acc, m| acc.checked_add(m))
    }

    /// Line total: this unit price × quantity, rounded half away from zero to whole
    /// minor units. Matches Postgres `round(numeric)`, which the reporting views use.
    pub fn times_quantity(self, quantity: Decimal) -> Result<Money, MoneyError> {
        let exact = Decimal::from(self.minor)
            .checked_mul(quantity)
            .ok_or(MoneyError::Overflow)?;
        let rounded = exact.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
        let minor = i64::try_from(rounded).map_err(|_| MoneyError::Overflow)?;
        Ok(Money::new(minor, self.currency))
    }

    /// Converts at `rate` = units of `target` per one unit of this amount's currency,
    /// rounded half away from zero. Converting to the same currency ignores the rate.
    pub fn convert(self, target: Currency, rate: Decimal) -> Result<Money, MoneyError> {
        if target == self.currency {
            return Ok(self);
        }
        debug_assert_eq!(self.currency.exponent(), target.exponent());
        let exact = Decimal::from(self.minor)
            .checked_mul(rate)
            .ok_or(MoneyError::Overflow)?;
        let rounded = exact.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
        let minor = i64::try_from(rounded).map_err(|_| MoneyError::Overflow)?;
        Ok(Money::new(minor, target))
    }

    fn same_currency(self, other: Money) -> Result<(), MoneyError> {
        if self.currency == other.currency {
            Ok(())
        } else {
            Err(MoneyError::CurrencyMismatch(self.currency, other.currency))
        }
    }
}

impl fmt::Display for Money {
    /// Plain machine-ish rendering for logs: `-1234.56 EUR`. Locale formatting is the UI's job.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scale = 10i64.pow(self.currency.exponent());
        let sign = if self.minor < 0 { "-" } else { "" };
        let abs = self.minor.unsigned_abs();
        write!(
            f,
            "{sign}{}.{:0width$} {}",
            abs / scale as u64,
            abs % scale as u64,
            self.currency,
            width = self.currency.exponent() as usize
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    const EUR_10: Money = Money::new(1000, Currency::EUR);
    const HUF_10: Money = Money::new(1000, Currency::HUF);

    #[test]
    fn adding_same_currency_works() {
        assert_eq!(
            EUR_10.checked_add(EUR_10),
            Ok(Money::new(2000, Currency::EUR))
        );
        assert_eq!(
            EUR_10.checked_sub(Money::new(1500, Currency::EUR)),
            Ok(Money::new(-500, Currency::EUR))
        );
    }

    #[test]
    fn mixing_currencies_is_an_error() {
        assert_eq!(
            EUR_10.checked_add(HUF_10),
            Err(MoneyError::CurrencyMismatch(Currency::EUR, Currency::HUF))
        );
        assert!(Money::sum(Currency::EUR, [EUR_10, HUF_10]).is_err());
    }

    #[test]
    fn overflow_is_an_error() {
        let max = Money::new(i64::MAX, Currency::HUF);
        assert_eq!(
            max.checked_add(Money::new(1, Currency::HUF)),
            Err(MoneyError::Overflow)
        );
    }

    #[test]
    fn empty_sum_is_zero() {
        assert_eq!(
            Money::sum(Currency::HUF, []),
            Ok(Money::zero(Currency::HUF))
        );
    }

    #[test]
    fn line_totals_round_half_away_from_zero() {
        // 0.5 minor units rounds up, -0.5 rounds down — same as Postgres round().
        assert_eq!(
            Money::new(1, Currency::EUR)
                .times_quantity(dec("0.5"))
                .unwrap()
                .minor(),
            1
        );
        assert_eq!(
            Money::new(-1, Currency::EUR)
                .times_quantity(dec("0.5"))
                .unwrap()
                .minor(),
            -1
        );
        assert_eq!(
            Money::new(333, Currency::EUR)
                .times_quantity(dec("1.5"))
                .unwrap()
                .minor(),
            500
        ); // 499.5
        assert_eq!(
            Money::new(1999, Currency::HUF)
                .times_quantity(dec("2.125"))
                .unwrap()
                .minor(),
            4248
        ); // 4247.875
    }

    #[test]
    fn conversion_uses_rate_and_rounds() {
        // 10.00 EUR at 365.22 HUF/EUR = 3652.20 HUF
        let huf = EUR_10.convert(Currency::HUF, dec("365.22")).unwrap();
        assert_eq!(huf, Money::new(365_220, Currency::HUF));
        // 0.01 EUR at 365.225 = 365.225 fillér → 365; at 365.5 = 365.5 fillér → 366
        assert_eq!(
            Money::new(1, Currency::EUR)
                .convert(Currency::HUF, dec("365.225"))
                .unwrap()
                .minor(),
            365
        );
        assert_eq!(
            Money::new(1, Currency::EUR)
                .convert(Currency::HUF, dec("365.5"))
                .unwrap()
                .minor(),
            366
        );
        assert_eq!(HUF_10.convert(Currency::HUF, dec("999")).unwrap(), HUF_10);
    }

    #[test]
    fn display_is_unambiguous() {
        assert_eq!(Money::new(123456, Currency::EUR).to_string(), "1234.56 EUR");
        assert_eq!(Money::new(-5, Currency::HUF).to_string(), "-0.05 HUF");
    }

    #[test]
    fn currency_parsing() {
        assert_eq!("EUR".parse::<Currency>(), Ok(Currency::EUR));
        assert!("USD".parse::<Currency>().is_err());
    }
}

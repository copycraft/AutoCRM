//! Leave and absence rules: the kinds, and counting working days.
//!
//! A leave day is a working day. Weekends and Hungarian public holidays inside a period do
//! not use up the allowance. The holidays are the fixed ones plus those that follow Easter.
//! The government's yearly "bridge day" decrees (a swapped Saturday working day, say) are
//! not in any formula and are not handled: HR adjusts those by hand.

use chrono::{Datelike, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum LeaveKind {
    /// Paid annual leave. The only kind that counts against the allowance.
    Annual,
    Sick,
    Unpaid,
    Other,
}

impl LeaveKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "annual" => Some(LeaveKind::Annual),
            "sick" => Some(LeaveKind::Sick),
            "unpaid" => Some(LeaveKind::Unpaid),
            "other" => Some(LeaveKind::Other),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            LeaveKind::Annual => "annual",
            LeaveKind::Sick => "sick",
            LeaveKind::Unpaid => "unpaid",
            LeaveKind::Other => "other",
        }
    }
}

/// Easter Sunday (Gregorian), by the anonymous algorithm.
pub fn easter(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = (h + l - 7 * m + 114) % 31 + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32).expect("a real date")
}

/// Public holidays of `year`.
pub fn holidays(year: i32) -> Vec<NaiveDate> {
    let fixed = [
        (1, 1),   // New Year
        (3, 15),  // 1848 revolution
        (5, 1),   // Labour Day
        (8, 20),  // State foundation
        (10, 23), // 1956 revolution
        (11, 1),  // All Saints
        (12, 25),
        (12, 26),
    ];
    let easter = easter(year);
    let mut days: Vec<NaiveDate> = fixed
        .iter()
        .map(|&(m, d)| NaiveDate::from_ymd_opt(year, m, d).expect("a real date"))
        .collect();
    days.push(easter - chrono::TimeDelta::days(2)); // Good Friday
    days.push(easter + chrono::TimeDelta::days(1)); // Easter Monday
    days.push(easter + chrono::TimeDelta::days(50)); // Whit Monday
    days
}

fn is_working_day(date: NaiveDate, holidays: &[NaiveDate]) -> bool {
    !matches!(date.weekday(), Weekday::Sat | Weekday::Sun) && !holidays.contains(&date)
}

/// Working days in `from..=to` that fall inside calendar `year` (the whole period when
/// `year` is None).
pub fn working_days(from: NaiveDate, to: NaiveDate, year: Option<i32>) -> i32 {
    if to < from {
        return 0;
    }
    let mut holiday_cache: Vec<(i32, Vec<NaiveDate>)> = Vec::new();
    let mut count = 0;
    let mut date = from;
    while date <= to {
        if year.is_none_or(|y| y == date.year()) {
            if !holiday_cache.iter().any(|(y, _)| *y == date.year()) {
                holiday_cache.push((date.year(), holidays(date.year())));
            }
            let h = &holiday_cache
                .iter()
                .find(|(y, _)| *y == date.year())
                .unwrap()
                .1;
            if is_working_day(date, h) {
                count += 1;
            }
        }
        date = date.succ_opt().expect("date within range");
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn easter_dates_are_right() {
        assert_eq!(easter(2024), d(2024, 3, 31));
        assert_eq!(easter(2025), d(2025, 4, 20));
        assert_eq!(easter(2026), d(2026, 4, 5));
        assert_eq!(easter(2027), d(2027, 3, 28));
    }

    #[test]
    fn holidays_include_the_easter_ones() {
        let h = holidays(2026);
        assert!(h.contains(&d(2026, 4, 3))); // Good Friday
        assert!(h.contains(&d(2026, 4, 6))); // Easter Monday
        assert!(h.contains(&d(2026, 5, 25))); // Whit Monday
        assert!(h.contains(&d(2026, 10, 23)));
        assert_eq!(h.len(), 11);
    }

    #[test]
    fn weekends_and_holidays_do_not_use_leave() {
        // Mon 2026-03-30 to Fri 2026-04-10: ten weekdays, minus Good Friday and Easter Monday.
        assert_eq!(working_days(d(2026, 3, 30), d(2026, 4, 10), None), 8);
        // A single Saturday is nothing.
        assert_eq!(working_days(d(2026, 6, 6), d(2026, 6, 6), None), 0);
        // One ordinary day is one.
        assert_eq!(working_days(d(2026, 6, 8), d(2026, 6, 8), None), 1);
        assert_eq!(working_days(d(2026, 6, 9), d(2026, 6, 8), None), 0);
    }

    #[test]
    fn a_period_over_new_year_is_split_by_year() {
        // Wed 2025-12-24 .. Fri 2026-01-02. 2025: 24, 29, 30, 31 are working days (25-26 are
        // holidays, 27-28 a weekend); 2026: Jan 1 is a holiday, Jan 2 is a working day.
        let (from, to) = (d(2025, 12, 24), d(2026, 1, 2));
        assert_eq!(working_days(from, to, Some(2025)), 4);
        assert_eq!(working_days(from, to, Some(2026)), 1);
        assert_eq!(working_days(from, to, None), 5);
    }
}

//! Use cases: orchestration of domain rules, repositories and integrations.

pub mod auth;
pub mod automation;
pub mod email;
pub mod invoicing;
pub mod leads;
pub mod media;
pub mod orders;
pub mod stages;

use chrono::{NaiveDate, Utc};
use chrono_tz::Tz;

/// "Today" as the business experiences it (Budapest), not as UTC does.
pub fn business_today(tz: Tz) -> NaiveDate {
    Utc::now().with_timezone(&tz).date_naive()
}

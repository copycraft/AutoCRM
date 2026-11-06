//! Use cases: orchestration of domain rules, repositories and integrations.

pub mod ads;
pub mod assistant;
pub mod auth;
pub mod automation;
pub mod email;
pub mod followups;
pub mod invoicing;
pub mod leads;
pub mod mailbox;
pub mod media;
pub mod modules;
pub mod newsletter;
pub mod notifications;
pub mod orders;
pub mod questionnaire;
pub mod relocate;
pub mod reminders;
pub mod repeat_order;
pub mod secrets;
pub mod stages;
pub mod vat;
pub mod weekly_report;

use chrono::{NaiveDate, Utc};
use chrono_tz::Tz;

/// "Today" as the business experiences it (Budapest), not as UTC does.
pub fn business_today(tz: Tz) -> NaiveDate {
    Utc::now().with_timezone(&tz).date_naive()
}

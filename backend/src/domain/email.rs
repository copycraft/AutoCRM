//! Rules deciding *whether* and *when* an email may go out. The sending itself lives in
//! integrations/email; this is pure policy.

use chrono::{DateTime, Datelike, NaiveDateTime, NaiveTime, TimeDelta, TimeZone, Utc, Weekday};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[sqlx(type_name = "email_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum EmailStatus {
    Queued,
    Sending,
    Sent,
    Failed,
    Cancelled,
    /// A send was interrupted after contacting the provider. It may or may not have been
    /// delivered, so it is never retried automatically — a human checks and decides.
    NeedsReview,
}

/// Business-hours window for automatic mail, in the business time zone.
#[derive(Debug, Clone, Copy)]
pub struct SendWindow {
    pub start: NaiveTime,
    pub end: NaiveTime,
    pub weekdays_only: bool,
    pub tz: Tz,
}

impl SendWindow {
    /// `now` if inside the window, otherwise the next moment the window opens.
    pub fn next_opening(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        if self.start >= self.end {
            return now; // misconfigured window: settings validation prevents this
        }
        let mut date = now.with_timezone(&self.tz).date_naive();
        for _ in 0..8 {
            let weekend = matches!(date.weekday(), Weekday::Sat | Weekday::Sun);
            if !(self.weekdays_only && weekend) {
                let open = self.resolve_local(date.and_time(self.start));
                let close = self.resolve_local(date.and_time(self.end));
                if now < open {
                    return open;
                }
                if now < close {
                    return now;
                }
            }
            date = date.succ_opt().expect("date within chrono range");
        }
        now
    }

    fn resolve_local(&self, local: NaiveDateTime) -> DateTime<Utc> {
        // DST gap (the skipped hour) has no local mapping: shift forward an hour.
        self.tz
            .from_local_datetime(&local)
            .earliest()
            .or_else(|| {
                self.tz
                    .from_local_datetime(&(local + TimeDelta::hours(1)))
                    .earliest()
            })
            .expect("local time resolvable after DST shift")
            .with_timezone(&Utc)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AutoSendContext {
    /// The global kill switch (settings.automatic_email_enabled).
    pub automatic_enabled: bool,
    pub recipient_suppressed: bool,
    /// Automatic mails already sent to this recipient in the last 24 hours.
    pub sent_to_recipient_last_24h: i64,
    pub max_per_recipient_day: i32,
    pub has_unresolved_variables: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoSendDecision {
    Send,
    Defer(DateTime<Utc>),
    Cancel(&'static str),
    Fail(&'static str),
}

/// Safety rails for automatic mail, in order: kill switch, suppression, broken template,
/// send window, per-recipient rate limit. Manual mail skips this entirely.
pub fn decide_automatic(
    ctx: AutoSendContext,
    window: &SendWindow,
    now: DateTime<Utc>,
) -> AutoSendDecision {
    if !ctx.automatic_enabled {
        return AutoSendDecision::Cancel("automatic email is switched off");
    }
    if ctx.recipient_suppressed {
        return AutoSendDecision::Cancel("recipient is on the suppression list");
    }
    if ctx.has_unresolved_variables {
        return AutoSendDecision::Fail("template has unresolved variables");
    }
    let opening = window.next_opening(now);
    if opening > now {
        return AutoSendDecision::Defer(opening);
    }
    if ctx.sent_to_recipient_last_24h >= i64::from(ctx.max_per_recipient_day) {
        return AutoSendDecision::Cancel("recipient rate limit reached");
    }
    AutoSendDecision::Send
}

/// Minimal sanity check + normalisation for stored addresses. Full RFC parsing happens in
/// the mail library at send time; this rejects the obviously wrong and anything that could
/// smuggle extra headers or recipients.
pub fn normalize_address(raw: &str) -> Option<String> {
    let addr = raw.trim().to_lowercase();
    if addr.len() > 254
        || addr.chars().any(|c| {
            c.is_whitespace() || c.is_control() || matches!(c, ',' | ';' | '<' | '>' | '"')
        })
    {
        return None;
    }
    let (local, domain) = addr.split_once('@')?;
    if local.is_empty()
        || domain.len() < 3
        || !domain.contains('.')
        || domain.contains('@')
        || domain.starts_with('.')
        || domain.ends_with('.')
    {
        return None;
    }
    Some(addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> SendWindow {
        SendWindow {
            start: NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
            end: NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
            weekdays_only: true,
            tz: chrono_tz::Europe::Budapest,
        }
    }

    fn budapest(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        chrono_tz::Europe::Budapest
            .with_ymd_and_hms(y, m, d, h, min, 0)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn ok_ctx() -> AutoSendContext {
        AutoSendContext {
            automatic_enabled: true,
            recipient_suppressed: false,
            sent_to_recipient_last_24h: 0,
            max_per_recipient_day: 3,
            has_unresolved_variables: false,
        }
    }

    #[test]
    fn inside_window_sends_now() {
        let now = budapest(2026, 9, 10, 10, 30); // Thursday
        assert_eq!(window().next_opening(now), now);
        assert_eq!(
            decide_automatic(ok_ctx(), &window(), now),
            AutoSendDecision::Send
        );
    }

    #[test]
    fn night_defers_to_morning() {
        let now = budapest(2026, 9, 10, 3, 0);
        assert_eq!(window().next_opening(now), budapest(2026, 9, 10, 8, 0));
        let evening = budapest(2026, 9, 10, 17, 0);
        assert_eq!(window().next_opening(evening), budapest(2026, 9, 11, 8, 0));
    }

    #[test]
    fn weekend_defers_to_monday() {
        let saturday = budapest(2026, 9, 12, 11, 0);
        assert_eq!(window().next_opening(saturday), budapest(2026, 9, 14, 8, 0));
        let friday_evening = budapest(2026, 9, 11, 18, 0);
        assert_eq!(
            window().next_opening(friday_evening),
            budapest(2026, 9, 14, 8, 0)
        );
    }

    #[test]
    fn window_respects_dst() {
        // Budapest leaves DST on 2026-10-25; 08:00 local is 07:00 UTC after that.
        let monday_night = budapest(2026, 10, 26, 2, 0);
        assert_eq!(
            window().next_opening(monday_night).to_rfc3339(),
            "2026-10-26T07:00:00+00:00"
        );
    }

    #[test]
    fn safety_rails_in_order() {
        let now = budapest(2026, 9, 10, 10, 0);
        let w = window();
        assert!(matches!(
            decide_automatic(
                AutoSendContext {
                    automatic_enabled: false,
                    ..ok_ctx()
                },
                &w,
                now
            ),
            AutoSendDecision::Cancel(_)
        ));
        assert!(matches!(
            decide_automatic(
                AutoSendContext {
                    recipient_suppressed: true,
                    ..ok_ctx()
                },
                &w,
                now
            ),
            AutoSendDecision::Cancel(_)
        ));
        assert!(matches!(
            decide_automatic(
                AutoSendContext {
                    has_unresolved_variables: true,
                    ..ok_ctx()
                },
                &w,
                now
            ),
            AutoSendDecision::Fail(_)
        ));
        assert!(matches!(
            decide_automatic(
                AutoSendContext {
                    sent_to_recipient_last_24h: 3,
                    ..ok_ctx()
                },
                &w,
                now
            ),
            AutoSendDecision::Cancel(_)
        ));
        let night = budapest(2026, 9, 10, 23, 0);
        assert!(matches!(
            decide_automatic(ok_ctx(), &w, night),
            AutoSendDecision::Defer(_)
        ));
    }

    #[test]
    fn address_normalisation() {
        assert_eq!(
            normalize_address(" Michael@Mueller-GmbH.de ").as_deref(),
            Some("michael@mueller-gmbh.de")
        );
        assert_eq!(normalize_address("a@b"), None);
        assert_eq!(normalize_address("a@b.c, evil@x.y"), None);
        assert_eq!(normalize_address("a\r\nbcc:x@y.z"), None);
        assert_eq!(normalize_address("noatsign.hu"), None);
    }
}

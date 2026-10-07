//! iCalendar (RFC 5545) for the read-only calendar feed: all-day events only, which is all
//! the CRM has (due dates, leave). Pure text building; the caller supplies the events.

use chrono::{DateTime, NaiveDate, Utc};

pub struct Event {
    /// Stable across feeds, so a calendar app updates the event rather than duplicating it.
    pub uid: String,
    pub start: NaiveDate,
    /// The last day, inclusive (iCalendar's DTEND is exclusive; we add the day).
    pub last_day: NaiveDate,
    pub summary: String,
    pub description: Option<String>,
    pub url: Option<String>,
}

/// TEXT escaping: backslash, semicolon, comma, newline.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

/// Lines longer than 75 octets continue on the next line after CRLF + space, split on a
/// character boundary.
fn fold(line: &str, out: &mut String) {
    let mut width = 0;
    for c in line.chars() {
        let len = c.len_utf8();
        if width + len > 75 {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(c);
        width += len;
    }
    out.push_str("\r\n");
}

fn day(d: NaiveDate) -> String {
    d.format("%Y%m%d").to_string()
}

pub fn calendar(name: &str, domain: &str, now: DateTime<Utc>, events: &[Event]) -> String {
    let mut out = String::new();
    let stamp = now.format("%Y%m%dT%H%M%SZ").to_string();
    for line in [
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//Autotherm//AutoCRM//HU".to_string(),
        "CALSCALE:GREGORIAN".to_string(),
        "METHOD:PUBLISH".to_string(),
        format!("X-WR-CALNAME:{}", escape(name)),
        "X-WR-TIMEZONE:Europe/Budapest".to_string(),
        // Ask clients to refresh hourly; most poll less often regardless.
        "REFRESH-INTERVAL;VALUE=DURATION:PT1H".to_string(),
        "X-PUBLISHED-TTL:PT1H".to_string(),
    ] {
        fold(&line, &mut out);
    }
    for e in events {
        let end = e.last_day.max(e.start).succ_opt().unwrap_or(e.last_day);
        fold("BEGIN:VEVENT", &mut out);
        fold(&format!("UID:{}@{}", e.uid, domain), &mut out);
        fold(&format!("DTSTAMP:{stamp}"), &mut out);
        fold(&format!("DTSTART;VALUE=DATE:{}", day(e.start)), &mut out);
        fold(&format!("DTEND;VALUE=DATE:{}", day(end)), &mut out);
        fold(&format!("SUMMARY:{}", escape(&e.summary)), &mut out);
        if let Some(d) = &e.description {
            fold(&format!("DESCRIPTION:{}", escape(d)), &mut out);
        }
        if let Some(u) = &e.url {
            fold(&format!("URL:{u}"), &mut out);
        }
        fold("TRANSP:TRANSPARENT", &mut out);
        fold("END:VEVENT", &mut out);
    }
    fold("END:VCALENDAR", &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_all_day_event_ends_the_day_after() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
        let ics = calendar(
            "Teszt",
            "autotherm.test",
            "2026-10-01T08:00:00Z".parse().unwrap(),
            &[Event {
                uid: "task-1".into(),
                start: d,
                last_day: d,
                summary: "Feladat: hívás, árajánlat; sürgős".into(),
                description: None,
                url: Some("https://crm.example/hu/orders/1".into()),
            }],
        );
        assert!(ics.starts_with("BEGIN:VCALENDAR\r\n"));
        assert!(ics.contains("DTSTART;VALUE=DATE:20261007\r\n"));
        assert!(ics.contains("DTEND;VALUE=DATE:20261008\r\n"));
        assert!(ics.contains("SUMMARY:Feladat: hívás\\, árajánlat\\; sürgős\r\n"));
        assert!(ics.contains("UID:task-1@autotherm.test\r\n"));
        assert!(ics.ends_with("END:VCALENDAR\r\n"));
    }

    #[test]
    fn long_lines_fold_at_75_octets_on_char_boundaries() {
        let mut out = String::new();
        fold(&format!("SUMMARY:{}", "é".repeat(60)), &mut out);
        for line in out.split("\r\n").filter(|l| !l.is_empty()) {
            assert!(line.len() <= 75, "{} octets", line.len());
        }
        assert!(out.contains("\r\n "));
    }
}

//! The Monday report (0049): last week in numbers, mailed to the managers the settings
//! name. One letter per recipient per week; the usual automatic-mail rules apply (kill
//! switch, send window).

use chrono::{Datelike, NaiveDate, TimeDelta, TimeZone};
use serde_json::json;
use sqlx::PgPool;

use crate::AppState;
use crate::domain::money::{Currency, Money};
use crate::domain::template;
use crate::repo::emails::{self, NewEmail};
use crate::repo::{config, jobs};
use crate::service::email::{format_from, triggers};

/// Last week's figures. Money is in minor units.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct WeekFigures {
    pub new_leads: i64,
    pub top_sources: Vec<(String, i64)>,
    pub quotes_sent: i64,
    pub leads_won: i64,
    pub leads_lost: i64,
    pub new_orders: i64,
    pub new_orders_huf_minor: i64,
    pub orders_finished: i64,
    pub orders_stalled: i64,
    pub invoiced_huf_minor: i64,
    pub invoiced_eur_minor: i64,
    pub received_huf_minor: i64,
    pub received_eur_minor: i64,
    pub overdue_invoices: i64,
    pub overdue_huf_minor: i64,
    pub overdue_eur_minor: i64,
    pub open_blockers_overdue: i64,
    pub new_subscribers: i64,
}

/// The Monday that starts the week holding `day`.
pub fn week_start(day: NaiveDate) -> NaiveDate {
    day - TimeDelta::days(i64::from(day.weekday().num_days_from_monday()))
}

#[allow(clippy::type_complexity)]
pub async fn figures(
    db: &PgPool,
    tz: chrono_tz::Tz,
    monday: NaiveDate,
) -> anyhow::Result<WeekFigures> {
    let start = tz
        .from_local_datetime(&monday.and_hms_opt(0, 0, 0).expect("midnight"))
        .earliest()
        .expect("midnight exists")
        .with_timezone(&chrono::Utc);
    let end = start + TimeDelta::days(7);
    let sunday = monday + TimeDelta::days(6);
    let today = crate::service::business_today(tz);

    let row: (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT
            (SELECT count(*) FROM leads WHERE created_at >= $1 AND created_at < $2),
            (SELECT count(*) FROM email_messages WHERE trigger = 'quotation' AND status::text = 'sent'
                AND sent_at >= $1 AND sent_at < $2),
            (SELECT count(*) FROM orders WHERE lead_id IS NOT NULL AND created_at >= $1 AND created_at < $2),
            (SELECT count(*) FROM lead_stages ls JOIN stage_definitions s ON s.entity = 'lead' AND s.key = ls.stage_key
              WHERE s.is_exit AND ls.entered_at >= $1 AND ls.entered_at < $2),
            (SELECT count(*) FROM orders WHERE created_at >= $1 AND created_at < $2),
            (SELECT coalesce(sum(ov.total_huf_minor), 0)::bigint FROM orders o JOIN order_values ov ON ov.order_id = o.id
              WHERE o.created_at >= $1 AND o.created_at < $2),
            (SELECT count(*) FROM order_stages os JOIN stage_definitions s ON s.entity = 'order' AND s.key = os.stage_key
              WHERE s.is_terminal AND NOT s.is_exit AND os.entered_at >= $1 AND os.entered_at < $2),
            (SELECT coalesce(sum(gross_amount) FILTER (WHERE currency = 'HUF'), 0)::bigint FROM invoices
              WHERE status IN ('issued', 'stornoed') AND kind IN ('invoice', 'storno') AND issue_date BETWEEN $3 AND $4),
            (SELECT coalesce(sum(gross_amount) FILTER (WHERE currency = 'EUR'), 0)::bigint FROM invoices
              WHERE status IN ('issued', 'stornoed') AND kind IN ('invoice', 'storno') AND issue_date BETWEEN $3 AND $4),
            (SELECT coalesce(sum(p.amount_minor) FILTER (WHERE i.currency = 'HUF'), 0)::bigint
               FROM invoice_payments p JOIN invoices i ON i.id = p.invoice_id WHERE p.paid_on BETWEEN $3 AND $4),
            (SELECT coalesce(sum(p.amount_minor) FILTER (WHERE i.currency = 'EUR'), 0)::bigint
               FROM invoice_payments p JOIN invoices i ON i.id = p.invoice_id WHERE p.paid_on BETWEEN $3 AND $4),
            (SELECT count(*) FROM blockers WHERE resolved_at IS NULL AND due_date < $5),
            (SELECT count(*) FROM newsletter_subscriptions WHERE confirmed_at >= $1 AND confirmed_at < $2)",
    )
    .bind(start)
    .bind(end)
    .bind(monday)
    .bind(sunday)
    .bind(today)
    .fetch_one(db)
    .await?;

    let overdue: (i64, i64, i64) = sqlx::query_as(
        "SELECT count(*),
                coalesce(sum(gross_amount - paid_amount) FILTER (WHERE currency = 'HUF'), 0)::bigint,
                coalesce(sum(gross_amount - paid_amount) FILTER (WHERE currency = 'EUR'), 0)::bigint
           FROM invoices
          WHERE kind = 'invoice' AND status = 'issued' AND paid_at IS NULL AND payment_date < $1",
    )
    .bind(today)
    .fetch_one(db)
    .await?;

    let top_sources: Vec<(String, i64)> = sqlx::query_as(
        "SELECT coalesce(s.label, l.source, 'Nincs megadva'), count(*)
           FROM leads l LEFT JOIN lead_sources s ON s.key = l.source
          WHERE l.created_at >= $1 AND l.created_at < $2
          GROUP BY 1 ORDER BY 2 DESC, 1 LIMIT 5",
    )
    .bind(start)
    .bind(end)
    .fetch_all(db)
    .await?;

    let stalled = crate::repo::reports::stalled_orders(db).await?.len() as i64;

    Ok(WeekFigures {
        new_leads: row.0,
        top_sources,
        quotes_sent: row.1,
        leads_won: row.2,
        leads_lost: row.3,
        new_orders: row.4,
        new_orders_huf_minor: row.5,
        orders_finished: row.6,
        orders_stalled: stalled,
        invoiced_huf_minor: row.7,
        invoiced_eur_minor: row.8,
        received_huf_minor: row.9,
        received_eur_minor: row.10,
        overdue_invoices: overdue.0,
        overdue_huf_minor: overdue.1,
        overdue_eur_minor: overdue.2,
        open_blockers_overdue: row.11,
        new_subscribers: row.12,
    })
}

fn huf(minor: i64) -> String {
    Money::new(minor, Currency::HUF).to_string()
}

fn eur(minor: i64) -> String {
    Money::new(minor, Currency::EUR).to_string()
}

fn both(huf_minor: i64, eur_minor: i64) -> String {
    match (huf_minor, eur_minor) {
        (h, 0) => huf(h),
        (0, e) => eur(e),
        (h, e) => format!("{} + {}", huf(h), eur(e)),
    }
}

/// The letter's text. Pure: tested without a database.
pub fn render(monday: NaiveDate, f: &WeekFigures, crm_url: &str) -> (String, String) {
    let sunday = monday + TimeDelta::days(6);
    let subject = format!(
        "Heti jelentés: {} – {}",
        monday.format("%Y.%m.%d."),
        sunday.format("%m.%d.")
    );
    let sources = if f.top_sources.is_empty() {
        "–".to_string()
    } else {
        f.top_sources
            .iter()
            .map(|(label, n)| format!("{label}: {n}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let body = format!(
        "Jó reggelt!\n\nAz elmúlt hét ({} – {}) számokban:\n\n\
         ÉRTÉKESÍTÉS\n\
         Új érdeklődő: {}  (forrás: {})\n\
         Elküldött árajánlat: {}\n\
         Megnyert: {}, elveszett: {}\n\n\
         MEGRENDELÉSEK\n\
         Új megrendelés: {} ({})\n\
         Elkészült: {}\n\
         Elakadt (most): {}\n\
         Lejárt határidejű akadály (most): {}\n\n\
         PÉNZÜGY\n\
         Kiállított számlák: {}\n\
         Beérkezett befizetések: {}\n\
         Lejárt, kifizetetlen számla (most): {} db, {}\n\n\
         HÍRLEVÉL\n\
         Új feliratkozó: {}\n\n\
         Részletek: {}/hu/reports\n\nAutoCRM",
        monday.format("%Y.%m.%d."),
        sunday.format("%Y.%m.%d."),
        f.new_leads,
        sources,
        f.quotes_sent,
        f.leads_won,
        f.leads_lost,
        f.new_orders,
        huf(f.new_orders_huf_minor),
        f.orders_finished,
        f.orders_stalled,
        f.open_blockers_overdue,
        both(f.invoiced_huf_minor, f.invoiced_eur_minor),
        both(f.received_huf_minor, f.received_eur_minor),
        f.overdue_invoices,
        both(f.overdue_huf_minor, f.overdue_eur_minor),
        f.new_subscribers,
        crm_url.trim_end_matches('/'),
    );
    (subject, body)
}

/// Queues last week's report for every recipient. Idempotent per week and recipient.
pub async fn send(state: &AppState) -> anyhow::Result<usize> {
    let recipients = config::weekly_report_recipients(&state.db).await?;
    if recipients.is_empty() {
        return Ok(0);
    }
    let today = crate::service::business_today(state.config.business_tz);
    let monday = week_start(today) - TimeDelta::days(7);
    let f = figures(&state.db, state.config.business_tz, monday).await?;
    let (subject, body) = render(monday, &f, &state.config.public_base_url);
    let html = template::text_to_html(&body);
    let from = format_from(
        &state.config.email.from_name,
        &state.config.email.from_automatic,
    )
    .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let mut tx = state.db.begin().await?;
    let mut queued = 0;
    for to in &recipients {
        let key = format!("weekly_report:{monday}:{to}");
        let id = emails::insert(
            &mut *tx,
            &NewEmail {
                order_id: None,
                lead_id: None,
                partner_id: None,
                blocker_id: None,
                template_key: None,
                trigger: triggers::WEEKLY_REPORT,
                sent_by: None,
                idempotency_key: Some(&key),
                to_address: to,
                cc: &[],
                bcc: &[],
                from_address: &from,
                reply_to: Some(&state.config.email.reply_to_default),
                subject: &subject,
                body_html: &html,
                body_text: &body,
                attachments: json!([]),
                send_after: None,
            },
        )
        .await?;
        if let Some(id) = id {
            jobs::enqueue(
                &mut *tx,
                "send_email",
                json!({ "email_id": id }),
                None,
                Some(&format!("send_email:{id}")),
            )
            .await?;
            queued += 1;
        }
    }
    tx.commit().await?;
    Ok(queued)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weeks_start_on_monday() {
        let wed = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
        assert_eq!(
            week_start(wed),
            NaiveDate::from_ymd_opt(2026, 10, 5).unwrap()
        );
        let mon = NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();
        assert_eq!(week_start(mon), mon);
    }

    #[test]
    fn the_letter_names_the_week_and_the_numbers() {
        let f = WeekFigures {
            new_leads: 7,
            top_sources: vec![("Weboldal".into(), 5), ("Telefon".into(), 2)],
            quotes_sent: 3,
            leads_won: 1,
            ..WeekFigures::default()
        };
        let (subject, body) = render(
            NaiveDate::from_ymd_opt(2026, 9, 28).unwrap(),
            &f,
            "https://crm.example/",
        );
        assert_eq!(subject, "Heti jelentés: 2026.09.28. – 10.04.");
        assert!(body.contains("Új érdeklődő: 7  (forrás: Weboldal: 5, Telefon: 2)"));
        assert!(body.contains("https://crm.example/hu/reports"));
    }
}

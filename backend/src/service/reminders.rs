//! The daily nudges: payment reminders to customers with an overdue invoice, a heads-up
//! to the salesperson whose quote is about to expire, and HR's warning about employee
//! documents running out. Each is recorded once, so a rerun of the day's job sends nothing
//! twice.

use chrono::{NaiveDate, TimeDelta};
use serde::Serialize;
use serde_json::json;
use sqlx::PgExecutor;
use utoipa::ToSchema;

use crate::AppState;
use crate::domain::template::TemplateValues;
use crate::repo::{audit, config, invoices, notifications, orders};
use crate::service::email::{About, AutomaticEmail, queue_automatic, triggers};
use crate::service::invoicing::{hu_date, money_text};
use crate::service::{automation, business_today};

/// A quote is "expiring" this many days before its validity ends.
pub const QUOTE_EXPIRY_DAYS: i64 = 7;
/// HR hears about a document this many days before it runs out, and again on the day.
pub const DOCUMENT_EXPIRY_DAYS: i64 = 30;

// ── Payment reminders ───────────────────────────────────────────────────────

/// Queues the payment reminders that are due today. When a customer is so late that
/// several steps are due at once, only the latest goes out; the earlier ones are recorded
/// as skipped so they never follow it.
pub async fn send_payment_reminders(state: &AppState) -> anyhow::Result<usize> {
    let settings = config::settings(&state.db).await?;
    if !settings.automatic_email_enabled {
        tracing::debug!("automatic email is off; not sending payment reminders");
        return Ok(0);
    }
    let today = business_today(state.config.business_tz);
    let mut sent = 0;
    loop {
        let mut tx = state.db.begin().await?;
        let due = invoices::lock_due_reminders(&mut *tx, today, 20).await?;
        if due.is_empty() {
            break;
        }
        // Latest step first per invoice (the query orders it so).
        let mut handled: Option<i64> = None;
        for d in due {
            if handled == Some(d.invoice_id) {
                invoices::record_reminder(
                    &mut *tx,
                    d.invoice_id,
                    d.step_id,
                    None,
                    "skipped",
                    Some("egy későbbi emlékeztető ment ki helyette"),
                )
                .await?;
                continue;
            }
            handled = Some(d.invoice_id);
            let outcome = queue_reminder(&mut tx, state, d.invoice_id, &d.template_key, d.step_id).await;
            match outcome {
                Ok(Some(email_id)) => {
                    invoices::record_reminder(&mut *tx, d.invoice_id, d.step_id, Some(email_id), "sent", None)
                        .await?;
                    audit::record(
                        &mut *tx,
                        None,
                        "invoice",
                        d.invoice_id,
                        "payment_reminder_sent",
                        json!({ "email_id": email_id, "template": d.template_key }),
                    )
                    .await?;
                    sent += 1;
                }
                Ok(None) => {
                    invoices::record_reminder(
                        &mut *tx,
                        d.invoice_id,
                        d.step_id,
                        None,
                        "skipped",
                        Some("nincs e-mail cím, vagy már sorban volt"),
                    )
                    .await?;
                }
                Err(e) => {
                    tracing::warn!(invoice_id = d.invoice_id, error = %e, "could not queue a payment reminder");
                    invoices::record_reminder(
                        &mut *tx,
                        d.invoice_id,
                        d.step_id,
                        None,
                        "skipped",
                        Some(&format!("nem sikerült: {e}")),
                    )
                    .await?;
                }
            }
        }
        tx.commit().await?;
    }
    if sent > 0 {
        tracing::info!(sent, "payment reminders queued");
    }
    Ok(sent)
}

async fn queue_reminder(
    tx: &mut sqlx::PgConnection,
    state: &AppState,
    invoice_id: i64,
    template_key: &str,
    step_id: i64,
) -> anyhow::Result<Option<i64>> {
    let Some(invoice) = invoices::find(&mut *tx, invoice_id).await? else {
        return Ok(None);
    };
    let Some(order) = orders::find(&mut *tx, invoice.order_id).await? else {
        return Ok(None);
    };
    let Some(to) = automation::customer_recipient(&mut *tx, &order).await? else {
        return Ok(None);
    };
    let mut values = TemplateValues::new();
    values.insert("invoice.number", invoice.number.clone());
    values.insert("invoice.issue_date", hu_date(invoice.issue_date));
    values.insert("invoice.total", money_text(invoice.gross_amount, &invoice.currency));
    if let Some(day) = invoice.payment_date {
        values.insert("invoice.payment_date", hu_date(day));
    }
    Ok(queue_automatic(
        &mut *tx,
        &state.config,
        AutomaticEmail {
            template_key,
            about: About {
                order_id: Some(invoice.order_id),
                partner_id: Some(order.partner_id),
                ..Default::default()
            },
            to: &to,
            trigger: triggers::INVOICE_REMINDER,
            idempotency_key: format!("invoice_reminder:{invoice_id}:{step_id}"),
            attachments: invoice
                .document_id
                .map(|document_id| crate::service::email::AttachmentRef {
                    document_id,
                    filename: None,
                    byte_size: None,
                    mode: None,
                    content_id: None,
                })
                .into_iter()
                .collect(),
            extra_values: values,
            sender: None,
        },
    )
    .await?)
}

// ── Expiring quotes ─────────────────────────────────────────────────────────

/// An open lead whose quote runs out soon (or ran out in the last days).
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct ExpiringQuote {
    pub lead_id: i64,
    pub title: String,
    pub partner_name: Option<String>,
    pub contact_name: Option<String>,
    pub assigned_to: Option<i64>,
    pub assigned_name: Option<String>,
    pub quoted_value_minor: Option<i64>,
    pub currency: Option<String>,
    pub valid_until: NaiveDate,
    /// Negative once it has expired.
    pub days_left: i32,
}

/// Open, unconverted leads with a quote valid until within `days` from `today`; ones that
/// expired up to `days` ago are kept so a missed one is still visible.
pub async fn expiring_quotes(
    db: impl PgExecutor<'_>,
    today: NaiveDate,
    days: i64,
    assigned_to: Option<i64>,
) -> sqlx::Result<Vec<ExpiringQuote>> {
    sqlx::query_as(
        "SELECT l.id AS lead_id, l.title, p.name AS partner_name, l.contact_name,
                l.assigned_to, u.display_name AS assigned_name, l.quoted_value_minor,
                l.currency, l.quote_valid_until AS valid_until,
                (l.quote_valid_until - $1::date)::int AS days_left
           FROM leads l
           LEFT JOIN partners p ON p.id = l.partner_id
           LEFT JOIN users u ON u.id = l.assigned_to
           LEFT JOIN lead_current_stage cs ON cs.lead_id = l.id
           LEFT JOIN stage_definitions sd ON sd.entity = 'lead' AND sd.key = cs.stage_key
          WHERE l.quote_valid_until BETWEEN $1::date - $2::int AND $1::date + $2::int
            AND NOT coalesce(sd.is_terminal, false)
            AND NOT EXISTS (SELECT 1 FROM orders o WHERE o.lead_id = l.id)
            AND ($3::bigint IS NULL OR l.assigned_to = $3)
          ORDER BY l.quote_valid_until, l.id",
    )
    .bind(today)
    .bind(days as i32)
    .bind(assigned_to)
    .fetch_all(db)
    .await
}

/// Tells each salesperson about their quotes expiring within the week, once per quote per
/// validity date. A lead nobody owns goes to the whole office.
pub async fn quote_expiry_alerts(state: &AppState) -> anyhow::Result<usize> {
    let today = business_today(state.config.business_tz);
    let quotes = expiring_quotes(&state.db, today, QUOTE_EXPIRY_DAYS, None).await?;
    let mut sent = 0;
    for q in quotes.into_iter().filter(|q| q.days_left >= 0) {
        let mut tx = state.db.begin().await?;
        let fresh = sqlx::query(
            "INSERT INTO quote_expiry_alerts (lead_id, valid_until) VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
        )
        .bind(q.lead_id)
        .bind(q.valid_until)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            == 1;
        if !fresh {
            continue;
        }
        let title = if q.days_left == 0 {
            "Ma lejár egy árajánlat".to_string()
        } else {
            format!("{} nap múlva lejár egy árajánlat", q.days_left)
        };
        let who = q
            .partner_name
            .as_deref()
            .or(q.contact_name.as_deref())
            .unwrap_or("");
        let body = if who.is_empty() {
            format!("{} — érvényes: {}", q.title, hu_date(q.valid_until))
        } else {
            format!("{} ({who}) — érvényes: {}", q.title, hu_date(q.valid_until))
        };
        let link = format!("/leads/{}", q.lead_id);
        match q.assigned_to {
            Some(user) => {
                notifications::notify_user(&mut *tx, user, "quote_expiry", &title, Some(&body), Some(&link))
                    .await?
            }
            None => {
                notifications::broadcast(
                    &mut *tx,
                    &["admin", "office"],
                    "quote_expiry",
                    &title,
                    Some(&body),
                    Some(&link),
                )
                .await?
            }
        };
        tx.commit().await?;
        sent += 1;
    }
    Ok(sent)
}

// ── Employee documents ──────────────────────────────────────────────────────

/// Tells HR about employee documents that run out within a month, and again on the day
/// they expire. One alert per document per stage per validity date.
pub async fn document_expiry_alerts(state: &AppState) -> anyhow::Result<usize> {
    let today = business_today(state.config.business_tz);
    let soon_until = today + TimeDelta::days(DOCUMENT_EXPIRY_DAYS);
    let rows: Vec<(i64, i64, String, String, NaiveDate)> = sqlx::query_as(
        "SELECT d.id, d.employee_id, e.full_name, d.title, d.valid_until
           FROM employee_documents d
           JOIN employees e ON e.id = d.employee_id
          WHERE d.deleted_at IS NULL AND d.valid_until IS NOT NULL
            AND d.valid_until <= $1 AND d.valid_until >= $2::date - 30
          ORDER BY d.valid_until",
    )
    .bind(soon_until)
    .bind(today)
    .fetch_all(&state.db)
    .await?;
    let mut sent = 0;
    for (id, employee_id, name, title, valid_until) in rows {
        let stage = if valid_until <= today { "expired" } else { "soon" };
        let mut tx = state.db.begin().await?;
        let fresh = sqlx::query(
            "INSERT INTO employee_document_alerts (document_id, stage, valid_until)
             VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        )
        .bind(id)
        .bind(stage)
        .bind(valid_until)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            == 1;
        if !fresh {
            continue;
        }
        let heading = if stage == "expired" {
            "Lejárt egy dolgozói dokumentum"
        } else {
            "Hamarosan lejár egy dolgozói dokumentum"
        };
        notifications::broadcast_hr(
            &mut *tx,
            "employee_document",
            heading,
            Some(&format!("{name}: {title} — {}", hu_date(valid_until))),
            Some(&format!("/hr?employee={employee_id}")),
        )
        .await?;
        tx.commit().await?;
        sent += 1;
    }
    Ok(sent)
}

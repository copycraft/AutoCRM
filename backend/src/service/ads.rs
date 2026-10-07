//! Ad platforms (0049).
//!
//! - Facebook/Instagram lead ads: Meta calls our webhook when someone fills a lead form;
//!   we fetch the answers from the Graph API with the page token and file a lead (source
//!   `meta_ads`), once per form submission.
//! - Meta Conversions API: when a lead becomes an order, the order is reported back as a
//!   Purchase, matched on the hashed email/phone and the click id, so the campaigns learn
//!   which leads turn into work.
//! - Google Ads: won website leads that arrived with a `gclid` are listed in Google's
//!   offline-conversion CSV, which Google Ads fetches on a schedule.

use std::time::Duration;

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::AppState;
use crate::config::MetaConfig;
use crate::domain::email::normalize_address;
use crate::repo::leads::LeadInput;
use crate::repo::{audit, config, jobs, leads, stages};

const GRAPH: &str = "https://graph.facebook.com/v21.0";

/// Meta signs each webhook call with the app secret: `X-Hub-Signature-256: sha256=<hex>`.
pub fn signature_ok(app_secret: &str, body: &[u8], header: Option<&str>) -> bool {
    let Some(given) = header.and_then(|h| h.strip_prefix("sha256=")) else {
        return false;
    };
    let Ok(given) = hex::decode(given.trim()) else {
        return false;
    };
    let mut mac =
        Hmac::<Sha256>::new_from_slice(app_secret.as_bytes()).expect("HMAC takes any key");
    mac.update(body);
    mac.verify_slice(&given).is_ok()
}

#[derive(Debug, Deserialize)]
struct Webhook {
    #[serde(default)]
    entry: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    #[serde(default)]
    changes: Vec<Change>,
}

#[derive(Debug, Deserialize)]
struct Change {
    field: String,
    value: Value,
}

/// The form submissions a webhook call announces.
pub fn leadgen_ids(body: &[u8]) -> Vec<String> {
    let Ok(hook) = serde_json::from_slice::<Webhook>(body) else {
        return Vec::new();
    };
    hook.entry
        .into_iter()
        .flat_map(|e| e.changes)
        .filter(|c| c.field == "leadgen")
        .filter_map(|c| {
            c.value.get("leadgen_id").and_then(|v| {
                v.as_str()
                    .map(str::to_string)
                    .or_else(|| v.as_i64().map(|n| n.to_string()))
            })
        })
        .collect()
}

/// Records each new submission and queues fetching it. Answering the webhook quickly is
/// all Meta asks; the Graph call happens in the job.
pub async fn accept_leadgen(db: &PgPool, ids: &[String]) -> sqlx::Result<usize> {
    let mut tx = db.begin().await?;
    let mut queued = 0;
    for id in ids.iter().filter(|id| !id.is_empty() && id.len() <= 64) {
        let fresh: Option<String> = sqlx::query_scalar(
            "INSERT INTO meta_leadgen (leadgen_id) VALUES ($1) ON CONFLICT DO NOTHING RETURNING leadgen_id",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if fresh.is_some() {
            jobs::enqueue(
                &mut *tx,
                crate::jobs::kinds::META_LEADGEN,
                json!({ "leadgen_id": id }),
                None,
                Some(&format!("meta_leadgen:{id}")),
            )
            .await?;
            queued += 1;
        }
    }
    tx.commit().await?;
    Ok(queued)
}

fn http() -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()?)
}

/// A lead form's answers, as a lead. Meta's standard field names; custom questions go to
/// the description.
pub fn lead_from_answers(
    answers: &Value,
    form_name: Option<&str>,
    campaign: Option<&str>,
) -> Option<LeadInput> {
    let fields = answers.as_array()?;
    let mut name = None;
    let mut first = None;
    let mut last = None;
    let mut email = None;
    let mut phone = None;
    let mut extra = Vec::new();
    for f in fields {
        let key = f.get("name").and_then(Value::as_str).unwrap_or("");
        let value = f
            .get("values")
            .and_then(Value::as_array)
            .map(|v| {
                v.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        if value.trim().is_empty() {
            continue;
        }
        match key {
            "full_name" => name = Some(value),
            "first_name" => first = Some(value),
            "last_name" => last = Some(value),
            "email" => email = normalize_address(&value),
            "phone_number" | "phone" => phone = Some(value),
            other => extra.push(format!("{}: {value}", other.replace('_', " "))),
        }
    }
    let name = name.or_else(|| match (last, first) {
        // Hungarian order: family name first.
        (Some(l), Some(f)) => Some(format!("{l} {f}")),
        (Some(n), None) | (None, Some(n)) => Some(n),
        (None, None) => None,
    });
    if email.is_none() && phone.is_none() {
        return None;
    }
    let display = name
        .clone()
        .or_else(|| email.clone())
        .or_else(|| phone.clone())?;
    let mut description = Vec::new();
    if let Some(c) = campaign {
        description.push(format!("Kampány: {c}"));
    }
    if let Some(f) = form_name {
        description.push(format!("Űrlap: {f}"));
    }
    description.extend(extra);
    Some(LeadInput {
        title: format!("Facebook/Instagram: {display}"),
        partner_id: None,
        contact_id: None,
        contact_name: name,
        contact_email: email,
        contact_phone: phone,
        source: Some("meta_ads".into()),
        source_detail: campaign.map(|c| c.chars().take(200).collect()),
        description: (!description.is_empty()).then(|| description.join("\n")),
        assigned_to: None,
        quoted_value_minor: None,
        currency: None,
        quote_valid_until: None,
    })
}

/// The job: fetch one submission and file it as a lead.
pub async fn fetch_leadgen(state: &AppState, leadgen_id: &str) -> anyhow::Result<()> {
    let Some(token) = state
        .config
        .meta
        .as_ref()
        .and_then(|m| m.page_token.as_deref())
    else {
        anyhow::bail!("META_PAGE_TOKEN is not set; the lead form answers cannot be fetched");
    };
    let already: Option<Option<i64>> =
        sqlx::query_scalar("SELECT lead_id FROM meta_leadgen WHERE leadgen_id = $1")
            .bind(leadgen_id)
            .fetch_optional(&state.db)
            .await?;
    if let Some(Some(_)) = already {
        return Ok(());
    }
    let answer: Value = http()?
        .get(format!("{GRAPH}/{leadgen_id}"))
        .query(&[
            ("access_token", token),
            (
                "fields",
                "created_time,field_data,form_id,campaign_name,ad_name",
            ),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let campaign = answer.get("campaign_name").and_then(Value::as_str);
    let form_name = answer.get("ad_name").and_then(Value::as_str);
    let Some(input) = lead_from_answers(
        answer.get("field_data").unwrap_or(&Value::Null),
        form_name,
        campaign,
    ) else {
        tracing::warn!(
            leadgen_id,
            "a lead form submission without email or phone; skipped"
        );
        return Ok(());
    };

    let mut tx = state.db.begin().await?;
    let lead = leads::insert_by(&mut *tx, &input, None).await?;
    let definitions =
        config::stage_definitions(&mut *tx, crate::domain::stage::StageEntity::Lead).await?;
    let initial = crate::domain::stage::initial_stage(&definitions)
        .ok_or_else(|| anyhow::anyhow!("no active initial lead stage is configured"))?;
    stages::insert_lead_stage(&mut *tx, lead.id, &initial.key, None, None).await?;
    crate::repo::attribution::insert(
        &mut *tx,
        lead.id,
        &crate::repo::attribution::Attribution {
            channel: "paid".into(),
            utm_source: Some("facebook".into()),
            utm_medium: Some("lead_ads".into()),
            utm_campaign: campaign.map(str::to_string),
            referrer: None,
            landing_page: None,
            gclid: None,
            fbclid: None,
        },
    )
    .await?;
    sqlx::query("UPDATE meta_leadgen SET lead_id = $2 WHERE leadgen_id = $1")
        .bind(leadgen_id)
        .bind(lead.id)
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut *tx,
        None,
        "lead",
        lead.id,
        "create",
        json!({ "title": lead.title, "source": "meta_ads", "leadgen_id": leadgen_id }),
    )
    .await?;
    tx.commit().await?;
    tracing::info!(lead_id = lead.id, leadgen_id, "lead ads submission filed");
    if let Err(e) = crate::service::email::website_lead_alert(state, &lead).await {
        tracing::error!(lead_id = lead.id, error = %e, "could not queue the lead alert");
    }
    if let Err(e) = crate::service::notifications::lead_arrived(&state.db, &lead).await {
        tracing::error!(lead_id = lead.id, error = %e, "could not raise the lead notification");
    }
    Ok(())
}

fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

/// Phone as Meta wants it hashed: digits only, with the country code (Hungarian numbers
/// written 06… get 36).
pub fn normalize_phone(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    let digits = if let Some(rest) = digits.strip_prefix("06") {
        format!("36{rest}")
    } else if let Some(rest) = digits.strip_prefix("00") {
        rest.to_string()
    } else {
        digits
    };
    (digits.len() >= 8).then_some(digits)
}

/// The Purchase event for a won lead. Pure, so the hashing and shape are tested.
#[allow(clippy::too_many_arguments)]
pub fn purchase_event(
    order_id: i64,
    at: DateTime<Utc>,
    email: Option<&str>,
    phone: Option<&str>,
    fbclid: Option<&str>,
    clicked_at: Option<DateTime<Utc>>,
    value_minor: i64,
    currency: &str,
) -> Value {
    let mut user = serde_json::Map::new();
    if let Some(e) = email.and_then(normalize_address) {
        user.insert("em".into(), json!([sha256_hex(&e)]));
    }
    if let Some(p) = phone.and_then(normalize_phone) {
        user.insert("ph".into(), json!([sha256_hex(&p)]));
    }
    if let Some(f) = fbclid {
        // fbc: fb.<subdomain index>.<creation time ms>.<fbclid>
        let ms = clicked_at.unwrap_or(at).timestamp_millis();
        user.insert("fbc".into(), json!(format!("fb.1.{ms}.{f}")));
    }
    json!({
        "event_name": "Purchase",
        "event_time": at.timestamp(),
        "event_id": format!("order-{order_id}"),
        "action_source": "system_generated",
        "user_data": user,
        "custom_data": {
            "currency": currency,
            "value": value_minor as f64 / 100.0,
        }
    })
}

/// The Conversions API is on: a pixel and its token.
pub fn capi_enabled(meta: Option<&MetaConfig>) -> bool {
    meta.is_some_and(|m| m.pixel_id.is_some() && m.capi_token.is_some())
}

/// Queues the report of a won lead, when the Conversions API is set up.
pub async fn queue_conversion(state: &AppState, lead_id: i64, order_id: i64) -> sqlx::Result<()> {
    if !capi_enabled(state.config.meta.as_ref()) {
        return Ok(());
    }
    let mut conn = state.db.acquire().await?;
    jobs::enqueue(
        &mut *conn,
        crate::jobs::kinds::META_CONVERSION,
        json!({ "lead_id": lead_id, "order_id": order_id }),
        None,
        Some(&format!("meta_conversion:{lead_id}")),
    )
    .await?;
    Ok(())
}

/// The job: report one won lead to Meta, once.
pub async fn send_conversion(state: &AppState, lead_id: i64, order_id: i64) -> anyhow::Result<()> {
    let Some(meta) = state.config.meta.as_ref().filter(|m| capi_enabled(Some(m))) else {
        return Ok(());
    };
    let done: Option<i64> = sqlx::query_scalar(
        "SELECT lead_id FROM ad_conversions WHERE lead_id = $1 AND platform = 'meta'",
    )
    .bind(lead_id)
    .fetch_optional(&state.db)
    .await?;
    if done.is_some() {
        return Ok(());
    }
    #[derive(sqlx::FromRow)]
    struct Row {
        contact_email: Option<String>,
        contact_phone: Option<String>,
        fbclid: Option<String>,
        lead_created: DateTime<Utc>,
        currency: String,
        total_minor: i64,
        source: Option<String>,
    }
    let row: Row = sqlx::query_as(
        "SELECT l.contact_email, l.contact_phone, a.fbclid, l.created_at AS lead_created,
                o.currency, ov.total_minor, l.source
           FROM leads l
           JOIN orders o ON o.id = $2 AND o.lead_id = l.id
           JOIN order_values ov ON ov.order_id = o.id
           LEFT JOIN lead_attribution a ON a.lead_id = l.id
          WHERE l.id = $1",
    )
    .bind(lead_id)
    .bind(order_id)
    .fetch_one(&state.db)
    .await?;
    // Only leads Meta could have sent: its lead ads, or a website visit from a Meta click.
    if row.fbclid.is_none() && row.source.as_deref() != Some("meta_ads") {
        return Ok(());
    }
    let event = purchase_event(
        order_id,
        Utc::now(),
        row.contact_email.as_deref(),
        row.contact_phone.as_deref(),
        row.fbclid.as_deref(),
        Some(row.lead_created),
        row.total_minor,
        &row.currency,
    );
    let pixel = meta.pixel_id.as_deref().unwrap_or_default();
    let token = meta.capi_token.as_deref().unwrap_or_default();
    let response = http()?
        .post(format!("{GRAPH}/{pixel}/events"))
        .query(&[("access_token", token)])
        .json(&json!({ "data": [event] }))
        .send()
        .await?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!(
            "Meta refused the conversion ({status}): {}",
            text.chars().take(300).collect::<String>()
        );
    }
    sqlx::query(
        "INSERT INTO ad_conversions (lead_id, platform, order_id, response) VALUES ($1, 'meta', $2, $3)
         ON CONFLICT DO NOTHING",
    )
    .bind(lead_id)
    .bind(order_id)
    .bind(text.chars().take(1000).collect::<String>())
    .execute(&state.db)
    .await?;
    tracing::info!(lead_id, order_id, "conversion reported to Meta");
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
pub struct GoogleConversion {
    pub gclid: String,
    pub converted_at: DateTime<Utc>,
    pub currency: String,
    pub total_minor: i64,
}

/// Won leads with a Google click id, the last 90 days (Google takes clicks up to 90 days
/// old).
pub async fn google_conversions(db: &PgPool) -> sqlx::Result<Vec<GoogleConversion>> {
    sqlx::query_as(
        "SELECT DISTINCT ON (a.gclid) a.gclid, o.created_at AS converted_at, o.currency, ov.total_minor
           FROM lead_attribution a
           JOIN orders o ON o.lead_id = a.lead_id
           JOIN order_values ov ON ov.order_id = o.id
          WHERE a.gclid IS NOT NULL AND o.created_at > now() - interval '90 days'
          ORDER BY a.gclid, o.created_at",
    )
    .fetch_all(db)
    .await
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Google Ads' offline-conversion import format (the "Google Click ID" template).
pub fn google_csv(rows: &[GoogleConversion], conversion_name: &str, tz: chrono_tz::Tz) -> String {
    let mut out = String::from("Parameters:TimeZone=Europe/Budapest\r\n");
    out.push_str(
        "Google Click ID,Conversion Name,Conversion Time,Conversion Value,Conversion Currency\r\n",
    );
    for r in rows {
        let at = r
            .converted_at
            .with_timezone(&tz)
            .format("%Y-%m-%d %H:%M:%S");
        out.push_str(&format!(
            "{},{},{at},{:.2},{}\r\n",
            csv_field(&r.gclid),
            csv_field(conversion_name),
            r.total_minor as f64 / 100.0,
            r.currency
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signature_is_the_app_secret_hmac() {
        let body = br#"{"object":"page"}"#;
        let mut mac = Hmac::<Sha256>::new_from_slice(b"secret").unwrap();
        mac.update(body);
        let good = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
        assert!(signature_ok("secret", body, Some(&good)));
        assert!(!signature_ok("other", body, Some(&good)));
        assert!(!signature_ok("secret", body, None));
    }

    #[test]
    fn leadgen_ids_come_out_of_the_webhook() {
        let body = br#"{"object":"page","entry":[{"id":"1","changes":[
            {"field":"leadgen","value":{"leadgen_id":"444","page_id":"1"}},
            {"field":"feed","value":{}},
            {"field":"leadgen","value":{"leadgen_id":555}}]}]}"#;
        assert_eq!(
            leadgen_ids(body),
            vec!["444".to_string(), "555".to_string()]
        );
        assert!(leadgen_ids(b"not json").is_empty());
    }

    #[test]
    fn answers_become_a_lead() {
        let answers = json!([
            {"name": "first_name", "values": ["Péter"]},
            {"name": "last_name", "values": ["Kovács"]},
            {"name": "email", "values": ["Peter@Example.HU"]},
            {"name": "jarmu_tipusa", "values": ["Sprinter"]}
        ]);
        let lead = lead_from_answers(&answers, Some("Hűtős űrlap"), Some("Őszi kampány")).unwrap();
        assert_eq!(lead.contact_name.as_deref(), Some("Kovács Péter"));
        assert_eq!(lead.contact_email.as_deref(), Some("peter@example.hu"));
        assert_eq!(lead.source.as_deref(), Some("meta_ads"));
        assert!(lead.description.unwrap().contains("jarmu tipusa: Sprinter"));
        assert!(
            lead_from_answers(&json!([{"name": "full_name", "values": ["X"]}]), None, None)
                .is_none()
        );
    }

    #[test]
    fn purchase_events_hash_what_identifies_the_customer() {
        assert_eq!(
            normalize_phone("06 30 123 4567").as_deref(),
            Some("36301234567")
        );
        assert_eq!(
            normalize_phone("+36 30 123 4567").as_deref(),
            Some("36301234567")
        );
        let at: DateTime<Utc> = "2026-10-01T10:00:00Z".parse().unwrap();
        let e = purchase_event(
            7,
            at,
            Some("A@B.hu"),
            Some("06301234567"),
            Some("abc"),
            Some(at),
            12_345_600,
            "HUF",
        );
        assert_eq!(e["event_id"], "order-7");
        assert_eq!(e["user_data"]["em"][0], sha256_hex("a@b.hu"));
        assert_eq!(e["user_data"]["ph"][0], sha256_hex("36301234567"));
        assert_eq!(
            e["user_data"]["fbc"],
            format!("fb.1.{}.abc", at.timestamp_millis())
        );
        assert_eq!(e["custom_data"]["value"], 123456.0);
    }

    #[test]
    fn the_google_csv_has_its_header_lines() {
        let rows = vec![GoogleConversion {
            gclid: "Cj0KCQ".into(),
            converted_at: "2026-10-01T10:00:00Z".parse().unwrap(),
            currency: "HUF".into(),
            total_minor: 15_000_000,
        }];
        let csv = google_csv(&rows, "Megrendeles", chrono_tz::Europe::Budapest);
        assert!(csv.starts_with("Parameters:TimeZone=Europe/Budapest\r\nGoogle Click ID,"));
        assert!(csv.contains("Cj0KCQ,Megrendeles,2026-10-01 12:00:00,150000.00,HUF"));
    }
}

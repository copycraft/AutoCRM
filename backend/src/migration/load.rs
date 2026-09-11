//! Phase M3: transform the raw extract into AutoCRM rows.
//!
//! Idempotent: every row is upserted by `minicrm_id`, so the load can be re-run as the
//! mapping improves. The full source JSON is kept in `raw_import` — nothing unmapped is
//! silently dropped. Re-running overwrites migrated fields, so stop re-running once staff
//! start editing migrated records.
//!
//! MiniCRM custom fields differ per account; the mapping file (see
//! docs/migration/mapping.example.json) says which categories are orders or leads, how
//! statuses map to stage keys, and which file fields hold which photo category.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{Context, bail};
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::extract::as_i64;
use super::fetch::{FetchedEntry, read_jsonl};
use super::manifest::{ManifestEntry, field_group};
use crate::AppState;
use crate::domain::email::normalize_address;
use crate::domain::media::{DocumentKind, ImageCategory};
use crate::domain::partner::PartnerKind;
use crate::domain::stage::StageEntity;
use crate::repo::documents::{self, NewDocument};
use crate::repo::images::{self, NewImage};
use crate::repo::{config, jobs};

fn default_image_category() -> String {
    "production".into()
}

fn default_currency() -> String {
    "HUF".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct Mapping {
    /// MiniCRM CategoryId → "order" | "lead" | "skip".
    pub categories: BTreeMap<String, String>,
    #[serde(default)]
    pub order_statuses: BTreeMap<String, String>,
    #[serde(default)]
    pub lead_statuses: BTreeMap<String, String>,
    pub default_order_stage: String,
    pub default_lead_stage: String,
    /// Field group as printed in the manifest summary (e.g. "$.Atveteli_kepek[]") → image category.
    #[serde(default)]
    pub image_fields: BTreeMap<String, String>,
    #[serde(default = "default_image_category")]
    pub default_image_category: String,
    #[serde(default = "default_currency")]
    pub default_currency: String,
    /// Optional project field holding the currency code.
    pub order_currency_field: Option<String>,
    /// Optional project field holding the order's total value (becomes one line item).
    pub order_value_field: Option<String>,
}

fn parse_image_category(s: &str) -> Option<ImageCategory> {
    match s {
        "intake" => Some(ImageCategory::Intake),
        "production" => Some(ImageCategory::Production),
        "completion" => Some(ImageCategory::Completion),
        "marketing" => Some(ImageCategory::Marketing),
        _ => None,
    }
}

impl Mapping {
    pub fn load(path: &Path) -> anyhow::Result<Mapping> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading mapping {}", path.display()))?;
        let mapping: Mapping = serde_json::from_str(&text).context("parsing mapping")?;
        for (category, entity) in &mapping.categories {
            if !matches!(entity.as_str(), "order" | "lead" | "skip") {
                bail!("category {category}: entity must be order, lead or skip, got '{entity}'");
            }
        }
        for field in mapping
            .image_fields
            .values()
            .chain(std::iter::once(&mapping.default_image_category))
        {
            if parse_image_category(field).is_none() {
                bail!("unknown image category '{field}'");
            }
        }
        if !matches!(mapping.default_currency.as_str(), "HUF" | "EUR") {
            bail!("default_currency must be HUF or EUR");
        }
        Ok(mapping)
    }

    pub fn entity_for(&self, category: Option<i64>) -> Option<&str> {
        category
            .and_then(|c| self.categories.get(&c.to_string()))
            .map(String::as_str)
    }

    fn image_category(&self, field: &str) -> ImageCategory {
        self.image_fields
            .get(&field_group(field))
            .and_then(|c| parse_image_category(c))
            .or_else(|| parse_image_category(&self.default_image_category))
            .unwrap_or(ImageCategory::Production)
    }
}

#[derive(Debug, Default, Serialize)]
pub struct LoadSummary {
    pub partners: usize,
    pub contacts: usize,
    pub orders: usize,
    pub leads: usize,
    pub skipped_projects: usize,
    pub images: usize,
    pub documents: usize,
    pub files_without_project: usize,
    pub files_not_on_order: usize,
    pub files_not_fetched: usize,
    pub problems: Vec<String>,
}

impl LoadSummary {
    fn problem(&mut self, message: String) {
        tracing::warn!("{message}");
        if self.problems.len() < 1000 {
            self.problems.push(message);
        }
    }
}

pub fn text(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
    .filter(|s| !s.is_empty())
}

fn display_name(v: &Value, id: i64) -> String {
    text(v, "Name")
        .or_else(|| {
            let parts: Vec<String> = ["LastName", "FirstName"]
                .iter()
                .filter_map(|k| text(v, k))
                .collect();
            (!parts.is_empty()).then(|| parts.join(" "))
        })
        .unwrap_or_else(|| format!("MiniCRM #{id}"))
}

pub fn parse_timestamp(s: &str, tz: Tz) -> Option<DateTime<Utc>> {
    let s = s.trim();
    for format in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y.%m.%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, format) {
            return tz
                .from_local_datetime(&naive)
                .earliest()
                .map(|d| d.with_timezone(&Utc));
        }
    }
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(s, "%Y.%m.%d"))
        .ok()?;
    tz.from_local_datetime(&date.and_hms_opt(12, 0, 0)?)
        .earliest()
        .map(|d| d.with_timezone(&Utc))
}

/// "1 234 567,50", "1234567.5", 1234567 → Decimal.
pub fn parse_amount(v: &Value) -> Option<Decimal> {
    match v {
        Value::Number(n) => n.to_string().parse().ok(),
        Value::String(s) => {
            let cleaned: String = s
                .chars()
                .filter(|c| !c.is_whitespace() && *c != '\u{a0}')
                .collect::<String>()
                .replace(',', ".");
            cleaned.parse().ok()
        }
        _ => None,
    }
}

fn read_dir_json(dir: &Path) -> anyhow::Result<Vec<Value>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut files: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    files.sort();
    files
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .map(|p| {
            let text = std::fs::read_to_string(&p)?;
            serde_json::from_str(&text).with_context(|| format!("parsing {}", p.display()))
        })
        .collect()
}

pub async fn run(
    state: &AppState,
    mapping: &Mapping,
    raw_dir: &Path,
    data_dir: &Path,
) -> anyhow::Result<LoadSummary> {
    let mut summary = LoadSummary::default();

    // Fail early if the mapping names stages that don't exist.
    for (entity, keys) in [
        (
            StageEntity::Order,
            mapping
                .order_statuses
                .values()
                .chain(std::iter::once(&mapping.default_order_stage))
                .collect::<Vec<_>>(),
        ),
        (
            StageEntity::Lead,
            mapping
                .lead_statuses
                .values()
                .chain(std::iter::once(&mapping.default_lead_stage))
                .collect::<Vec<_>>(),
        ),
    ] {
        let defined = config::stage_definitions(&state.db, entity).await?;
        for key in keys {
            if !defined.iter().any(|d| &d.key == key) {
                bail!("mapping uses unknown {} stage '{key}'", entity.as_str());
            }
        }
    }

    load_contacts(state, raw_dir, &mut summary).await?;
    load_projects(state, mapping, raw_dir, &mut summary).await?;
    load_files(state, mapping, data_dir, &mut summary).await?;
    Ok(summary)
}

async fn load_contacts(
    state: &AppState,
    raw_dir: &Path,
    summary: &mut LoadSummary,
) -> anyhow::Result<()> {
    let contacts = read_dir_json(&raw_dir.join("contacts"))?;

    // Pass 1: businesses, and people who don't belong to a business, become partners.
    for c in &contacts {
        let Some(id) = c.get("Id").and_then(as_i64) else {
            summary.problem("contact without Id".into());
            continue;
        };
        let business_id = c
            .get("BusinessId")
            .and_then(as_i64)
            .filter(|b| *b > 0 && *b != id);
        let is_business = text(c, "Type").is_some_and(|t| t.eq_ignore_ascii_case("business"));
        if !is_business && business_id.is_some() {
            continue;
        }
        let kind = if is_business {
            PartnerKind::Business
        } else {
            PartnerKind::Person
        };
        let raw_email = text(c, "Email");
        let email = raw_email.as_deref().and_then(normalize_address);
        if raw_email.is_some() && email.is_none() {
            summary.problem(format!(
                "contact {id}: unusable email '{}' kept in raw_import only",
                raw_email.unwrap_or_default()
            ));
        }
        sqlx::query_scalar!(
            "INSERT INTO partners (kind, name, email, phone, website, minicrm_id, raw_import)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (minicrm_id) DO UPDATE
             SET kind = EXCLUDED.kind, name = EXCLUDED.name, email = EXCLUDED.email, phone = EXCLUDED.phone,
                 website = EXCLUDED.website, raw_import = EXCLUDED.raw_import
             RETURNING id",
            kind as PartnerKind,
            display_name(c, id),
            email,
            text(c, "Phone"),
            text(c, "Url"),
            id,
            c
        )
        .fetch_one(&state.db)
        .await?;
        summary.partners += 1;
    }

    // Pass 2: people at a business become that partner's contacts.
    for c in &contacts {
        let Some(id) = c.get("Id").and_then(as_i64) else {
            continue;
        };
        let Some(business_id) = c
            .get("BusinessId")
            .and_then(as_i64)
            .filter(|b| *b > 0 && *b != id)
        else {
            continue;
        };
        if text(c, "Type").is_some_and(|t| t.eq_ignore_ascii_case("business")) {
            continue;
        }
        let partner_id =
            sqlx::query_scalar!("SELECT id FROM partners WHERE minicrm_id = $1", business_id)
                .fetch_optional(&state.db)
                .await?;
        let Some(partner_id) = partner_id else {
            summary.problem(format!(
                "person {id}: business {business_id} was not extracted; person not loaded"
            ));
            continue;
        };
        sqlx::query_scalar!(
            "INSERT INTO contacts (partner_id, name, email, phone, position, minicrm_id, raw_import)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (minicrm_id) DO UPDATE
             SET partner_id = EXCLUDED.partner_id, name = EXCLUDED.name, email = EXCLUDED.email,
                 phone = EXCLUDED.phone, position = EXCLUDED.position, raw_import = EXCLUDED.raw_import
             RETURNING id",
            partner_id,
            display_name(c, id),
            text(c, "Email").as_deref().and_then(normalize_address),
            text(c, "Phone"),
            text(c, "Position"),
            id,
            c
        )
        .fetch_one(&state.db)
        .await?;
        summary.contacts += 1;
    }
    Ok(())
}

/// Resolves a MiniCRM ContactId to (partner, contact, contact name).
async fn resolve_party(
    state: &AppState,
    contact: Option<i64>,
) -> anyhow::Result<Option<(i64, Option<i64>, Option<String>)>> {
    let Some(contact) = contact else {
        return Ok(None);
    };
    if let Some(pid) = sqlx::query_scalar!("SELECT id FROM partners WHERE minicrm_id = $1", contact)
        .fetch_optional(&state.db)
        .await?
    {
        return Ok(Some((pid, None, None)));
    }
    let row = sqlx::query!(
        "SELECT id, partner_id, name FROM contacts WHERE minicrm_id = $1",
        contact
    )
    .fetch_optional(&state.db)
    .await?;
    Ok(row.map(|r| (r.partner_id, Some(r.id), Some(r.name))))
}

async fn load_projects(
    state: &AppState,
    mapping: &Mapping,
    raw_dir: &Path,
    summary: &mut LoadSummary,
) -> anyhow::Result<()> {
    let tz = state.config.business_tz;
    for p in read_dir_json(&raw_dir.join("projects"))? {
        let Some(id) = p.get("Id").and_then(as_i64) else {
            summary.problem("project without Id".into());
            continue;
        };
        let category = p.get("CategoryId").and_then(as_i64);
        let entity = match mapping.entity_for(category) {
            Some(e) => e,
            None => {
                summary.problem(format!(
                    "project {id}: category {category:?} is not in the mapping; skipped"
                ));
                summary.skipped_projects += 1;
                continue;
            }
        };
        if entity == "skip" {
            summary.skipped_projects += 1;
            continue;
        }
        if p.get("Deleted").and_then(as_i64).is_some_and(|d| d != 0) {
            summary.skipped_projects += 1;
            continue;
        }

        let title = display_name(&p, id);
        let created_at = text(&p, "CreatedAt").and_then(|s| parse_timestamp(&s, tz));
        let status_changed_at = text(&p, "StatusUpdatedAt")
            .and_then(|s| parse_timestamp(&s, tz))
            .or(created_at);
        let status = p.get("StatusId").and_then(as_i64).map(|s| s.to_string());
        let party = resolve_party(state, p.get("ContactId").and_then(as_i64)).await?;

        if entity == "order" {
            let Some((partner_id, contact_id, _)) = party else {
                summary.problem(format!(
                    "project {id}: contact {:?} not loaded; order skipped",
                    p.get("ContactId")
                ));
                summary.skipped_projects += 1;
                continue;
            };
            let stage = status
                .as_ref()
                .and_then(|s| mapping.order_statuses.get(s))
                .cloned()
                .unwrap_or_else(|| mapping.default_order_stage.clone());
            let currency = mapping
                .order_currency_field
                .as_deref()
                .and_then(|f| text(&p, f))
                .map(|c| c.to_uppercase())
                .filter(|c| matches!(c.as_str(), "HUF" | "EUR"))
                .unwrap_or_else(|| mapping.default_currency.clone());
            if created_at.is_none() {
                summary.problem(format!(
                    "project {id}: no parseable CreatedAt; valuation date set to today"
                ));
            }
            let valuation_date = created_at
                .map(|t| t.with_timezone(&tz).date_naive())
                .unwrap_or_else(|| crate::service::business_today(tz));

            let order_id = sqlx::query_scalar!(
                "INSERT INTO orders (number, title, partner_id, contact_id, currency, valuation_date, minicrm_id, raw_import, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, coalesce($9, now()))
                 ON CONFLICT (minicrm_id) DO UPDATE
                 SET title = EXCLUDED.title, partner_id = EXCLUDED.partner_id, contact_id = EXCLUDED.contact_id,
                     valuation_date = EXCLUDED.valuation_date, raw_import = EXCLUDED.raw_import
                 RETURNING id",
                format!("MC-{id}"),
                title,
                partner_id,
                contact_id,
                currency,
                valuation_date,
                id,
                &p,
                created_at
            )
            .fetch_one(&state.db)
            .await?;

            sqlx::query!(
                "INSERT INTO order_stages (order_id, stage_key, entered_at, note)
                 SELECT $1, $2, coalesce($3, now()), 'MiniCRM import'
                 WHERE NOT EXISTS (SELECT 1 FROM order_stages WHERE order_id = $1)",
                order_id,
                stage,
                status_changed_at
            )
            .execute(&state.db)
            .await?;

            if let Some(amount) = mapping
                .order_value_field
                .as_deref()
                .and_then(|f| p.get(f))
                .and_then(parse_amount)
            {
                let minor = (amount * Decimal::from(100))
                    .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
                match i64::try_from(minor) {
                    Ok(unit_price) => {
                        sqlx::query!(
                            "INSERT INTO order_items (order_id, position, description, quantity, unit_price, currency)
                             SELECT $1, 10, 'MiniCRM érték', 1, $2, currency FROM orders
                             WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM order_items WHERE order_id = $1)",
                            order_id,
                            unit_price
                        )
                        .execute(&state.db)
                        .await?;
                    }
                    Err(_) => summary.problem(format!("project {id}: value {amount} out of range")),
                }
            }
            summary.orders += 1;
        } else {
            let (partner_id, contact_id, contact_name) = match party {
                Some((p, c, n)) => (Some(p), c, n),
                None => (None, None, None),
            };
            let stage = status
                .as_ref()
                .and_then(|s| mapping.lead_statuses.get(s))
                .cloned()
                .unwrap_or_else(|| mapping.default_lead_stage.clone());
            let lead_id = sqlx::query_scalar!(
                "INSERT INTO leads (title, partner_id, contact_id, contact_name, source, minicrm_id, raw_import, created_at)
                 VALUES ($1, $2, $3, $4, 'minicrm', $5, $6, coalesce($7, now()))
                 ON CONFLICT (minicrm_id) DO UPDATE
                 SET title = EXCLUDED.title, partner_id = EXCLUDED.partner_id, contact_id = EXCLUDED.contact_id,
                     contact_name = EXCLUDED.contact_name, raw_import = EXCLUDED.raw_import
                 RETURNING id",
                title,
                partner_id,
                contact_id,
                contact_name,
                id,
                &p,
                created_at
            )
            .fetch_one(&state.db)
            .await?;
            sqlx::query!(
                "INSERT INTO lead_stages (lead_id, stage_key, entered_at, note)
                 SELECT $1, $2, coalesce($3, now()), 'MiniCRM import'
                 WHERE NOT EXISTS (SELECT 1 FROM lead_stages WHERE lead_id = $1)",
                lead_id,
                stage,
                status_changed_at
            )
            .execute(&state.db)
            .await?;
            summary.leads += 1;
        }
    }
    Ok(())
}

async fn load_files(
    state: &AppState,
    mapping: &Mapping,
    data_dir: &Path,
    summary: &mut LoadSummary,
) -> anyhow::Result<()> {
    let manifest: Vec<ManifestEntry> = read_jsonl(&data_dir.join("manifest.jsonl")).await?;
    let fetched: HashMap<String, FetchedEntry> =
        read_jsonl::<FetchedEntry>(&data_dir.join("fetched.jsonl"))
            .await?
            .into_iter()
            .map(|f| (f.source_url.clone(), f))
            .collect();
    let orders: HashMap<i64, i64> = sqlx::query!(
        r#"SELECT id, minicrm_id AS "minicrm_id!" FROM orders WHERE minicrm_id IS NOT NULL"#
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|r| (r.minicrm_id, r.id))
    .collect();

    for entry in &manifest {
        let Some(project) = entry.minicrm_project_id else {
            summary.files_without_project += 1;
            continue;
        };
        let Some(&order_id) = orders.get(&project) else {
            summary.files_not_on_order += 1;
            continue;
        };
        let Some(file) = fetched.get(&entry.source_url) else {
            summary.files_not_fetched += 1;
            continue;
        };
        let hash = hex::decode(&file.sha256).context("bad sha256 in fetched.jsonl")?;
        let mut conn = state.db.acquire().await?;
        if file.content_type.starts_with("image/") {
            let category = mapping.image_category(&entry.field);
            let (image, created) = images::insert(
                &mut conn,
                &NewImage {
                    order_id,
                    category,
                    storage_key: &file.storage_key,
                    content_type: &file.content_type,
                    original_filename: entry.filename.as_deref(),
                    content_hash: &hash,
                    byte_size: file.byte_size,
                    uploaded_by: None,
                    source_ref: Some(&entry.source_url),
                },
            )
            .await?;
            if created {
                if category.is_immutable() {
                    if let Some(until) = state.storage.intake_lock_until(Utc::now()) {
                        state.storage.lock_object(&file.storage_key, until).await?;
                    }
                }
                jobs::enqueue(
                    &mut *conn,
                    "process_image",
                    json!({ "image_id": image.id }),
                    None,
                    Some(&format!("process_image:{}", image.id)),
                )
                .await?;
                summary.images += 1;
            }
        } else {
            let filename = entry
                .filename
                .clone()
                .unwrap_or_else(|| format!("minicrm-{}", &file.sha256[..12]));
            let (_, created) = documents::insert(
                &mut conn,
                &NewDocument {
                    order_id,
                    kind: DocumentKind::Other,
                    filename: &filename,
                    content_type: &file.content_type,
                    storage_key: &file.storage_key,
                    content_hash: &hash,
                    byte_size: file.byte_size,
                    uploaded_by: None,
                    source_ref: Some(&entry.source_url),
                },
            )
            .await?;
            if created {
                summary.documents += 1;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn d(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    #[test]
    fn amounts_in_hungarian_formats() {
        assert_eq!(parse_amount(&json!("1 234 567,50")), Some(d("1234567.50")));
        assert_eq!(parse_amount(&json!("1\u{a0}000")), Some(d("1000")));
        assert_eq!(parse_amount(&json!(99.5)), Some(d("99.5")));
        assert_eq!(parse_amount(&json!("n/a")), None);
    }

    #[test]
    fn timestamps_in_minicrm_formats() {
        let tz = chrono_tz::Europe::Budapest;
        assert_eq!(
            parse_timestamp("2019-05-10 12:33:00", tz)
                .unwrap()
                .to_rfc3339(),
            "2019-05-10T10:33:00+00:00"
        );
        assert!(parse_timestamp("2019.05.10", tz).is_some());
        assert!(parse_timestamp("garbage", tz).is_none());
    }

    #[test]
    fn names_fall_back_sensibly() {
        assert_eq!(
            display_name(&json!({"Name": " Müller GmbH "}), 1),
            "Müller GmbH"
        );
        assert_eq!(
            display_name(&json!({"FirstName": "János", "LastName": "Kovács"}), 2),
            "Kovács János"
        );
        assert_eq!(display_name(&json!({}), 3), "MiniCRM #3");
    }

    #[test]
    fn image_fields_map_to_categories() {
        let mapping: Mapping = serde_json::from_value(json!({
            "categories": {"1": "order"},
            "default_order_stage": "completed",
            "default_lead_stage": "lost",
            "image_fields": {"$.Atveteli_kepek[]": "intake"}
        }))
        .unwrap();
        assert_eq!(
            mapping.image_category("$.Atveteli_kepek[4]"),
            ImageCategory::Intake
        );
        assert_eq!(
            mapping.image_category("$.Egyeb[0]"),
            ImageCategory::Production
        );
        assert_eq!(mapping.entity_for(Some(1)), Some("order"));
        assert_eq!(mapping.entity_for(Some(2)), None);
    }
}

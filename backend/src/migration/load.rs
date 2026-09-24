//! Phase M3: transform the raw extract into AutoCRM rows.
//!
//! Idempotent: every row is upserted by `minicrm_id`, so the load can be re-run as the
//! mapping improves. The full source JSON is kept in `raw_import` — nothing unmapped is
//! silently dropped. Re-running overwrites migrated fields, so stop re-running once staff
//! start editing migrated records.
//!
//! MiniCRM custom fields differ per account; the mapping file (see
//! docs/migration/mapping.example.json) says which categories are orders or leads, how
//! statuses map to stage keys, which file fields hold which photo category, and — since
//! V1.1 — where every remaining order and partner column comes from. Values the mapping
//! does not cover hard-fail before a single row is written (`check_coverage`): a silent
//! default is how 1000 orders ended up with no project type.

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
use crate::repo::{config, jobs, vehicles};

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

    /// Destination `orders` column → path in the MiniCRM project JSON. Paths are dotted
    /// (`Jarmu.Rendszam`); a leading `$.` is ignored so manifest-style paths work.
    /// Keys: see `ORDER_FIELD_KEYS`.
    #[serde(default)]
    pub order_fields: BTreeMap<String, String>,
    /// Destination `partners` column → path in the MiniCRM contact JSON.
    /// Keys: see `PARTNER_FIELD_KEYS`.
    #[serde(default)]
    pub partner_fields: BTreeMap<String, String>,
    /// MiniCRM project-type value → `project_types.key`. An unmapped value hard-fails.
    #[serde(default)]
    pub project_types: BTreeMap<String, String>,
    /// MiniCRM assignee value (user id or name) → AutoCRM user e-mail. Unmapped hard-fails.
    #[serde(default)]
    pub assignees: BTreeMap<String, String>,
    /// MiniCRM country value → ISO-3166 alpha-2. Two-letter source values pass through.
    #[serde(default)]
    pub countries: BTreeMap<String, String>,
    /// TODO(client): the vehicle plate may be a structured field or only inside project
    /// names. With no `order_fields.vehicle_plate` path, this regex is applied to the
    /// project name and its first capture group (or the whole match) becomes the plate.
    /// `autocrm-migrate load --dry-run` reports how many orders would get one.
    pub vehicle_plate_from_title: Option<String>,
    /// TODO(client): where the customer-facing order number comes from. Absent →
    /// `MC-{MiniCRM id}`. Set it to the field Autotherm prints on its paperwork, or
    /// every historical order is unfindable by the number on the document.
    pub order_number_field: Option<String>,
}

/// `orders` columns the mapping may source. Anything else in `order_fields` is a typo.
pub const ORDER_FIELD_KEYS: &[&str] = &[
    "vehicle_make",
    "vehicle_model",
    "vehicle_plate",
    "vehicle_vin",
    "description",
    "due_date",
    "project_type",
    "assignee",
];

/// `partners` columns the mapping may source.
pub const PARTNER_FIELD_KEYS: &[&str] = &[
    "tax_number",
    "eu_tax_number",
    "country",
    "default_currency",
    "postal_code",
    "city",
    "address_line",
    "notes",
];

/// Reads a dotted path out of a JSON object. `$.` and `$` prefixes are tolerated so a
/// path copied out of the manifest summary works unchanged.
pub fn value_at<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let path = path.trim_start_matches("$.").trim_start_matches('$');
    let mut cur = v;
    for segment in path.split('.').filter(|s| !s.is_empty()) {
        cur = cur.get(segment)?;
    }
    Some(cur)
}

/// Trimmed non-empty text at a dotted path.
pub fn text_at(v: &Value, path: &str) -> Option<String> {
    match value_at(v, path)? {
        Value::String(s) => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
    .filter(|s| !s.is_empty())
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
        for key in mapping.order_fields.keys() {
            if !ORDER_FIELD_KEYS.contains(&key.as_str()) {
                bail!(
                    "order_fields: unknown destination '{key}'; expected one of {}",
                    ORDER_FIELD_KEYS.join(", ")
                );
            }
        }
        for key in mapping.partner_fields.keys() {
            if !PARTNER_FIELD_KEYS.contains(&key.as_str()) {
                bail!(
                    "partner_fields: unknown destination '{key}'; expected one of {}",
                    PARTNER_FIELD_KEYS.join(", ")
                );
            }
        }
        if let Some(pattern) = &mapping.vehicle_plate_from_title {
            regex::Regex::new(pattern)
                .with_context(|| format!("vehicle_plate_from_title is not a regex: {pattern}"))?;
        }
        Ok(mapping)
    }

    fn order_field(&self, key: &str) -> Option<&str> {
        self.order_fields.get(key).map(String::as_str)
    }

    fn partner_field(&self, key: &str) -> Option<&str> {
        self.partner_fields.get(key).map(String::as_str)
    }

    /// The plate: the configured field, else the project name via `vehicle_plate_from_title`.
    fn plate(&self, project: &Value, title: &str) -> Option<String> {
        if let Some(path) = self.order_field("vehicle_plate")
            && let Some(value) = text_at(project, path)
        {
            return Some(value.to_uppercase());
        }
        let pattern = self.vehicle_plate_from_title.as_deref()?;
        let re = regex::Regex::new(pattern).ok()?;
        let captures = re.captures(title)?;
        let matched = captures.get(1).or_else(|| captures.get(0))?;
        Some(matched.as_str().trim().to_uppercase())
    }

    /// The customer-facing order number (V7.1).
    fn order_number(&self, project: &Value) -> Option<String> {
        let path = self.order_number_field.as_deref()?;
        text_at(project, path)
    }

    fn country(&self, raw: &str) -> Option<String> {
        if let Some(mapped) = self.countries.get(raw) {
            return Some(mapped.to_uppercase());
        }
        let trimmed = raw.trim();
        (trimmed.len() == 2 && trimmed.chars().all(|c| c.is_ascii_alphabetic()))
            .then(|| trimmed.to_uppercase())
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
    pub dry_run: bool,
    pub partners: usize,
    pub contacts: usize,
    pub orders: usize,
    pub leads: usize,
    pub notes: usize,
    /// Distinct vehicles created or matched from the four order text columns (V2.1).
    pub vehicles: usize,
    pub skipped_projects: usize,
    pub images: usize,
    pub documents: usize,
    pub files_without_project: usize,
    pub files_not_on_order: usize,
    pub files_not_fetched: usize,
    /// Destination column → how many source records carry a value for it (V7.2, V1.6).
    /// A column at 0 is the signal the mapping is missing a path.
    pub order_field_coverage: BTreeMap<String, usize>,
    pub partner_field_coverage: BTreeMap<String, usize>,
    pub problems: Vec<String>,
}

impl LoadSummary {
    fn problem(&mut self, message: String) {
        tracing::warn!("{message}");
        if self.problems.len() < 1000 {
            self.problems.push(message);
        }
    }

    fn covered(map: &mut BTreeMap<String, usize>, column: &str, present: bool) {
        let entry = map.entry(column.to_string()).or_default();
        if present {
            *entry += 1;
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
            // A wall-clock time inside the spring-forward gap (02:00–03:00 on the last
            // Sunday of March in Budapest) does not exist; read it as the clock after
            // the jump rather than dropping a real record's timestamp.
            return tz
                .from_local_datetime(&naive)
                .earliest()
                .or_else(|| {
                    tz.from_local_datetime(&(naive + chrono::TimeDelta::hours(1)))
                        .earliest()
                })
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

/// Everything the load resolves for one order before any SQL runs. Building it separately
/// is what makes `--dry-run` truthful: the dry run reports exactly the values the real run
/// would write, because it is the same function.
#[derive(Debug, Default)]
struct OrderImport {
    project_type_id: Option<i64>,
    assigned_to: Option<i64>,
    vehicle_make: Option<String>,
    vehicle_model: Option<String>,
    vehicle_plate: Option<String>,
    vehicle_vin: Option<String>,
    description: Option<String>,
    due_date: Option<NaiveDate>,
}

/// Everything the load resolves for one partner before any SQL runs.
#[derive(Debug, Default)]
struct PartnerImport {
    tax_number: Option<String>,
    eu_tax_number: Option<String>,
    country: Option<String>,
    default_currency: Option<String>,
    postal_code: Option<String>,
    city: Option<String>,
    address_line: Option<String>,
    notes: Option<String>,
}

/// Resolved AutoCRM ids for the mapping's project-type and assignee targets.
pub struct Lookups {
    project_types: HashMap<String, i64>,
    assignees: HashMap<String, i64>,
}

/// Every mapped project type and assignee must exist before the load starts, and every
/// source value must be mapped. Both hard-fail, listing everything wrong at once — the
/// alternative is a silent default, which is how `volume_by_project_type` came to bucket
/// 1000 historical orders as "(nincs megadva)".
pub async fn check_coverage(
    state: &AppState,
    mapping: &Mapping,
    projects: &[Value],
) -> anyhow::Result<Lookups> {
    let mut project_types = HashMap::new();
    for (source, key) in &mapping.project_types {
        let id = sqlx::query_scalar!("SELECT id FROM project_types WHERE key = $1", key)
            .fetch_optional(&state.db)
            .await?;
        match id {
            Some(id) => {
                project_types.insert(source.clone(), id);
            }
            None => bail!(
                "mapping project_types['{source}'] = '{key}': no such project type. \
                 Create it first (POST /project-types) or correct the mapping."
            ),
        }
    }
    let mut assignees = HashMap::new();
    for (source, email) in &mapping.assignees {
        let id = sqlx::query_scalar!(
            "SELECT id FROM users WHERE email = $1",
            email.to_lowercase()
        )
        .fetch_optional(&state.db)
        .await?;
        match id {
            Some(id) => {
                assignees.insert(source.clone(), id);
            }
            None => bail!(
                "mapping assignees['{source}'] = '{email}': no such user. \
                 Create the account first or correct the mapping."
            ),
        }
    }

    // Source values with no mapping entry.
    let mut unmapped_types: BTreeMap<String, usize> = BTreeMap::new();
    let mut unmapped_assignees: BTreeMap<String, usize> = BTreeMap::new();
    let mut numbers: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for p in projects {
        let Some(id) = p.get("Id").and_then(as_i64) else {
            continue;
        };
        if p.get("Deleted").and_then(as_i64).is_some_and(|d| d != 0) {
            continue;
        }
        if mapping.entity_for(p.get("CategoryId").and_then(as_i64)) != Some("order") {
            continue;
        }
        if let Some(path) = mapping.order_field("project_type")
            && let Some(value) = text_at(p, path)
            && !mapping.project_types.contains_key(&value)
        {
            *unmapped_types.entry(value).or_default() += 1;
        }
        if let Some(path) = mapping.order_field("assignee")
            && let Some(value) = text_at(p, path)
            && !mapping.assignees.contains_key(&value)
        {
            *unmapped_assignees.entry(value).or_default() += 1;
        }
        if let Some(number) = mapping.order_number(p) {
            numbers.entry(number).or_default().push(id);
        }
    }
    let listing = |what: &str, values: &BTreeMap<String, usize>| {
        let lines: Vec<String> = values
            .iter()
            .map(|(v, n)| format!("  '{v}' ({n} orders)"))
            .collect();
        format!(
            "{} MiniCRM {what} values are not in the mapping:\n{}",
            values.len(),
            lines.join("\n")
        )
    };
    if !unmapped_types.is_empty() {
        bail!("{}", listing("project type", &unmapped_types));
    }
    if !unmapped_assignees.is_empty() {
        bail!("{}", listing("assignee", &unmapped_assignees));
    }
    let duplicates: Vec<String> = numbers
        .iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|(number, ids)| format!("  '{number}' ← projects {ids:?}"))
        .collect();
    if !duplicates.is_empty() {
        bail!(
            "order_number_field '{}' is not unique across {} numbers:\n{}",
            mapping.order_number_field.as_deref().unwrap_or(""),
            duplicates.len(),
            duplicates.join("\n")
        );
    }
    Ok(Lookups {
        project_types,
        assignees,
    })
}

pub async fn run(
    state: &AppState,
    mapping: &Mapping,
    raw_dir: &Path,
    data_dir: &Path,
    dry_run: bool,
) -> anyhow::Result<LoadSummary> {
    let mut summary = LoadSummary {
        dry_run,
        ..LoadSummary::default()
    };

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

    let projects = read_dir_json(&raw_dir.join("projects"))?;
    let lookups = check_coverage(state, mapping, &projects).await?;

    load_contacts(state, mapping, raw_dir, &mut summary, dry_run).await?;
    load_projects(state, mapping, &lookups, &projects, &mut summary, dry_run).await?;
    if dry_run {
        tracing::info!("dry run: no rows written, no files attached");
        return Ok(summary);
    }
    load_notes(state, raw_dir, &mut summary).await?;
    load_files(state, mapping, data_dir, &mut summary).await?;
    Ok(summary)
}

/// The MiniCRM address record for a contact, if `extract --addresses` downloaded one.
/// `Api/R3/AddressList/{cid}` returns either `{"<id>": {...}}` or a bare array; a contact
/// can have several (postal, billing), and the first with a usable city wins.
pub fn address_for(addresses: &HashMap<i64, Value>, contact_id: i64) -> Option<&Value> {
    let list = addresses.get(&contact_id)?;
    let candidates: Vec<&Value> = match list {
        Value::Object(map) => map.values().collect(),
        Value::Array(items) => items.iter().collect(),
        _ => return None,
    };
    candidates
        .iter()
        .find(|a| text(a, "City").is_some() || text(a, "PostalCode").is_some())
        .or(candidates.first())
        .copied()
}

/// Reads `raw/addresses/{id}.json` into a map keyed by contact id.
pub fn read_addresses(raw_dir: &Path) -> anyhow::Result<HashMap<i64, Value>> {
    let dir = raw_dir.join("addresses");
    if !dir.exists() {
        return Ok(HashMap::new());
    }
    let mut out = HashMap::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "json")
            && let Some(id) = path
                .file_stem()
                .and_then(|s| s.to_str()?.parse::<i64>().ok())
        {
            let text = std::fs::read_to_string(&path)?;
            out.insert(
                id,
                serde_json::from_str(&text)
                    .with_context(|| format!("parsing {}", path.display()))?,
            );
        }
    }
    Ok(out)
}

/// Resolves every mapped partner column. `address` is the AddressList record, consulted
/// for postal code / city / address line when the contact record itself has none — those
/// files were being downloaded and then never opened (V1.2).
fn partner_import(
    mapping: &Mapping,
    contact: &Value,
    address: Option<&Value>,
    summary: &mut LoadSummary,
    contact_id: i64,
) -> PartnerImport {
    let field = |key: &str, fallback: &str| -> Option<String> {
        mapping
            .partner_field(key)
            .and_then(|path| text_at(contact, path))
            .or_else(|| text(contact, fallback))
            .or_else(|| address.and_then(|a| text(a, fallback)))
    };
    let raw_country = field("country", "CountryId").or_else(|| text(contact, "Country"));
    let country = raw_country.as_ref().and_then(|c| mapping.country(c));
    if let Some(raw) = &raw_country
        && country.is_none()
    {
        summary.problem(format!(
            "contact {contact_id}: country '{raw}' is not a two-letter code and is not in \
             the mapping's `countries`; left unset"
        ));
    }
    let raw_currency = field("default_currency", "Penznem");
    let default_currency = raw_currency
        .as_ref()
        .map(|c| c.trim().to_uppercase())
        .filter(|c| matches!(c.as_str(), "HUF" | "EUR"));
    if let Some(raw) = &raw_currency
        && default_currency.is_none()
    {
        summary.problem(format!(
            "contact {contact_id}: currency '{raw}' is neither HUF nor EUR; left at the default"
        ));
    }
    PartnerImport {
        tax_number: field("tax_number", "VatNumber"),
        eu_tax_number: field("eu_tax_number", "EuVatNumber"),
        country,
        default_currency,
        postal_code: field("postal_code", "PostalCode"),
        city: field("city", "City"),
        address_line: field("address_line", "Address"),
        notes: field("notes", "Comment"),
    }
}

async fn load_contacts(
    state: &AppState,
    mapping: &Mapping,
    raw_dir: &Path,
    summary: &mut LoadSummary,
    dry_run: bool,
) -> anyhow::Result<()> {
    let contacts = read_dir_json(&raw_dir.join("contacts"))?;
    let addresses = read_addresses(raw_dir)?;

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
        let p = partner_import(mapping, c, address_for(&addresses, id), summary, id);
        let cov = &mut summary.partner_field_coverage;
        LoadSummary::covered(cov, "tax_number", p.tax_number.is_some());
        LoadSummary::covered(cov, "eu_tax_number", p.eu_tax_number.is_some());
        LoadSummary::covered(cov, "country", p.country.is_some());
        LoadSummary::covered(cov, "default_currency", p.default_currency.is_some());
        LoadSummary::covered(cov, "postal_code", p.postal_code.is_some());
        LoadSummary::covered(cov, "city", p.city.is_some());
        LoadSummary::covered(cov, "address_line", p.address_line.is_some());
        LoadSummary::covered(cov, "notes", p.notes.is_some());
        summary.partners += 1;
        if dry_run {
            continue;
        }
        // coalesce on update: a re-run must not blank a column staff have since filled in.
        sqlx::query_scalar!(
            "INSERT INTO partners (kind, name, email, phone, website, tax_number, eu_tax_number,
                                   country, default_currency, postal_code, city, address_line, notes,
                                   minicrm_id, raw_import)
             VALUES ($1, $2, $3, $4, $5, $6, $7, coalesce($8, 'HU'), coalesce($9, 'HUF'), $10, $11, $12, $13, $14, $15)
             ON CONFLICT (minicrm_id) DO UPDATE
             SET kind = EXCLUDED.kind, name = EXCLUDED.name, email = EXCLUDED.email, phone = EXCLUDED.phone,
                 website = EXCLUDED.website, tax_number = EXCLUDED.tax_number,
                 eu_tax_number = EXCLUDED.eu_tax_number, country = EXCLUDED.country,
                 default_currency = EXCLUDED.default_currency, postal_code = EXCLUDED.postal_code,
                 city = EXCLUDED.city, address_line = EXCLUDED.address_line, notes = EXCLUDED.notes,
                 raw_import = EXCLUDED.raw_import
             RETURNING id",
            kind as PartnerKind,
            display_name(c, id),
            email,
            text(c, "Phone"),
            text(c, "Url"),
            p.tax_number,
            p.eu_tax_number,
            p.country,
            p.default_currency,
            p.postal_code,
            p.city,
            p.address_line,
            p.notes,
            id,
            c
        )
        .fetch_one(&state.db)
        .await?;
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
        if dry_run {
            summary.contacts += 1;
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

/// Resolves every mapped order column. Project type and assignee are looked up against
/// `Lookups`, which `check_coverage` has already proved complete, so an unmapped value
/// cannot reach here.
fn order_import(
    mapping: &Mapping,
    lookups: &Lookups,
    project: &Value,
    title: &str,
    tz: Tz,
) -> OrderImport {
    let field = |key: &str| -> Option<String> {
        mapping
            .order_field(key)
            .and_then(|path| text_at(project, path))
    };
    OrderImport {
        project_type_id: field("project_type").and_then(|v| lookups.project_types.get(&v).copied()),
        assigned_to: field("assignee").and_then(|v| lookups.assignees.get(&v).copied()),
        vehicle_make: field("vehicle_make"),
        vehicle_model: field("vehicle_model"),
        vehicle_plate: mapping.plate(project, title),
        vehicle_vin: field("vehicle_vin").map(|v| v.to_uppercase()),
        description: field("description"),
        due_date: field("due_date")
            .and_then(|s| parse_timestamp(&s, tz))
            .map(|t| t.with_timezone(&tz).date_naive()),
    }
}

async fn load_projects(
    state: &AppState,
    mapping: &Mapping,
    lookups: &Lookups,
    projects: &[Value],
    summary: &mut LoadSummary,
    dry_run: bool,
) -> anyhow::Result<()> {
    let tz = state.config.business_tz;
    for p in projects {
        let p = p.clone();
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
            // Resolved before the party check so a dry run reports coverage for every
            // order, including ones a real load would skip for a missing contact.
            let o = order_import(mapping, lookups, &p, &title, tz);
            let cov = &mut summary.order_field_coverage;
            LoadSummary::covered(cov, "project_type_id", o.project_type_id.is_some());
            LoadSummary::covered(cov, "assigned_to", o.assigned_to.is_some());
            LoadSummary::covered(cov, "vehicle_make", o.vehicle_make.is_some());
            LoadSummary::covered(cov, "vehicle_model", o.vehicle_model.is_some());
            LoadSummary::covered(cov, "vehicle_plate", o.vehicle_plate.is_some());
            LoadSummary::covered(cov, "vehicle_vin", o.vehicle_vin.is_some());
            LoadSummary::covered(cov, "description", o.description.is_some());
            LoadSummary::covered(cov, "due_date", o.due_date.is_some());
            summary.orders += 1;
            if dry_run {
                continue;
            }

            let Some((partner_id, contact_id, _)) = party else {
                summary.problem(format!(
                    "project {id}: contact {:?} not loaded; order skipped",
                    p.get("ContactId")
                ));
                summary.orders -= 1;
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

            // TODO(client): confirm whether Autotherm prints the MiniCRM project id or its
            // own number. `order_number_field` in the mapping switches between the two;
            // without it every historical order is findable only by `MC-{id}`.
            let number = match mapping.order_number(&p) {
                Some(number) => number,
                None => {
                    if mapping.order_number_field.is_some() {
                        summary.problem(format!(
                            "project {id}: order_number_field is empty; numbered MC-{id}"
                        ));
                    }
                    format!("MC-{id}")
                }
            };

            let order_id = sqlx::query_scalar!(
                "INSERT INTO orders (number, title, partner_id, contact_id, project_type_id, currency,
                                     valuation_date, vehicle_make, vehicle_model, vehicle_plate, vehicle_vin,
                                     description, due_date, assigned_to, minicrm_id, raw_import, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, coalesce($17, now()))
                 ON CONFLICT (minicrm_id) DO UPDATE
                 SET number = EXCLUDED.number, title = EXCLUDED.title, partner_id = EXCLUDED.partner_id,
                     contact_id = EXCLUDED.contact_id, project_type_id = EXCLUDED.project_type_id,
                     valuation_date = EXCLUDED.valuation_date, vehicle_make = EXCLUDED.vehicle_make,
                     vehicle_model = EXCLUDED.vehicle_model, vehicle_plate = EXCLUDED.vehicle_plate,
                     vehicle_vin = EXCLUDED.vehicle_vin, description = EXCLUDED.description,
                     due_date = EXCLUDED.due_date, assigned_to = EXCLUDED.assigned_to,
                     raw_import = EXCLUDED.raw_import
                 RETURNING id",
                number,
                title,
                partner_id,
                contact_id,
                o.project_type_id,
                currency,
                valuation_date,
                o.vehicle_make,
                o.vehicle_model,
                o.vehicle_plate,
                o.vehicle_vin,
                o.description,
                o.due_date,
                o.assigned_to,
                id,
                &p,
                created_at
            )
            .fetch_one(&state.db)
            .await?;

            // V2.1: both. The four text columns are the fallback if deduplicating plates
            // typed by hand over thirty years turns out wrong; the vehicle row is what
            // plate search and the warranty question actually use.
            let mut conn = state.db.acquire().await?;
            if let Some(vehicle) = vehicles::upsert(
                &mut conn,
                &vehicles::VehicleFields {
                    vin: o.vehicle_vin.clone(),
                    plate: o.vehicle_plate.clone(),
                    make: o.vehicle_make.clone(),
                    model: o.vehicle_model.clone(),
                    year: None,
                    partner_id: Some(partner_id),
                    notes: None,
                },
            )
            .await?
            {
                vehicles::attach(&mut *conn, order_id, vehicle.id).await?;
                summary.vehicles += 1;
            }
            drop(conn);

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
            summary.leads += 1;
            if dry_run {
                continue;
            }
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
        }
    }
    Ok(())
}

/// MiniCRM to-do entries → `order_notes` (V1.3).
///
/// `raw/todos/{projectId}.json` was being downloaded and then read by nothing. For a
/// company that has run its project management in MiniCRM for years this is the largest
/// body of institutional record in the account: who said what, when, and what was agreed.
/// It is loaded as an append-only activity log shown on the order's Napló tab — AutoCRM
/// has no task feature and is not gaining one, so nothing here is completable.
///
/// Field names differ per account, so several plausible keys are tried for each of body,
/// author and date; anything unrecognised still lands in the row's own `raw_import`.
async fn load_notes(
    state: &AppState,
    raw_dir: &Path,
    summary: &mut LoadSummary,
) -> anyhow::Result<()> {
    let dir = raw_dir.join("todos");
    if !dir.exists() {
        summary.problem(
            "no raw/todos directory: run `autocrm-migrate extract` (to-dos are on by default) \
             or the order history tab will be empty for every migrated order"
                .into(),
        );
        return Ok(());
    }
    let tz = state.config.business_tz;
    let orders: HashMap<i64, i64> = sqlx::query!(
        r#"SELECT id, minicrm_id AS "minicrm_id!" FROM orders WHERE minicrm_id IS NOT NULL"#
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|r| (r.minicrm_id, r.id))
    .collect();

    let mut files: Vec<_> = std::fs::read_dir(&dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();

    for path in files {
        let Some(project) = path
            .file_stem()
            .and_then(|s| s.to_str()?.parse::<i64>().ok())
        else {
            continue;
        };
        let Some(&order_id) = orders.get(&project) else {
            // Leads and skipped categories have to-dos too; they have nowhere to go.
            continue;
        };
        let value: Value = serde_json::from_str(&std::fs::read_to_string(&path)?)
            .with_context(|| format!("parsing {}", path.display()))?;
        for todo in todo_entries(&value) {
            let Some(id) = todo.get("Id").and_then(as_i64) else {
                continue;
            };
            let body = ["Comment", "Description", "Body", "Note", "Name", "Subject"]
                .iter()
                .filter_map(|k| text(todo, k))
                .collect::<Vec<_>>()
                .join("\n\n");
            if body.is_empty() {
                summary.problem(format!(
                    "to-do {id} on project {project}: no recognisable text; kept in raw_import only"
                ));
                continue;
            }
            let author = ["UserName", "User", "CreatedByName", "Owner"]
                .iter()
                .find_map(|k| text(todo, k));
            let occurred_at = ["CreatedAt", "Deadline", "UpdatedAt", "Date"]
                .iter()
                .find_map(|k| text(todo, k))
                .and_then(|s| parse_timestamp(&s, tz))
                .unwrap_or_else(Utc::now);
            sqlx::query!(
                "INSERT INTO order_notes (order_id, minicrm_id, author_name, body, occurred_at, raw_import)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (minicrm_id) DO UPDATE
                 SET order_id = EXCLUDED.order_id, author_name = EXCLUDED.author_name,
                     body = EXCLUDED.body, occurred_at = EXCLUDED.occurred_at,
                     raw_import = EXCLUDED.raw_import",
                order_id,
                id,
                author,
                body,
                occurred_at,
                todo
            )
            .execute(&state.db)
            .await?;
            summary.notes += 1;
        }
    }
    Ok(())
}

/// ToDoList comes back as `{"Results": {...}}`, a bare id-keyed object, or an array.
fn todo_entries(v: &Value) -> Vec<&Value> {
    let body = v.get("Results").unwrap_or(v);
    match body {
        Value::Object(map) => map.values().filter(|e| e.is_object()).collect(),
        Value::Array(items) => items.iter().filter(|e| e.is_object()).collect(),
        _ => Vec::new(),
    }
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
                    owner: documents::Owner::Order(order_id),
                    vehicle_id: None,
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

/// What the loader would write for one project, column → value as text (V1.6).
///
/// Reconciliation compares this against the row that is actually in the database. It calls
/// the same `order_import` the loader does, so the comparison cannot drift from the load:
/// if the mapping stops finding a plate, both change together and the diff still shows it.
pub fn expected_order_fields(
    mapping: &Mapping,
    lookups: &Lookups,
    project: &Value,
    tz: Tz,
) -> BTreeMap<&'static str, Option<String>> {
    let id = project.get("Id").and_then(as_i64).unwrap_or_default();
    let title = display_name(project, id);
    let o = order_import(mapping, lookups, project, &title, tz);
    BTreeMap::from([
        ("title", Some(title)),
        (
            "number",
            Some(
                mapping
                    .order_number(project)
                    .unwrap_or_else(|| format!("MC-{id}")),
            ),
        ),
        ("vehicle_make", o.vehicle_make),
        ("vehicle_model", o.vehicle_model),
        ("vehicle_plate", o.vehicle_plate),
        ("vehicle_vin", o.vehicle_vin),
        ("description", o.description),
        ("due_date", o.due_date.map(|d| d.to_string())),
        ("project_type_id", o.project_type_id.map(|v| v.to_string())),
        ("assigned_to", o.assigned_to.map(|v| v.to_string())),
    ])
}

/// What the loader would write for one contact, column → value as text (V1.6).
pub fn expected_partner_fields(
    mapping: &Mapping,
    contact: &Value,
    address: Option<&Value>,
) -> BTreeMap<&'static str, Option<String>> {
    let id = contact.get("Id").and_then(as_i64).unwrap_or_default();
    let mut discard = LoadSummary::default();
    let p = partner_import(mapping, contact, address, &mut discard, id);
    BTreeMap::from([
        ("name", Some(display_name(contact, id))),
        (
            "email",
            text(contact, "Email")
                .as_deref()
                .and_then(normalize_address),
        ),
        ("phone", text(contact, "Phone")),
        ("website", text(contact, "Url")),
        ("tax_number", p.tax_number),
        ("eu_tax_number", p.eu_tax_number),
        ("country", p.country.or_else(|| Some("HU".into()))),
        (
            "default_currency",
            p.default_currency.or_else(|| Some("HUF".into())),
        ),
        ("postal_code", p.postal_code),
        ("city", p.city),
        ("address_line", p.address_line),
        ("notes", p.notes),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn d(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    #[test]
    fn dotted_paths_reach_nested_fields() {
        let v = json!({"Jarmu": {"Rendszam": " abc-123 "}, "Top": 7});
        assert_eq!(text_at(&v, "Jarmu.Rendszam").as_deref(), Some("abc-123"));
        assert_eq!(text_at(&v, "$.Jarmu.Rendszam").as_deref(), Some("abc-123"));
        assert_eq!(text_at(&v, "Top").as_deref(), Some("7"));
        assert_eq!(text_at(&v, "Jarmu.Nincs"), None);
    }

    #[test]
    fn the_plate_comes_from_the_field_or_the_title() {
        let with_field: Mapping = serde_json::from_value(json!({
            "categories": {"1": "order"},
            "default_order_stage": "completed",
            "default_lead_stage": "lost",
            "order_fields": {"vehicle_plate": "Rendszam"},
            "vehicle_plate_from_title": "[A-Z]{3}-[0-9]{3}"
        }))
        .unwrap();
        let project = json!({"Rendszam": "xyz-987"});
        assert_eq!(
            with_field.plate(&project, "Iveco (ABC-123)").as_deref(),
            Some("XYZ-987"),
            "a structured field wins"
        );
        assert_eq!(
            with_field.plate(&json!({}), "Iveco (ABC-123)").as_deref(),
            Some("ABC-123"),
            "without one, the project name is the fallback"
        );
        assert_eq!(with_field.plate(&json!({}), "Iveco"), None);
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
    fn timestamps_in_the_spring_forward_gap_still_parse_to_that_day() {
        // 2019-03-31 02:00–03:00 does not exist in Budapest (clocks jump to 03:00).
        // A MiniCRM CreatedAt inside that hour is still a real record from that day;
        // dropping it would set the valuation date to "today at load time".
        let tz = chrono_tz::Europe::Budapest;
        let parsed = parse_timestamp("2019-03-31 02:30:00", tz).expect("gap time must parse");
        assert_eq!(
            parsed.with_timezone(&tz).date_naive(),
            NaiveDate::from_ymd_opt(2019, 3, 31).unwrap()
        );
        assert_eq!(parsed.to_rfc3339(), "2019-03-31T01:30:00+00:00");
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

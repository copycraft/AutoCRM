//! Phase M4: compare source and destination and write a report a human signs off.
//! Structural integrity can be verified here; whether the data is *right* only Autotherm
//! staff can confirm.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

use serde_json::Value;

use super::extract::as_i64;
use super::fetch::{FailedEntry, FetchedEntry, read_jsonl};
use super::load::{self, Mapping, text};
use super::manifest::ManifestEntry;
use crate::AppState;

fn read_all(dir: &Path) -> anyhow::Result<Vec<Value>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "json") {
            out.push(serde_json::from_str(&std::fs::read_to_string(&path)?)?);
        }
    }
    Ok(out)
}

fn line(report: &mut String, label: &str, source: usize, dest: i64) {
    let mark = if source as i64 == dest {
        "✅"
    } else {
        "⚠️"
    };
    let _ = writeln!(report, "| {label} | {source} | {dest} | {mark} |");
}

pub async fn run(
    state: &AppState,
    mapping: &Mapping,
    raw_dir: &Path,
    data_dir: &Path,
) -> anyhow::Result<String> {
    let contacts = read_all(&raw_dir.join("contacts"))?;
    let projects = read_all(&raw_dir.join("projects"))?;

    let is_business =
        |c: &Value| text(c, "Type").is_some_and(|t| t.eq_ignore_ascii_case("business"));
    let has_business = |c: &Value| {
        let id = c.get("Id").and_then(as_i64);
        c.get("BusinessId")
            .and_then(as_i64)
            .is_some_and(|b| b > 0 && Some(b) != id)
    };
    let src_partners = contacts
        .iter()
        .filter(|c| is_business(c) || !has_business(c))
        .count();
    let src_contacts = contacts
        .iter()
        .filter(|c| !is_business(c) && has_business(c))
        .count();

    let live = |p: &&Value| !p.get("Deleted").and_then(as_i64).is_some_and(|d| d != 0);
    let entity = |p: &Value| {
        mapping
            .entity_for(p.get("CategoryId").and_then(as_i64))
            .unwrap_or("unmapped")
            .to_string()
    };
    let src_orders: Vec<i64> = projects
        .iter()
        .filter(live)
        .filter(|p| entity(p) == "order")
        .filter_map(|p| p.get("Id").and_then(as_i64))
        .collect();
    let src_leads = projects
        .iter()
        .filter(live)
        .filter(|p| entity(p) == "lead")
        .count();
    let unmapped = projects.iter().filter(|p| entity(p) == "unmapped").count();

    let db = &state.db;
    let dst_partners = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM partners WHERE minicrm_id IS NOT NULL"#
    )
    .fetch_one(db)
    .await?;
    let dst_contacts = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM contacts WHERE minicrm_id IS NOT NULL"#
    )
    .fetch_one(db)
    .await?;
    let dst_order_ids: HashSet<i64> = sqlx::query_scalar!(
        r#"SELECT minicrm_id AS "m!" FROM orders WHERE minicrm_id IS NOT NULL"#
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .collect();
    let dst_leads =
        sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM leads WHERE minicrm_id IS NOT NULL"#)
            .fetch_one(db)
            .await?;

    let mut report = String::new();
    let _ = writeln!(
        report,
        "# MiniCRM migration reconciliation\n\nGenerated {}\n",
        chrono::Utc::now().to_rfc3339()
    );
    let _ = writeln!(
        report,
        "## Records\n\n| Entity | MiniCRM | AutoCRM | |\n|---|---:|---:|---|"
    );
    line(
        &mut report,
        "Partners (businesses + standalone people)",
        src_partners,
        dst_partners,
    );
    line(
        &mut report,
        "Contacts (people at a business)",
        src_contacts,
        dst_contacts,
    );
    line(
        &mut report,
        "Orders",
        src_orders.len(),
        dst_order_ids.len() as i64,
    );
    line(&mut report, "Leads", src_leads, dst_leads);
    let _ = writeln!(report, "\nProjects in unmapped categories: **{unmapped}**");

    let missing: Vec<i64> = src_orders
        .iter()
        .copied()
        .filter(|id| !dst_order_ids.contains(id))
        .collect();
    if !missing.is_empty() {
        let _ = writeln!(
            report,
            "\n### Orders missing in AutoCRM ({})\n",
            missing.len()
        );
        for id in missing.iter().take(100) {
            let _ = writeln!(report, "- MiniCRM project {id}");
        }
    }

    // Files
    let manifest: Vec<ManifestEntry> = read_jsonl(&data_dir.join("manifest.jsonl")).await?;
    let fetched: Vec<FetchedEntry> = read_jsonl(&data_dir.join("fetched.jsonl")).await?;
    let failed: Vec<FailedEntry> = read_jsonl(&data_dir.join("failed.jsonl")).await?;
    let fetched_urls: HashSet<&str> = fetched.iter().map(|f| f.source_url.as_str()).collect();
    let unique_urls: HashSet<&str> = manifest.iter().map(|m| m.source_url.as_str()).collect();
    let still_failing: HashSet<&str> = failed
        .iter()
        .map(|f| f.source_url.as_str())
        .filter(|u| !fetched_urls.contains(u))
        .collect();
    let unique_hashes: HashSet<&str> = fetched.iter().map(|f| f.sha256.as_str()).collect();
    let total_bytes: i64 = fetched.iter().map(|f| f.byte_size).sum();

    let _ = writeln!(report, "\n## Files\n");
    let _ = writeln!(
        report,
        "- Manifest references: {} ({} unique URLs)",
        manifest.len(),
        unique_urls.len()
    );
    let _ = writeln!(
        report,
        "- Downloaded: {} ({:.1} GiB, {} unique contents after dedup)",
        fetched_urls.len(),
        total_bytes as f64 / 1_073_741_824.0,
        unique_hashes.len()
    );
    let _ = writeln!(
        report,
        "- Never downloaded: {}",
        unique_urls
            .iter()
            .filter(|u| !fetched_urls.contains(*u))
            .count()
    );
    let _ = writeln!(report, "- Currently failing: {}", still_failing.len());

    // Per-order file counts: manifest vs rows linked by source_ref.
    let mut expected: BTreeMap<i64, usize> = BTreeMap::new();
    for m in &manifest {
        if let Some(p) = m.minicrm_project_id.filter(|p| dst_order_ids.contains(p)) {
            *expected.entry(p).or_default() += 1;
        }
    }
    let actual: HashMap<i64, i64> = sqlx::query!(
        r#"SELECT o.minicrm_id AS "minicrm_id!",
                  (SELECT count(*) FROM images i WHERE i.order_id = o.id AND i.source_ref IS NOT NULL AND i.deleted_at IS NULL)
                + (SELECT count(*) FROM documents d WHERE d.order_id = o.id AND d.source_ref IS NOT NULL AND d.deleted_at IS NULL) AS "files!"
           FROM orders o WHERE o.minicrm_id IS NOT NULL"#
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.minicrm_id, r.files))
    .collect();
    let mismatches: Vec<(i64, usize, i64)> = expected
        .iter()
        .map(|(p, n)| (*p, *n, actual.get(p).copied().unwrap_or(0)))
        .filter(|(_, n, a)| *n as i64 != *a)
        .collect();
    let _ = writeln!(
        report,
        "- Orders whose attached-file count differs from the manifest: **{}**",
        mismatches.len()
    );
    if !mismatches.is_empty() {
        let _ = writeln!(
            report,
            "\n| MiniCRM project | Manifest | Loaded |\n|---|---:|---:|"
        );
        for (p, n, a) in mismatches.iter().take(100) {
            let _ = writeln!(report, "| {p} | {n} | {a} |");
        }
        let _ = writeln!(
            report,
            "\n(Identical files referenced twice on one project load once; small differences can be expected — check a few by hand.)"
        );
    }

    // Value by year, for comparison with MiniCRM's own totals.
    let values = sqlx::query!(
        r#"SELECT extract(year FROM ov.valuation_date)::int AS "year!", ov.currency AS "currency!",
                  count(*) AS "orders!", coalesce(sum(ov.total_minor), 0)::bigint AS "total_minor!"
           FROM order_values ov JOIN orders o ON o.id = ov.order_id
           WHERE o.minicrm_id IS NOT NULL
           GROUP BY 1, 2 ORDER BY 1, 2"#
    )
    .fetch_all(db)
    .await?;
    let _ = writeln!(
        report,
        "\n## Order value by year\n\n| Year | Currency | Orders | Total |\n|---|---|---:|---:|"
    );
    for v in values {
        let _ = writeln!(
            report,
            "| {} | {} | {} | {:.2} |",
            v.year,
            v.currency,
            v.orders,
            v.total_minor as f64 / 100.0
        );
    }

    let orphans = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM orders o WHERE NOT EXISTS (SELECT 1 FROM order_stages s WHERE s.order_id = o.id)"#
    )
    .fetch_one(db)
    .await?;
    let _ = writeln!(
        report,
        "\n## Integrity\n\n- Orders without stage history: {orphans}"
    );

    // V1.4: say plainly what the stage numbers do and do not cover.
    let single_stage = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM orders o WHERE o.minicrm_id IS NOT NULL
             AND (SELECT count(*) FROM order_stages s WHERE s.order_id = o.id) = 1"#
    )
    .fetch_one(db)
    .await?;
    let _ = writeln!(
        report,
        "- Migrated orders whose entire stage history is the single 'MiniCRM import' row: \
         **{single_stage}**\n\n\
         > **Historical stage durations are not available.** The MiniCRM R3 API exposes a \
         > project's *current* status and the date it was last changed (`StatusUpdatedAt`), \
         > and no per-status change log. Every migrated order therefore enters AutoCRM with \
         > one stage row: its final status, dated when it last changed. Stage-duration and \
         > throughput reports are meaningful only for orders created in AutoCRM — do not \
         > read them as covering the migrated years. See docs/migration/README.md."
    );

    coverage_and_field_sample(state, mapping, raw_dir, &projects, &contacts, &mut report).await?;

    let _ = writeln!(
        report,
        "\n## Sign-off\n\nChecked by Autotherm staff (spot-check at least 20 orders across different years, including their photos):\n\n- Name: ____________________\n- Date: ____________________\n- Result: ☐ approved ☐ needs fixes\n"
    );
    Ok(report)
}

/// Per-column coverage and a field-level sample (V1.6).
///
/// Counting rows proves nothing about content: the previous report printed ✅ on a
/// migration that dropped every vehicle, because it never compared a field. Two checks
/// close that: what share of migrated rows have each column set, and — for a random
/// sample — whether the loaded value equals what the source actually says.
const SAMPLE: i64 = 50;

async fn coverage_and_field_sample(
    state: &AppState,
    mapping: &Mapping,
    raw_dir: &Path,
    projects: &[Value],
    contacts: &[Value],
    report: &mut String,
) -> anyhow::Result<()> {
    let db = &state.db;
    let tz = state.config.business_tz;
    let lookups = load::check_coverage(state, mapping, projects).await?;
    let addresses = load::read_addresses(raw_dir)?;

    let by_id = |records: &[Value]| -> HashMap<i64, Value> {
        records
            .iter()
            .filter_map(|r| Some((r.get("Id").and_then(as_i64)?, r.clone())))
            .collect()
    };
    let projects_by_id = by_id(projects);
    let contacts_by_id = by_id(contacts);

    // ── Coverage ────────────────────────────────────────────────────────────────────
    let orders = sqlx::query!(
        r#"SELECT count(*) AS "total!",
                  count(vehicle_make) AS "vehicle_make!", count(vehicle_model) AS "vehicle_model!",
                  count(vehicle_plate) AS "vehicle_plate!", count(vehicle_vin) AS "vehicle_vin!",
                  count(description) AS "description!", count(due_date) AS "due_date!",
                  count(assigned_to) AS "assigned_to!", count(project_type_id) AS "project_type_id!",
                  count(lead_id) AS "lead_id!"
             FROM orders WHERE minicrm_id IS NOT NULL"#
    )
    .fetch_one(db)
    .await?;
    let partners = sqlx::query!(
        r#"SELECT count(*) AS "total!",
                  count(email) AS "email!", count(phone) AS "phone!", count(website) AS "website!",
                  count(tax_number) AS "tax_number!", count(eu_tax_number) AS "eu_tax_number!",
                  count(postal_code) AS "postal_code!", count(city) AS "city!",
                  count(address_line) AS "address_line!", count(notes) AS "notes!",
                  count(*) FILTER (WHERE country <> 'HU') AS "non_hu_country!"
             FROM partners WHERE minicrm_id IS NOT NULL"#
    )
    .fetch_one(db)
    .await?;
    let notes = sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM order_notes"#)
        .fetch_one(db)
        .await?;

    let _ = writeln!(
        report,
        "\n## Field coverage\n\n\
         What share of migrated rows actually carry each column. **A column at 0% means \
         the mapping has no source for it** — that is the shape of the defect this table \
         exists to catch, not a property of the data.\n\n\
         | Table | Column | Non-null | Coverage |\n|---|---|---:|---:|"
    );
    let mut row = |table: &str, column: &str, n: i64, total: i64| {
        let pct = if total == 0 {
            0.0
        } else {
            n as f64 * 100.0 / total as f64
        };
        let _ = writeln!(report, "| {table} | {column} | {n} | {pct:.1}% |");
    };
    let ot = orders.total;
    row("orders", "vehicle_make", orders.vehicle_make, ot);
    row("orders", "vehicle_model", orders.vehicle_model, ot);
    row("orders", "vehicle_plate", orders.vehicle_plate, ot);
    row("orders", "vehicle_vin", orders.vehicle_vin, ot);
    row("orders", "description", orders.description, ot);
    row("orders", "due_date", orders.due_date, ot);
    row("orders", "assigned_to", orders.assigned_to, ot);
    row("orders", "project_type_id", orders.project_type_id, ot);
    row("orders", "lead_id", orders.lead_id, ot);
    let pt = partners.total;
    row("partners", "email", partners.email, pt);
    row("partners", "phone", partners.phone, pt);
    row("partners", "website", partners.website, pt);
    row("partners", "tax_number", partners.tax_number, pt);
    row("partners", "eu_tax_number", partners.eu_tax_number, pt);
    row("partners", "postal_code", partners.postal_code, pt);
    row("partners", "city", partners.city, pt);
    row("partners", "address_line", partners.address_line, pt);
    row("partners", "notes", partners.notes, pt);
    row("partners", "country (non-HU)", partners.non_hu_country, pt);
    let _ = writeln!(
        report,
        "\nImported activity entries (`order_notes`): **{notes}**{}",
        if notes == 0 {
            " — nothing loaded. Did `extract` run with to-dos enabled?"
        } else {
            ""
        }
    );

    // ── Field-level sample ──────────────────────────────────────────────────────────
    let sampled_orders = sqlx::query!(
        r#"SELECT minicrm_id AS "minicrm_id!", number, title, vehicle_make, vehicle_model,
                  vehicle_plate, vehicle_vin, description, due_date, project_type_id, assigned_to
             FROM orders WHERE minicrm_id IS NOT NULL ORDER BY random() LIMIT $1"#,
        SAMPLE
    )
    .fetch_all(db)
    .await?;
    let mut order_diffs: Vec<String> = Vec::new();
    for o in &sampled_orders {
        let Some(source) = projects_by_id.get(&o.minicrm_id) else {
            order_diffs.push(format!(
                "| {} | — | (source project not in the extract) | — |",
                o.minicrm_id
            ));
            continue;
        };
        let actual: BTreeMap<&str, Option<String>> = BTreeMap::from([
            ("title", Some(o.title.clone())),
            ("number", Some(o.number.clone())),
            ("vehicle_make", o.vehicle_make.clone()),
            ("vehicle_model", o.vehicle_model.clone()),
            ("vehicle_plate", o.vehicle_plate.clone()),
            ("vehicle_vin", o.vehicle_vin.clone()),
            ("description", o.description.clone()),
            ("due_date", o.due_date.map(|d| d.to_string())),
            ("project_type_id", o.project_type_id.map(|v| v.to_string())),
            ("assigned_to", o.assigned_to.map(|v| v.to_string())),
        ]);
        diff_row(
            &load::expected_order_fields(mapping, &lookups, source, tz),
            &actual,
            o.minicrm_id,
            &mut order_diffs,
        );
    }

    let sampled_partners = sqlx::query!(
        r#"SELECT minicrm_id AS "minicrm_id!", name, email, phone, website, tax_number,
                  eu_tax_number, country, default_currency, postal_code, city, address_line, notes
             FROM partners WHERE minicrm_id IS NOT NULL ORDER BY random() LIMIT $1"#,
        SAMPLE
    )
    .fetch_all(db)
    .await?;
    let mut partner_diffs: Vec<String> = Vec::new();
    for p in &sampled_partners {
        let Some(source) = contacts_by_id.get(&p.minicrm_id) else {
            partner_diffs.push(format!(
                "| {} | — | (source contact not in the extract) | — |",
                p.minicrm_id
            ));
            continue;
        };
        let actual: BTreeMap<&str, Option<String>> = BTreeMap::from([
            ("name", Some(p.name.clone())),
            ("email", p.email.clone()),
            ("phone", p.phone.clone()),
            ("website", p.website.clone()),
            ("tax_number", p.tax_number.clone()),
            ("eu_tax_number", p.eu_tax_number.clone()),
            ("country", Some(p.country.clone())),
            ("default_currency", Some(p.default_currency.clone())),
            ("postal_code", p.postal_code.clone()),
            ("city", p.city.clone()),
            ("address_line", p.address_line.clone()),
            ("notes", p.notes.clone()),
        ]);
        diff_row(
            &load::expected_partner_fields(
                mapping,
                source,
                load::address_for(&addresses, p.minicrm_id),
            ),
            &actual,
            p.minicrm_id,
            &mut partner_diffs,
        );
    }

    let _ = writeln!(
        report,
        "\n## Field-level sample\n\n\
         {} orders and {} partners drawn at random, every loaded column compared with the \
         source JSON. A row here is a value that differs, or a null where the source had \
         something.",
        sampled_orders.len(),
        sampled_partners.len()
    );
    for (label, diffs) in [("Orders", &order_diffs), ("Partners", &partner_diffs)] {
        if diffs.is_empty() {
            let _ = writeln!(report, "\n**{label}: no differences.**");
            continue;
        }
        let _ = writeln!(
            report,
            "\n### {label}: {} differences\n\n| MiniCRM id | Column | Source | Loaded |\n|---|---|---|---|",
            diffs.len()
        );
        for line in diffs.iter().take(200) {
            let _ = writeln!(report, "{line}");
        }
        if diffs.len() > 200 {
            let _ = writeln!(report, "\n(… {} more)", diffs.len() - 200);
        }
    }
    Ok(())
}

/// Appends one table row per column whose loaded value is not what the source says.
fn diff_row(
    expected: &BTreeMap<&'static str, Option<String>>,
    actual: &BTreeMap<&str, Option<String>>,
    id: i64,
    out: &mut Vec<String>,
) {
    for (column, want) in expected {
        let got = actual.get(column).cloned().flatten();
        if &got == want {
            continue;
        }
        let show = |v: &Option<String>| match v {
            Some(s) if s.chars().count() > 60 => s.chars().take(57).collect::<String>() + "…",
            Some(s) => s.replace('|', "\\|").replace('\n', " "),
            None => "(null)".into(),
        };
        out.push(format!(
            "| {id} | {column} | {} | {} |",
            show(want),
            show(&got)
        ));
    }
}

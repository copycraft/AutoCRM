//! Phase M4: compare source and destination and write a report a human signs off.
//! Structural integrity can be verified here; whether the data is *right* only Autotherm
//! staff can confirm.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

use serde_json::Value;

use super::extract::as_i64;
use super::fetch::{FailedEntry, FetchedEntry, read_jsonl};
use super::load::{Mapping, text};
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

    let _ = writeln!(
        report,
        "\n## Sign-off\n\nChecked by Autotherm staff (spot-check at least 20 orders across different years, including their photos):\n\n- Name: ____________________\n- Date: ____________________\n- Result: ☐ approved ☐ needs fixes\n"
    );
    Ok(report)
}

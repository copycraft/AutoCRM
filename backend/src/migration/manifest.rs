//! Phase M2a: enumerate every file reference in the extracted data before downloading
//! anything. The summary (counts by field and extension) is what a human compares with the
//! MiniCRM UI. Scanning is structural — any URL-looking string pointing at a file — so it
//! doesn't depend on guessing MiniCRM's field names up front.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::extract::as_i64;

const FILE_EXTENSIONS: [&str; 16] = [
    "jpg", "jpeg", "png", "gif", "webp", "heic", "heif", "tif", "tiff", "bmp", "pdf", "dwg", "dxf",
    "step", "stp", "zip",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub source_url: String,
    pub minicrm_project_id: Option<i64>,
    pub minicrm_contact_id: Option<i64>,
    /// JSON path of the field the URL came from, e.g. `$.Atveteli_kepek[3]`.
    pub field: String,
    pub filename: Option<String>,
    pub extension: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct ManifestSummary {
    pub entries: usize,
    pub by_field: BTreeMap<String, usize>,
    pub by_extension: BTreeMap<String, usize>,
    pub projects_with_files: usize,
}

fn extension_of(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next()?;
    let last = path.rsplit('/').next()?;
    let (_, ext) = last.rsplit_once('.')?;
    let ext = ext.to_ascii_lowercase();
    FILE_EXTENSIONS.contains(&ext.as_str()).then_some(ext)
}

fn filename_of(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next()?;
    path.rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Looks like a downloadable file: an http(s) URL that either has a known file extension or
/// points at a MiniCRM file/download path.
pub fn is_file_url(s: &str) -> bool {
    let lower = s.trim().to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://"))
        && (extension_of(&lower).is_some()
            || lower.contains("/download")
            || lower.contains("/file"))
}

/// Collapse array indices so fields group together: `$.Kepek[3]` → `$.Kepek[]`.
pub fn field_group(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut in_brackets = false;
    for c in path.chars() {
        match c {
            '[' => {
                in_brackets = true;
                out.push('[');
            }
            ']' => {
                in_brackets = false;
                out.push(']');
            }
            _ if in_brackets => {}
            _ => out.push(c),
        }
    }
    out
}

pub fn scan(value: &Value, path: &str, found: &mut Vec<(String, String)>) {
    match value {
        Value::String(s) if is_file_url(s) => found.push((path.to_string(), s.trim().to_string())),
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                scan(item, &format!("{path}[{i}]"), found);
            }
        }
        Value::Object(map) => {
            for (k, v) in map {
                scan(v, &format!("{path}.{k}"), found);
            }
        }
        _ => {}
    }
}

fn entries_for(value: &Value, project: Option<i64>, contact: Option<i64>) -> Vec<ManifestEntry> {
    let mut found = Vec::new();
    scan(value, "$", &mut found);
    found
        .into_iter()
        .map(|(field, url)| ManifestEntry {
            filename: filename_of(&url),
            extension: extension_of(&url),
            source_url: url,
            minicrm_project_id: project,
            minicrm_contact_id: contact,
            field,
        })
        .collect()
}

pub fn build(raw_dir: &Path) -> anyhow::Result<(Vec<ManifestEntry>, ManifestSummary)> {
    let mut entries = Vec::new();
    for (sub, is_project) in [("projects", true), ("contacts", false)] {
        let dir = raw_dir.join(sub);
        if !dir.exists() {
            continue;
        }
        let mut files: Vec<_> = std::fs::read_dir(&dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        files.sort();
        for file in files
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
        {
            let text = std::fs::read_to_string(&file)?;
            let value: Value = serde_json::from_str(&text)
                .with_context(|| format!("parsing {}", file.display()))?;
            let id = value.get("Id").and_then(as_i64).or_else(|| {
                file.file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.parse().ok())
            });
            let (project, contact) = if is_project {
                (id, value.get("ContactId").and_then(as_i64))
            } else {
                (None, id)
            };
            entries.extend(entries_for(&value, project, contact));
        }
    }

    // The same file referenced twice for the same project is one entry.
    let mut seen = BTreeSet::new();
    entries.retain(|e| seen.insert((e.source_url.clone(), e.minicrm_project_id)));

    let mut summary = ManifestSummary {
        entries: entries.len(),
        ..Default::default()
    };
    let mut projects = BTreeSet::new();
    for e in &entries {
        *summary.by_field.entry(field_group(&e.field)).or_default() += 1;
        *summary
            .by_extension
            .entry(e.extension.clone().unwrap_or_else(|| "(none)".into()))
            .or_default() += 1;
        if let Some(p) = e.minicrm_project_id {
            projects.insert(p);
        }
    }
    summary.projects_with_files = projects.len();
    Ok((entries, summary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finds_file_urls_anywhere() {
        let project = json!({
            "Id": 42,
            "Name": "Iveco hűtős",
            "Website": "https://example.hu",
            "Atveteli_kepek": ["https://r3.minicrm.hu/files/42/IMG_001.JPG", "https://r3.minicrm.hu/files/42/IMG_002.jpg?x=1"],
            "Nested": { "Terv": { "Url": "https://cdn.minicrm.hu/download/abc" } }
        });
        let entries = entries_for(&project, Some(42), None);
        let urls: Vec<_> = entries.iter().map(|e| e.source_url.as_str()).collect();
        assert_eq!(urls.len(), 3, "{urls:?}");
        assert!(!urls.contains(&"https://example.hu"));
        assert_eq!(entries[0].field, "$.Atveteli_kepek[0]");
        assert_eq!(entries[0].extension.as_deref(), Some("jpg"));
        assert_eq!(entries[1].filename.as_deref(), Some("IMG_002.jpg"));
    }

    #[test]
    fn field_groups_collapse_indices() {
        assert_eq!(field_group("$.Kepek[12].Url"), "$.Kepek[].Url");
    }
}

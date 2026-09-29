//! The error-code catalog (`src/error.rs::ERROR_CODES`) is what both clients render from.
//! A code sent by the source but missing from the catalog shows as a raw English line on
//! the phone; a catalog entry nothing sends is a false promise. Both fail here.

use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use autocrm::error::ERROR_CODES;

/// Codes the `IntoResponse` impl and `From<TransitionError>` emit without a
/// `rule("…")`/`conflict("…")` call site.
const IMPLICIT: &[&str] = &[
    "unauthenticated",
    "forbidden",
    "not_found",
    "validation",
    "too_many_requests",
    "duplicate",
    "invalid_reference",
    "constraint_violation",
    "immutable",
    "internal",
    "stage_gate",
    "note_required",
    "invalid_transition",
];

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Every string literal passed as the first argument of `rule(` or `conflict(`.
fn codes_in_source() -> BTreeSet<String> {
    let mut files = Vec::new();
    rust_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    let mut codes = BTreeSet::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for call in ["rule(", "conflict("] {
            for (at, _) in text.match_indices(call) {
                let rest = text[at + call.len()..].trim_start();
                let Some(rest) = rest.strip_prefix('"') else {
                    continue;
                };
                let Some(end) = rest.find('"') else { continue };
                let code = &rest[..end];
                if !code.is_empty() && code.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
                    codes.insert(code.to_string());
                }
            }
        }
    }
    codes
}

#[test]
fn every_code_the_source_sends_is_in_the_catalog() {
    let catalog: HashSet<&str> = ERROR_CODES.iter().map(|c| c.code).collect();
    let missing: Vec<String> = codes_in_source()
        .into_iter()
        .filter(|c| !catalog.contains(c.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "codes sent by the API but missing from ERROR_CODES in src/error.rs: {missing:?}"
    );
}

#[test]
fn every_catalog_entry_is_sent_by_something() {
    let sent = codes_in_source();
    let unused: Vec<&str> = ERROR_CODES
        .iter()
        .map(|c| c.code)
        .filter(|c| !sent.contains(*c) && !IMPLICIT.contains(c))
        .collect();
    assert!(
        unused.is_empty(),
        "catalog entries nothing sends: {unused:?}"
    );
}

#[test]
fn catalog_entries_are_unique_and_complete() {
    let mut seen = HashSet::new();
    for c in ERROR_CODES {
        assert!(seen.insert(c.code), "duplicate catalog code {}", c.code);
        assert!(!c.hu.trim().is_empty(), "{} has no Hungarian text", c.code);
        assert!(!c.en.trim().is_empty(), "{} has no English text", c.code);
        assert!(
            (400..600).contains(&c.status),
            "{} has status {}",
            c.code,
            c.status
        );
    }
}

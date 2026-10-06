//! SQL, one module per aggregate. Functions take `impl PgExecutor` so they work on the
//! pool or inside a transaction (`&mut *tx`). All queries are compile-time checked.

pub mod absences;
pub mod attribution;
pub mod audit;
pub mod blockers;
pub mod config;
pub mod contacts;
pub mod documents;
pub mod emails;
pub mod employee_details;
pub mod employee_documents;
pub mod employee_statuses;
pub mod employees;
pub mod followups;
pub mod fx;
pub mod images;
pub mod incoming_invoices;
pub mod inspections;
pub mod invoices;
pub mod jobs;
pub mod lead_tags;
pub mod leads;
pub mod lost_reasons;
pub mod newsletter;
pub mod newsletter_tags;
pub mod notifications;
pub mod order_items;
pub mod order_notes;
pub mod order_specs;
pub mod orders;
pub mod partners;
pub mod raw_import;
pub mod recruitment;
pub mod reports;
pub mod search;
pub mod sessions;
pub mod stages;
pub mod tasks;
pub mod templates;
pub mod timeline;
pub mod users;
pub mod vehicles;

/// Turns free-text search input into an ILIKE pattern, escaping the wildcard characters
/// so a search for "50%" matches "50%" rather than everything starting with "50".
pub fn like_pattern(q: &str) -> Option<String> {
    let q = q.trim();
    if q.is_empty() {
        return None;
    }
    let escaped = q
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    Some(format!("%{escaped}%"))
}

/// Phone search pattern: digits with Hungarian prefixes unified
/// (see `domain::partner::normalize_phone`), wrapped for LIKE. The SQL side
/// applies the same transform to the column; `None` disables the predicate.
pub fn phone_pattern(q: &str) -> Option<String> {
    let norm = crate::domain::partner::normalize_phone(q);
    (!norm.is_empty()).then_some(format!("%{norm}%"))
}

/// Parses a `sort` query value (`name`, `-created_at`) against a whitelist of
/// allowed keys, returning the validated key (with any `-` prefix intact).
/// Anything else is an error — column names can never come from user input.
/// Callers pass the key as a bind parameter; the SQL matches it against
/// static CASE branches, so list queries stay fully compile-time checked.
pub fn parse_sort(sort: Option<&str>, allowed: &[&str], default: &str) -> Result<String, String> {
    let Some(raw) = sort.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(default.to_string());
    };
    let key = raw.strip_prefix('-').unwrap_or(raw);
    if !allowed.contains(&key) || key.is_empty() {
        return Err(format!("unknown sort '{raw}'"));
    }
    Ok(raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::{like_pattern, parse_sort, phone_pattern};

    const ALLOWED: &[&str] = &["name", "created_at"];

    #[test]
    fn like_patterns_escape_wildcards() {
        assert_eq!(like_pattern("  müller "), Some("%müller%".into()));
        assert_eq!(like_pattern("50%_a\\b"), Some("%50\\%\\_a\\\\b%".into()));
        assert_eq!(like_pattern("   "), None);
    }

    #[test]
    fn phone_patterns_normalise() {
        assert_eq!(
            phone_pattern("+36 30 123 4567"),
            Some("%36301234567%".into())
        );
        assert_eq!(phone_pattern("   "), None);
    }

    #[test]
    fn sort_parsing_defaults_and_rejects() {
        assert_eq!(parse_sort(None, ALLOWED, "name"), Ok("name".to_string()));
        assert_eq!(
            parse_sort(Some(""), ALLOWED, "name"),
            Ok("name".to_string())
        );
        assert_eq!(
            parse_sort(Some("created_at"), ALLOWED, "name"),
            Ok("created_at".to_string())
        );
        assert_eq!(
            parse_sort(Some("-created_at"), ALLOWED, "name"),
            Ok("-created_at".to_string())
        );
        assert!(parse_sort(Some("number; DROP TABLE x"), ALLOWED, "name").is_err());
        assert!(parse_sort(Some("-"), ALLOWED, "name").is_err());
    }
}

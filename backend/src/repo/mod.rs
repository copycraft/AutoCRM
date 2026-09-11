//! SQL, one module per aggregate. Functions take `impl PgExecutor` so they work on the
//! pool or inside a transaction (`&mut *tx`). All queries are compile-time checked.

pub mod audit;
pub mod blockers;
pub mod config;
pub mod contacts;
pub mod documents;
pub mod emails;
pub mod fx;
pub mod images;
pub mod jobs;
pub mod leads;
pub mod order_items;
pub mod orders;
pub mod partners;
pub mod reports;
pub mod sessions;
pub mod stages;
pub mod templates;
pub mod users;

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

#[cfg(test)]
mod tests {
    use super::like_pattern;

    #[test]
    fn like_patterns_escape_wildcards() {
        assert_eq!(like_pattern("  müller "), Some("%müller%".into()));
        assert_eq!(like_pattern("50%_a\\b"), Some("%50\\%\\_a\\\\b%".into()));
        assert_eq!(like_pattern("   "), None);
    }
}

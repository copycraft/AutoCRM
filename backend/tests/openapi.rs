//! The committed OpenAPI document is the contract the frontend types are generated from.
//! It must match what the code produces, and every operation must be uniquely identifiable.

use std::collections::HashSet;

#[test]
fn committed_openapi_document_is_current() {
    let generated = autocrm::api::openapi::document_json().unwrap();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../openapi/openapi.json");
    let committed = std::fs::read_to_string(path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert!(
        committed == generated,
        "openapi/openapi.json is stale. Regenerate it with:\n  cargo run --bin autocrm -- openapi --out ../openapi/openapi.json"
    );
}

#[test]
fn every_operation_has_a_unique_id_and_a_tag() {
    let doc = autocrm::api::openapi::document();
    let mut ids = HashSet::new();
    for (path, item) in &doc.paths.paths {
        for op in [&item.get, &item.post, &item.put, &item.patch, &item.delete]
            .into_iter()
            .flatten()
        {
            let id = op.operation_id.clone().expect("operation id");
            assert!(
                ids.insert(id.clone()),
                "duplicate operation id {id} at {path}"
            );
            assert!(
                op.tags.as_ref().is_some_and(|t| !t.is_empty()),
                "untagged operation at {path}"
            );
        }
    }
    assert!(
        ids.len() >= 68,
        "expected every route to be documented, found {}",
        ids.len()
    );
}

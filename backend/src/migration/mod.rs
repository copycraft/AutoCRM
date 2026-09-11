//! MiniCRM migration tooling, driven by the `autocrm-migrate` binary.
//!
//! ```text
//! extract   → migration-data/raw/**.json        (API, once, resumable)
//! manifest  → migration-data/manifest.jsonl     (file references; verify counts vs MiniCRM UI)
//! fetch     → object storage + fetched.jsonl    (days; resumable; content-addressed)
//! load      → database                          (idempotent upserts by minicrm_id)
//! reconcile → migration-data/reconciliation.md  (human sign-off)
//! ```

pub mod client;
pub mod extract;
pub mod fetch;
pub mod load;
pub mod manifest;
pub mod reconcile;

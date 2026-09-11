pub mod api;
pub mod config;
pub mod db;
pub mod domain;
pub mod error;
pub mod integrations;
pub mod jobs;
pub mod media;
pub mod migration;
pub mod repo;
pub mod service;
pub mod telemetry;

use std::sync::Arc;

/// Shared by every request handler and background job. Cheap to clone.
#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub config: Arc<config::Config>,
    pub storage: media::storage::Storage,
}

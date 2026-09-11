//! MiniCRM → AutoCRM migration. See docs/migration/README.md for the full procedure.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand};

use autocrm::config::Config;
use autocrm::media::storage::Storage;
use autocrm::migration::client::MiniCrmClient;
use autocrm::migration::extract::{Extractor, write_atomic};
use autocrm::migration::load::Mapping;
use autocrm::migration::{fetch, load, manifest, reconcile};
use autocrm::{AppState, db};

#[derive(Parser)]
#[command(name = "autocrm-migrate", about = "MiniCRM → AutoCRM migration")]
struct Cli {
    /// Working directory for raw data, manifest and logs.
    #[arg(long, global = true, default_value = "migration-data")]
    data: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// M1: download all records from the MiniCRM API into <data>/raw (resumable).
    Extract {
        /// Also extract to-do lists per project.
        #[arg(long)]
        todos: bool,
        #[arg(long, default_value_t = 50)]
        requests_per_minute: u32,
    },
    /// M2a: list every file referenced in the raw data. Check the counts against MiniCRM's UI.
    Manifest,
    /// M2b: download every manifest file into object storage (resumable; run it for days).
    Fetch {
        #[arg(long, default_value_t = 4)]
        concurrency: usize,
    },
    /// M3: load partners, contacts, orders, leads and files into the database (idempotent).
    Load {
        #[arg(long)]
        mapping: PathBuf,
    },
    /// M4: write <data>/reconciliation.md for human sign-off.
    Reconcile {
        #[arg(long)]
        mapping: PathBuf,
    },
}

fn minicrm_client(requests_per_minute: u32) -> anyhow::Result<MiniCrmClient> {
    let system_id = std::env::var("MINICRM_SYSTEM_ID").context("MINICRM_SYSTEM_ID is required")?;
    let api_key = std::env::var("MINICRM_API_KEY").context("MINICRM_API_KEY is required")?;
    let base = std::env::var("MINICRM_BASE_URL").unwrap_or_else(|_| "https://r3.minicrm.hu".into());
    MiniCrmClient::new(&base, &system_id, &api_key, requests_per_minute)
}

async fn app_state() -> anyhow::Result<AppState> {
    let config = Config::from_env()?;
    let pool = db::connect(&config).await?;
    db::migrate(&pool).await?;
    let storage = Storage::new(&config.s3);
    Ok(AppState {
        db: pool,
        config: Arc::new(config),
        storage,
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt().with_target(false).init();
    let cli = Cli::parse();
    let raw = cli.data.join("raw");
    tokio::fs::create_dir_all(&raw).await?;

    match cli.command {
        Command::Extract {
            todos,
            requests_per_minute,
        } => {
            let client = minicrm_client(requests_per_minute)?;
            let summary = Extractor::new(&client, &raw, todos).run().await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Command::Manifest => {
            let (entries, summary) = manifest::build(&raw)?;
            fetch::write_jsonl(&cli.data.join("manifest.jsonl"), &entries).await?;
            write_atomic(
                &cli.data.join("manifest-summary.json"),
                &serde_json::to_vec_pretty(&summary)?,
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            println!("\nCompare these counts with the MiniCRM UI before running `fetch`.");
        }
        Command::Fetch { concurrency } => {
            let state = app_state().await?;
            let client = Arc::new(minicrm_client(600)?);
            let summary = fetch::run(client, state.storage.clone(), &cli.data, concurrency).await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Command::Load { mapping } => {
            let mapping = Mapping::load(&mapping)?;
            let state = app_state().await?;
            let summary = load::run(&state, &mapping, &raw, &cli.data).await?;
            write_atomic(
                &cli.data.join("load-report.json"),
                &serde_json::to_vec_pretty(&summary)?,
            )
            .await?;
            println!(
                "partners {}, contacts {}, orders {}, leads {}, images {}, documents {}, skipped projects {}, problems {} (see load-report.json)",
                summary.partners,
                summary.contacts,
                summary.orders,
                summary.leads,
                summary.images,
                summary.documents,
                summary.skipped_projects,
                summary.problems.len()
            );
        }
        Command::Reconcile { mapping } => {
            let mapping = Mapping::load(&mapping)?;
            let state = app_state().await?;
            let report = reconcile::run(&state, &mapping, &raw, &cli.data).await?;
            let path = cli.data.join("reconciliation.md");
            write_atomic(&path, report.as_bytes()).await?;
            println!("wrote {}", path.display());
        }
    }
    Ok(())
}

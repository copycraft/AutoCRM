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
        /// Skip per-project to-do lists. They are the account's project history and load
        /// into order_notes (V1.3); leave them on unless the account has none.
        #[arg(long)]
        no_todos: bool,
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
        /// Resolve every field and report per-column coverage without writing anything.
        /// Use it to answer "would the plate mapping actually find a plate?" (V7.2).
        #[arg(long)]
        dry_run: bool,
    },
    /// M4: write <data>/reconciliation.md for human sign-off.
    Reconcile {
        #[arg(long)]
        mapping: PathBuf,
    },
}

/// Per-column coverage: what share of source records carry a value for each destination
/// column. A column at 0% means the mapping has no path for it — the shape of the V1.1
/// finding, made visible before the load runs.
fn print_coverage(label: &str, total: usize, coverage: &std::collections::BTreeMap<String, usize>) {
    if coverage.is_empty() {
        return;
    }
    println!(
        "
{label}: {total} records"
    );
    for (column, n) in coverage {
        let pct = if total == 0 {
            0.0
        } else {
            *n as f64 * 100.0 / total as f64
        };
        let flag = if *n == 0 { "  ← nothing mapped" } else { "" };
        println!("  {column:<18} {n:>6} ({pct:5.1}%){flag}");
    }
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
            no_todos,
            requests_per_minute,
        } => {
            let client = minicrm_client(requests_per_minute)?;
            let summary = Extractor::new(&client, &raw, !no_todos).run().await?;
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
        Command::Load { mapping, dry_run } => {
            let mapping = Mapping::load(&mapping)?;
            let state = app_state().await?;
            let summary = load::run(&state, &mapping, &raw, &cli.data, dry_run).await?;
            let report = if dry_run {
                "load-dry-run.json"
            } else {
                "load-report.json"
            };
            write_atomic(
                &cli.data.join(report),
                &serde_json::to_vec_pretty(&summary)?,
            )
            .await?;
            if dry_run {
                println!(
                    "DRY RUN — nothing was written.
"
                );
                println!(
                    "orders {}, leads {}, partners {}, contacts {}, skipped projects {}, problems {}",
                    summary.orders,
                    summary.leads,
                    summary.partners,
                    summary.contacts,
                    summary.skipped_projects,
                    summary.problems.len()
                );
                print_coverage("orders", summary.orders, &summary.order_field_coverage);
                print_coverage(
                    "partners",
                    summary.partners,
                    &summary.partner_field_coverage,
                );
                println!(
                    "
See {report} for the full list."
                );
            } else {
                println!(
                    "partners {}, contacts {}, orders {}, leads {}, notes {}, images {}, documents {}, skipped projects {}, problems {} (see {report})",
                    summary.partners,
                    summary.contacts,
                    summary.orders,
                    summary.leads,
                    summary.notes,
                    summary.images,
                    summary.documents,
                    summary.skipped_projects,
                    summary.problems.len()
                );
            }
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

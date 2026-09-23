use std::io::BufRead;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use chrono::NaiveDate;
use clap::{Parser, Subcommand};

use autocrm::config::{Config, EmailTransportConfig};
use autocrm::domain::role::Role;
use autocrm::domain::template::text_to_html;
use autocrm::integrations::email::{Mailer, OutgoingEmail};
use autocrm::media::storage::Storage;
use autocrm::{AppState, api, db, jobs, repo, service, telemetry};

#[derive(Parser)]
#[command(
    name = "autocrm",
    version,
    about = "Autotherm project lifecycle system"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the HTTP API and background worker (the default).
    Serve,
    /// Apply pending database migrations and exit.
    Migrate,
    /// Create an admin account. Password comes from AUTOCRM_ADMIN_PASSWORD or the first line of stdin.
    CreateAdmin {
        #[arg(long)]
        email: String,
        #[arg(long)]
        name: String,
    },
    /// Fetch MNB exchange rates for a date range now (historical backfill before migration).
    FxBackfill {
        #[arg(long)]
        from: NaiveDate,
        #[arg(long)]
        to: NaiveDate,
    },
    /// Send one test email straight through the configured transport (bypassing the queue
    /// and safety rails) and report exactly what the server said.
    EmailTest {
        #[arg(long)]
        to: String,
    },
    /// Write the OpenAPI document (the API contract) as JSON. Needs no configuration.
    Openapi {
        /// Output file; stdout when omitted.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let cli = Cli::parse();
    if let Some(Command::Openapi { out }) = &cli.command {
        let json = api::openapi::document_json()?;
        match out {
            Some(path) => {
                std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))?
            }
            None => print!("{json}"),
        }
        return Ok(());
    }
    let config = Config::from_env()?;
    let _log_guard = telemetry::init(&config)?;

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve(config).await,
        Command::Migrate => {
            let pool = db::connect(&config).await?;
            db::migrate(&pool).await?;
            tracing::info!("migrations applied");
            Ok(())
        }
        Command::CreateAdmin { email, name } => create_admin(config, email, name).await,
        Command::FxBackfill { from, to } => {
            let state = build_state(config).await?;
            let stored = service::automation::fetch_fx_rates(&state, from, to).await?;
            println!("stored {stored} rates");
            Ok(())
        }
        Command::EmailTest { to } => email_test(config, to).await,
        Command::Openapi { .. } => unreachable!("handled before configuration is loaded"),
    }
}

async fn email_test(config: Config, to: String) -> anyhow::Result<()> {
    let to = autocrm::domain::email::normalize_address(&to).context("invalid recipient address")?;
    let mailer = Mailer::from_config(&config.email)?;
    match &config.email.transport {
        EmailTransportConfig::DryRun => {
            println!(
                "EMAIL_MODE=dry_run: the message is only logged. Set EMAIL_MODE=smtp to test a real server."
            );
        }
        EmailTransportConfig::Smtp(smtp) => {
            println!(
                "Connecting to {}:{} ({:?}, EHLO {}{}{})",
                smtp.host,
                smtp.port,
                smtp.security,
                smtp.helo_name,
                if smtp.force_ipv4 { ", IPv4 only" } else { "" },
                if smtp.username.is_some() {
                    ", with authentication"
                } else {
                    ", no authentication"
                },
            );
            mailer
                .test_connection()
                .await
                .map_err(|e| anyhow::anyhow!("connection test failed: {e}"))?;
            println!("Connection OK.");
        }
    }
    if let Some(redirect) = mailer.redirect_to() {
        println!("EMAIL_REDIRECT_TO is set: the message will be delivered to {redirect} instead.");
    }

    let from = service::email::format_from(&config.email.from_name, &config.email.from_automatic)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let random: [u8; 6] = rand::random();
    let body = format!(
        "Ez egy teszt üzenet az AutoCRM-ből.\n\nFeladó: {}\nKörnyezet: {:?}\n\nHa megérkezett, nézd meg a fejléceket (Gmail: Továbbiak → Eredeti megjelenítése): SPF, DKIM és DMARC is legyen PASS.",
        config.email.from_automatic, config.env
    );
    let result = mailer
        .send(OutgoingEmail {
            message_id: format!(
                "<email-test.{}@{}>",
                hex::encode(random),
                config.email.message_id_domain
            ),
            from,
            reply_to: Some(config.email.reply_to_default.clone()),
            to: to.clone(),
            cc: vec![],
            bcc: vec![],
            subject: "AutoCRM teszt e-mail".into(),
            body_html: text_to_html(&body),
            body_text: body,
            attachments: vec![],
            automatic: true,
        })
        .await;
    match result {
        Ok(()) if mailer.is_dry_run() => println!("Logged (dry run)."),
        Ok(()) => println!(
            "Accepted by the server for {to}. Check the inbox (and spam), then the original headers for spf=pass, dkim=pass, dmarc=pass."
        ),
        Err(e) => anyhow::bail!("send failed: {e}"),
    }
    Ok(())
}

async fn build_state(config: Config) -> anyhow::Result<AppState> {
    let pool = db::connect(&config).await?;
    db::migrate(&pool).await?;
    let storage = Storage::new(&config.s3);
    Ok(AppState {
        db: pool,
        config: Arc::new(config),
        storage,
    })
}

async fn serve(config: Config) -> anyhow::Result<()> {
    let state = build_state(config).await?;
    if let Some(redirect) = &state.config.email.redirect_to {
        tracing::warn!(%redirect, "EMAIL_REDIRECT_TO is set: every outgoing email is delivered to this address instead of its recipients");
    }
    if !state.config.is_production() {
        tracing::warn!(env = ?state.config.env, "non-production environment: outbound email is dry-run or local-sink only");
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let worker = state
        .config
        .worker_enabled
        .then(|| tokio::spawn(jobs::run(state.clone(), shutdown_rx)));

    let listener = tokio::net::TcpListener::bind(state.config.bind_addr)
        .await
        .with_context(|| format!("binding {}", state.config.bind_addr))?;
    tracing::info!(addr = %state.config.bind_addr, "listening");

    axum::serve(listener, api::router(state.clone()))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    let _ = shutdown_tx.send(true);
    if let Some(worker) = worker {
        let _ = worker.await;
    }
    tracing::info!("shut down cleanly");
    Ok(())
}

async fn create_admin(config: Config, email: String, name: String) -> anyhow::Result<()> {
    let password = match std::env::var("AUTOCRM_ADMIN_PASSWORD") {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Password (min 12 characters):");
            std::io::stdin()
                .lock()
                .lines()
                .next()
                .context("no password on stdin")??
        }
    };
    service::auth::validate_new_password(&password).map_err(|e| anyhow::anyhow!("{e}"))?;
    let email =
        autocrm::domain::email::normalize_address(&email).context("invalid email address")?;

    let pool = db::connect(&config).await?;
    db::migrate(&pool).await?;
    let hash = service::auth::hash_password_async(password).await?;
    let user = repo::users::insert(&pool, &email, name.trim(), Role::Admin, &hash, false).await?;
    println!("created admin #{} <{}>", user.id, user.email);
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}

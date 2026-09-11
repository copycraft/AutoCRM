//! Structured logging. Stdout by default; rotated daily files (30 kept) when LOG_DIR is set.

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::writer::BoxMakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

use crate::config::{Config, LogFormat};

/// Keep the returned guard alive for the life of the process, or buffered log lines are lost.
pub fn init(config: &Config) -> anyhow::Result<Option<WorkerGuard>> {
    let filter = EnvFilter::try_from_env("LOG_LEVEL").unwrap_or_else(|_| {
        EnvFilter::new("info,sqlx=warn,aws_smithy_runtime=warn,aws_runtime=warn,aws_sdk_s3=warn")
    });

    let (writer, guard) = match &config.log_dir {
        Some(dir) => {
            let appender = RollingFileAppender::builder()
                .rotation(Rotation::DAILY)
                .filename_prefix("autocrm")
                .filename_suffix("log")
                .max_log_files(30)
                .build(dir)?;
            let (non_blocking, guard) = tracing_appender::non_blocking(appender);
            (BoxMakeWriter::new(non_blocking), Some(guard))
        }
        None => (BoxMakeWriter::new(std::io::stdout), None),
    };

    let registry = tracing_subscriber::registry().with(filter);
    match config.log_format {
        LogFormat::Json => registry
            .with(fmt::layer().json().with_writer(writer))
            .try_init()?,
        LogFormat::Pretty => registry.with(fmt::layer().with_writer(writer)).try_init()?,
    }
    Ok(guard)
}

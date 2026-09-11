//! Phase M2b: download every manifest entry into object storage.
//!
//! Resumable by construction: every success is appended to `fetched.jsonl` the moment it
//! is stored, failures to `failed.jsonl`, and a rerun skips anything already fetched (and
//! retries failures). Files are hashed on arrival and stored content-addressed under
//! `migration/originals/{sha256}.{ext}`, which deduplicates a decade of repeated photos for
//! free. Order ids don't exist yet at download time; `load` links rows to these keys.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, bail};
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;

use super::client::MiniCrmClient;
use super::manifest::ManifestEntry;
use crate::media::storage::Storage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedEntry {
    pub source_url: String,
    pub sha256: String,
    pub byte_size: i64,
    pub content_type: String,
    pub storage_key: String,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedEntry {
    pub source_url: String,
    pub error: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Default, Serialize)]
pub struct FetchSummary {
    pub manifest_urls: usize,
    pub already_fetched: usize,
    pub fetched_now: usize,
    pub failed_now: usize,
    pub bytes_now: i64,
}

pub async fn read_jsonl<T: DeserializeOwned>(path: &Path) -> anyhow::Result<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = tokio::fs::read_to_string(path).await?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, line)| {
            serde_json::from_str(line).with_context(|| format!("{} line {}", path.display(), i + 1))
        })
        .collect()
}

pub async fn write_jsonl<T: Serialize>(path: &Path, items: &[T]) -> anyhow::Result<()> {
    let mut out = String::new();
    for item in items {
        out.push_str(&serde_json::to_string(item)?);
        out.push('\n');
    }
    super::extract::write_atomic(path, out.as_bytes()).await
}

async fn append_line<T: Serialize>(file: &Mutex<tokio::fs::File>, item: &T) -> anyhow::Result<()> {
    let mut line = serde_json::to_vec(item)?;
    line.push(b'\n');
    let mut f = file.lock().await;
    f.write_all(&line).await?;
    f.flush().await?;
    Ok(())
}

async fn open_append(path: &Path) -> anyhow::Result<tokio::fs::File> {
    Ok(tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?)
}

pub fn content_type_for(extension: Option<&str>, header: Option<&str>) -> String {
    let by_ext = match extension {
        Some("jpg" | "jpeg") => Some("image/jpeg"),
        Some("png") => Some("image/png"),
        Some("gif") => Some("image/gif"),
        Some("webp") => Some("image/webp"),
        Some("heic") => Some("image/heic"),
        Some("heif") => Some("image/heif"),
        Some("tif" | "tiff") => Some("image/tiff"),
        Some("bmp") => Some("image/bmp"),
        Some("pdf") => Some("application/pdf"),
        _ => None,
    };
    by_ext
        .map(str::to_string)
        .or_else(|| header.map(|h| h.split(';').next().unwrap_or(h).trim().to_ascii_lowercase()))
        .filter(|ct| !ct.is_empty())
        .unwrap_or_else(|| "application/octet-stream".into())
}

fn extension_for(content_type: &str) -> &'static str {
    match content_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/heic" => "heic",
        "image/tiff" => "tif",
        "application/pdf" => "pdf",
        _ => "bin",
    }
}

async fn fetch_one(
    client: &MiniCrmClient,
    storage: &Storage,
    url: &str,
    extension: Option<&str>,
) -> anyhow::Result<FetchedEntry> {
    let (bytes, header_type) = client.get_bytes(url).await?;
    if bytes.is_empty() {
        bail!("empty response");
    }
    if header_type
        .as_deref()
        .is_some_and(|h| h.starts_with("text/html"))
        && extension.is_some()
    {
        bail!("got an HTML page instead of a file (expired link or authentication required?)");
    }
    let sha256 = hex::encode(Sha256::digest(&bytes));
    let content_type = content_type_for(extension, header_type.as_deref());
    let ext = extension.unwrap_or_else(|| extension_for(&content_type));
    let storage_key = format!("migration/originals/{sha256}.{ext}");
    let byte_size = i64::try_from(bytes.len()).unwrap_or(i64::MAX);
    if storage.head(&storage_key).await?.is_none() {
        storage
            .put_bytes(&storage_key, bytes, &content_type)
            .await?;
    }
    Ok(FetchedEntry {
        source_url: url.to_string(),
        sha256,
        byte_size,
        content_type,
        storage_key,
        fetched_at: Utc::now(),
    })
}

pub async fn run(
    client: Arc<MiniCrmClient>,
    storage: Storage,
    dir: &Path,
    concurrency: usize,
) -> anyhow::Result<FetchSummary> {
    let manifest: Vec<ManifestEntry> = read_jsonl(&dir.join("manifest.jsonl")).await?;
    if manifest.is_empty() {
        bail!("manifest.jsonl is empty or missing; run `manifest` first");
    }
    let done: HashSet<String> = read_jsonl::<FetchedEntry>(&dir.join("fetched.jsonl"))
        .await?
        .into_iter()
        .map(|f| f.source_url)
        .collect();

    let mut summary = FetchSummary::default();
    let mut seen = HashSet::new();
    let mut todo = Vec::new();
    for entry in manifest {
        if !seen.insert(entry.source_url.clone()) {
            continue;
        }
        summary.manifest_urls += 1;
        if done.contains(&entry.source_url) {
            summary.already_fetched += 1;
        } else {
            todo.push((entry.source_url, entry.extension));
        }
    }
    tracing::info!(
        total = summary.manifest_urls,
        remaining = todo.len(),
        "starting download"
    );

    let fetched_log = Arc::new(Mutex::new(open_append(&dir.join("fetched.jsonl")).await?));
    let failed_log = Arc::new(Mutex::new(open_append(&dir.join("failed.jsonl")).await?));
    let semaphore = Arc::new(Semaphore::new(concurrency.max(1)));
    let mut tasks: JoinSet<anyhow::Result<Option<i64>>> = JoinSet::new();
    let total = todo.len();

    let record =
        |result: anyhow::Result<Option<i64>>, summary: &mut FetchSummary| -> anyhow::Result<()> {
            match result? {
                Some(bytes) => {
                    summary.fetched_now += 1;
                    summary.bytes_now += bytes;
                }
                None => summary.failed_now += 1,
            }
            let processed = summary.fetched_now + summary.failed_now;
            if processed % 100 == 0 {
                tracing::info!(
                    processed,
                    total,
                    failed = summary.failed_now,
                    gib = summary.bytes_now as f64 / 1_073_741_824.0,
                    "download progress"
                );
            }
            Ok(())
        };

    for (url, extension) in todo {
        let permit = semaphore.clone().acquire_owned().await?;
        let (client, storage, fetched_log, failed_log) = (
            client.clone(),
            storage.clone(),
            fetched_log.clone(),
            failed_log.clone(),
        );
        tasks.spawn(async move {
            let _permit = permit;
            match fetch_one(&client, &storage, &url, extension.as_deref()).await {
                Ok(entry) => {
                    let size = entry.byte_size;
                    append_line(&fetched_log, &entry).await?;
                    Ok(Some(size))
                }
                Err(e) => {
                    tracing::warn!(%url, error = %e, "download failed");
                    append_line(
                        &failed_log,
                        &FailedEntry {
                            source_url: url,
                            error: format!("{e:#}"),
                            at: Utc::now(),
                        },
                    )
                    .await?;
                    Ok(None)
                }
            }
        });
        while tasks.len() > concurrency.max(1) * 4 {
            if let Some(joined) = tasks.join_next().await {
                record(joined?, &mut summary)?;
            }
        }
    }
    while let Some(joined) = tasks.join_next().await {
        record(joined?, &mut summary)?;
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_types_prefer_extension() {
        assert_eq!(
            content_type_for(Some("jpg"), Some("application/octet-stream")),
            "image/jpeg"
        );
        assert_eq!(
            content_type_for(None, Some("image/png; charset=binary")),
            "image/png"
        );
        assert_eq!(content_type_for(None, None), "application/octet-stream");
    }
}

//! HEIC (iPhone) photos: the original is stored as uploaded, but the image library cannot
//! decode HEVC, so previews come from an external converter run on a copy.
//!
//! The command is configuration (`HEIC_CONVERTER`, default libheif's `heif-dec`), called as
//! `<command> <input.heic> <output.jpg>`; ImageMagick's `magick` takes the same arguments.
//! Install libheif on the server (`apt install libheif-examples`) and HEIC previews work.

use std::time::Duration;

use anyhow::{Context, bail};

const TIMEOUT: Duration = Duration::from_secs(120);

/// A JPEG of the HEIC `bytes`, made by `command` in a private temporary directory.
pub async fn to_jpeg(command: &str, bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    if command.is_empty() {
        bail!("HEIC previews are off (HEIC_CONVERTER is empty); the original is stored");
    }
    let dir = std::env::temp_dir().join(format!("autocrm-heic-{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&dir).await?;
    let input = dir.join("in.heic");
    let output = dir.join("out.jpg");
    let result = async {
        tokio::fs::write(&input, bytes).await?;
        let run = tokio::process::Command::new(command)
            .arg(&input)
            .arg(&output)
            .kill_on_drop(true)
            .output();
        let out = match tokio::time::timeout(TIMEOUT, run).await {
            Err(_) => bail!("{command} did not finish in {}s", TIMEOUT.as_secs()),
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => bail!(
                "HEIC previews need '{command}' on the server (libheif: apt install libheif-examples); the original is stored"
            ),
            Ok(r) => r.with_context(|| format!("running {command}"))?,
        };
        if !out.status.success() {
            bail!(
                "{command} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim().chars().take(300).collect::<String>()
            );
        }
        tokio::fs::read(&output)
            .await
            .with_context(|| format!("{command} wrote no output"))
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&dir).await;
    result
}

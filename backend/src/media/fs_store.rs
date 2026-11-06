//! Files on a mounted filesystem, in production an NFS share: plain files in project
//! folders ([`super::layout`]) that a person can open from the share. A key is a relative
//! path under `STORAGE_DIR`.
//!
//! Browsers and phones cannot PUT to a share, so a presigned request here points at this
//! server (`/api/files/{token}`, `api::files`). The token binds what an S3 presignature
//! binds: one key, one size, one sha256, one expiry. An upload is written under `.tmp/` and
//! linked into place only once its bytes match, so a half-written file never shows up in a
//! project folder, and a file already there is never replaced by different bytes.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, anyhow, bail};
use aws_sdk_s3::primitives::ByteStream;
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::storage::{ObjectInfo, PresignedRequest};
use crate::config::FsConfig;

/// Uploads in flight, and nothing else. Never inside a project folder.
pub const TMP_DIR: &str = ".tmp";
/// A partial upload this old is an aborted request's leftover.
const STALE_PART: Duration = Duration::from_secs(24 * 3600);
/// How long a successful mount check is trusted before /proc is read again.
const MOUNT_RECHECK: Duration = Duration::from_secs(5);
/// Longer than an upload ticket lives.
const VERIFIED_FOR: Duration = Duration::from_secs(3 * 3600);

/// Domain separation: an upload token never works as a download link, nor the reverse.
const PUT_CONTEXT: &[u8] = b"autocrm-file-put-v1.";
const GET_CONTEXT: &[u8] = b"autocrm-file-get-v1.";

#[derive(Clone)]
pub struct FsStore {
    inner: Arc<Inner>,
}

struct Inner {
    root: PathBuf,
    base_url: String,
    signing_key: Vec<u8>,
    require_mount: bool,
    intake_lock_years: u32,
    mount_checked: Mutex<Option<Instant>>,
    /// Files this process linked in after checking their sha256 (base64, size, when): the
    /// upload's completion need not read a 2 GB drawing back off the share to know it.
    verified: Mutex<HashMap<String, (String, u64, Instant)>>,
}

/// What an upload token allows: exactly this file, at exactly this key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PutClaims {
    #[serde(rename = "k")]
    pub key: String,
    #[serde(rename = "t")]
    pub content_type: String,
    #[serde(rename = "n")]
    pub size: i64,
    /// Base64, the way S3's `x-amz-checksum-sha256` carries it.
    #[serde(rename = "h")]
    pub sha256_b64: String,
    /// Write-once evidence: stored as a read-only file.
    #[serde(rename = "ro", default)]
    pub read_only: bool,
    #[serde(rename = "e")]
    pub expires_at: i64,
}

/// What a download link allows: this key, until then, shown this way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetClaims {
    #[serde(rename = "k")]
    pub key: String,
    #[serde(rename = "d", default, skip_serializing_if = "Option::is_none")]
    pub disposition: Option<String>,
    #[serde(rename = "e")]
    pub expires_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TokenError {
    #[error("malformed file link")]
    Malformed,
    #[error("file link signature is invalid")]
    BadSignature,
    #[error("file link has expired")]
    Expired,
}

#[derive(Debug, thiserror::Error)]
pub enum CommitError {
    #[error("a different file is already stored at {0}")]
    Conflict(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// What happens when the key is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Commit {
    /// Derived copies, generated PDFs, HR papers: the new bytes win (never over a
    /// read-only file).
    Replace,
    /// Uploads and moves: the same bytes again is success, different bytes a conflict.
    KeepExisting,
}

/// The filesystem a path lives on, from `/proc/self/mountinfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountInfo {
    pub mount_point: PathBuf,
    pub fs_type: String,
    pub source: String,
}

impl FsStore {
    pub fn new(cfg: &FsConfig, public_base_url: &str, signing_key: &[u8]) -> Self {
        FsStore {
            inner: Arc::new(Inner {
                root: cfg.dir.clone(),
                base_url: public_base_url.trim_end_matches('/').to_string(),
                signing_key: signing_key.to_vec(),
                require_mount: cfg.require_mount,
                intake_lock_years: cfg.intake_lock_years,
                mount_checked: Mutex::new(None),
                verified: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn root(&self) -> &Path {
        &self.inner.root
    }

    /// The file behind a key. Keys come from the layout, but a bad one must still never
    /// reach outside the root.
    pub fn path(&self, key: &str) -> anyhow::Result<PathBuf> {
        validate_key(key)?;
        Ok(self.inner.root.join(key))
    }

    pub fn intake_lock_until(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        (self.inner.intake_lock_years > 0).then(|| {
            now + chrono::TimeDelta::days(365 * i64::from(self.inner.intake_lock_years) + 2)
        })
    }

    /// A share has no retention dates: write-once means a read-only file. Real WORM, if
    /// wanted, belongs to the NAS (snapshots, a locked dataset).
    pub async fn lock_object(&self, key: &str, _until: DateTime<Utc>) -> anyhow::Result<()> {
        set_read_only(&self.path(key)?, true)
            .await
            .with_context(|| format!("making {key} read-only"))
    }

    // ── Signed links ──────────────────────────────────────────────────────────

    pub fn presign_put(
        &self,
        key: &str,
        content_type: &str,
        byte_size: i64,
        sha256_b64: &str,
        lock_until: Option<DateTime<Utc>>,
        expires_in: Duration,
    ) -> anyhow::Result<PresignedRequest> {
        validate_key(key)?;
        let claims = PutClaims {
            key: key.to_string(),
            content_type: content_type.to_string(),
            size: byte_size,
            sha256_b64: sha256_b64.to_string(),
            read_only: lock_until.is_some(),
            expires_at: expiry(expires_in),
        };
        let token = sign(&self.inner.signing_key, PUT_CONTEXT, &claims);
        Ok(PresignedRequest {
            method: "PUT".into(),
            url: format!("{}/api/files/{token}", self.inner.base_url),
            headers: vec![("content-type".into(), content_type.to_string())],
        })
    }

    pub fn presign_get(
        &self,
        key: &str,
        expires_in: Duration,
        content_disposition: Option<String>,
    ) -> anyhow::Result<String> {
        validate_key(key)?;
        let claims = GetClaims {
            key: key.to_string(),
            disposition: content_disposition,
            expires_at: expiry(expires_in),
        };
        let token = sign(&self.inner.signing_key, GET_CONTEXT, &claims);
        Ok(format!("{}/api/files/{token}", self.inner.base_url))
    }

    pub fn verify_put(&self, token: &str, now_unix: i64) -> Result<PutClaims, TokenError> {
        let claims: PutClaims = verify(&self.inner.signing_key, PUT_CONTEXT, token)?;
        if claims.expires_at < now_unix {
            return Err(TokenError::Expired);
        }
        Ok(claims)
    }

    pub fn verify_get(&self, token: &str, now_unix: i64) -> Result<GetClaims, TokenError> {
        let claims: GetClaims = verify(&self.inner.signing_key, GET_CONTEXT, token)?;
        if claims.expires_at < now_unix {
            return Err(TokenError::Expired);
        }
        Ok(claims)
    }

    // ── Reading ───────────────────────────────────────────────────────────────

    pub async fn head(&self, key: &str) -> anyhow::Result<Option<ObjectInfo>> {
        match tokio::fs::metadata(self.path(key)?).await {
            Ok(meta) if meta.is_file() => Ok(Some(ObjectInfo {
                size: i64::try_from(meta.len()).unwrap_or(i64::MAX),
                // Known only for what this process checked on the way in; otherwise a caller
                // that must be sure reads the file back.
                sha256_b64: self.verified_hash(key, meta.len()),
            })),
            Ok(_) => Ok(None),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(anyhow!("stat {key}: {e}")),
        }
    }

    pub async fn get_bytes(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        tokio::fs::read(self.path(key)?)
            .await
            .with_context(|| format!("reading {key}"))
    }

    pub async fn open(&self, key: &str) -> anyhow::Result<ByteStream> {
        ByteStream::from_path(self.path(key)?)
            .await
            .map_err(|e| anyhow!("opening {key}: {e}"))
    }

    // ── Writing ───────────────────────────────────────────────────────────────

    /// Starts an upload under `.tmp/`; [`FsStore::commit`] puts it in place.
    pub async fn begin(&self) -> io::Result<Upload> {
        self.ensure_mounted()?;
        let dir = self.inner.root.join(TMP_DIR);
        tokio::fs::create_dir_all(&dir).await?;
        let random: [u8; 12] = rand::random();
        let path = dir.join(format!("{}.part", hex::encode(random)));
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await?;
        Ok(Upload {
            path: Some(path),
            file: Some(file),
            hasher: Sha256::new(),
            size: 0,
        })
    }

    /// Moves a finished upload to `key`, creating the folders on the way.
    pub async fn commit(
        &self,
        mut done: Finished,
        key: &str,
        mode: Commit,
        read_only: bool,
    ) -> Result<(), CommitError> {
        self.ensure_mounted()?;
        let dest = self
            .path(key)
            .map_err(|e| io::Error::other(e.to_string()))?;
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let src = done.path().to_path_buf();
        match mode {
            Commit::Replace => {
                if let Ok(meta) = tokio::fs::metadata(&dest).await
                    && meta.permissions().readonly()
                {
                    return Err(CommitError::Io(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!("{key} is write-once"),
                    )));
                }
                tokio::fs::rename(&src, &dest).await?;
                self.forget(key);
            }
            // A hard link is "create unless it exists" in one step, which NFS does
            // atomically on the server; a rename would silently replace.
            Commit::KeepExisting => match tokio::fs::hard_link(&src, &dest).await {
                Ok(()) => {
                    let _ = tokio::fs::remove_file(&src).await;
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    same_or_conflict(&dest, &done.sha256, key).await?;
                    self.remember(key, &done);
                    return Ok(());
                }
                Err(_) => {
                    // A filesystem without hard links: look, then rename.
                    if tokio::fs::try_exists(&dest).await? {
                        same_or_conflict(&dest, &done.sha256, key).await?;
                        self.remember(key, &done);
                        return Ok(());
                    }
                    tokio::fs::rename(&src, &dest).await?;
                }
            },
        }
        done.disarm();
        if read_only {
            set_read_only(&dest, true).await?;
        }
        if mode == Commit::KeepExisting {
            self.remember(key, &done);
        }
        Ok(())
    }

    fn remember(&self, key: &str, done: &Finished) {
        let mut verified = self
            .inner
            .verified
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        verified.retain(|_, (_, _, at)| at.elapsed() < VERIFIED_FOR);
        verified.insert(
            key.to_string(),
            (sha256_b64(&done.sha256), done.size, Instant::now()),
        );
    }

    fn forget(&self, key: &str) {
        let mut verified = self
            .inner
            .verified
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        verified.remove(key);
    }

    fn verified_hash(&self, key: &str, size: u64) -> Option<String> {
        let verified = self
            .inner
            .verified
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        verified
            .get(key)
            .filter(|(_, n, at)| *n == size && at.elapsed() < VERIFIED_FOR)
            .map(|(hash, _, _)| hash.clone())
    }

    pub async fn put_bytes(&self, key: &str, bytes: Vec<u8>) -> anyhow::Result<()> {
        let mut upload = self.begin().await.context("starting a write")?;
        upload.write(&bytes).await?;
        let done = upload.finish().await?;
        self.commit(done, key, Commit::Replace, false)
            .await
            .with_context(|| format!("storing {key}"))
    }

    /// Copies a stream from elsewhere (the old object store) to `key`, checked against the
    /// recorded size and sha256 where they are known.
    pub async fn write_stream(
        &self,
        key: &str,
        mut stream: ByteStream,
        expect_size: Option<i64>,
        expect_sha256: Option<&[u8]>,
        read_only: bool,
    ) -> anyhow::Result<()> {
        let mut upload = self.begin().await.context("starting a write")?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| anyhow!("reading the source of {key}: {e}"))?;
            upload.write(&chunk).await?;
        }
        let done = upload.finish().await?;
        if let Some(size) = expect_size
            && i64::try_from(done.size).ok() != Some(size)
        {
            bail!("{key}: {} bytes arrived, {size} were recorded", done.size);
        }
        if let Some(hash) = expect_sha256
            && done.sha256.as_slice() != hash
        {
            bail!("{key}: the bytes do not match the recorded sha256");
        }
        self.commit(done, key, Commit::KeepExisting, read_only)
            .await
            .with_context(|| format!("storing {key}"))
    }

    /// Gives a file a second name: a hard link, so nothing is copied and the old name keeps
    /// working until it is deleted. Falls back to a copy where links are not supported.
    pub async fn link(&self, from: &str, to: &str) -> anyhow::Result<()> {
        self.ensure_mounted()?;
        let src = self.path(from)?;
        let dest = self.path(to)?;
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        match tokio::fs::hard_link(&src, &dest).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let (_, hash) = hash_file(&src).await?;
                same_or_conflict(&dest, &hash, to).await?;
                Ok(())
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err(anyhow!("{from} does not exist")),
            Err(_) => {
                let read_only = tokio::fs::metadata(&src).await?.permissions().readonly();
                let stream = self.open(from).await?;
                self.write_stream(to, stream, None, None, read_only).await
            }
        }
    }

    /// Removes a file, then any folders it leaves empty. Missing files are not an error.
    pub async fn delete(&self, key: &str) -> anyhow::Result<()> {
        let path = self.path(key)?;
        self.forget(key);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            // Windows refuses to remove a read-only file; Linux only asks the folder.
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                set_read_only(&path, false).await?;
                tokio::fs::remove_file(&path)
                    .await
                    .with_context(|| format!("deleting {key}"))?;
            }
            Err(e) => return Err(anyhow!("deleting {key}: {e}")),
        }
        let mut dir = path.parent().map(Path::to_path_buf);
        while let Some(d) = dir {
            if d == self.inner.root || tokio::fs::remove_dir(&d).await.is_err() {
                break;
            }
            dir = d.parent().map(Path::to_path_buf);
        }
        Ok(())
    }

    // ── Health ────────────────────────────────────────────────────────────────

    /// The folder exists, is the share (when required), and takes a write.
    pub async fn check(&self) -> anyhow::Result<()> {
        let meta = tokio::fs::metadata(&self.inner.root)
            .await
            .with_context(|| format!("STORAGE_DIR {}", self.inner.root.display()))?;
        if !meta.is_dir() {
            bail!("STORAGE_DIR {} is not a folder", self.inner.root.display());
        }
        let mut probe = self.begin().await.context("writing to STORAGE_DIR")?;
        probe.write(b"ok").await?;
        drop(probe.finish().await?);
        Ok(())
    }

    /// The filesystem STORAGE_DIR is on, for the startup log. `None` off Linux.
    pub fn mount(&self) -> io::Result<Option<MountInfo>> {
        mount_of(&self.inner.root)
    }

    /// Deletes partial uploads left by requests that died, returning how many.
    pub async fn sweep_partials(&self) -> io::Result<usize> {
        let dir = self.inner.root.join(TMP_DIR);
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e),
        };
        let mut removed = 0;
        while let Some(entry) = entries.next_entry().await? {
            let stale = entry
                .metadata()
                .await
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| SystemTime::now().duration_since(t).ok())
                .is_some_and(|age| age > STALE_PART);
            if stale && tokio::fs::remove_file(entry.path()).await.is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// An unmounted share leaves an empty local folder behind; writing there would fill the
    /// server's disk and hide the files from the NAS. Refuse instead.
    fn ensure_mounted(&self) -> io::Result<()> {
        if !self.inner.require_mount {
            return Ok(());
        }
        let mut checked = self
            .inner
            .mount_checked
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if checked.is_some_and(|t| t.elapsed() < MOUNT_RECHECK) {
            return Ok(());
        }
        match mount_of(&self.inner.root)? {
            Some(m) if m.mount_point != Path::new("/") => {
                *checked = Some(Instant::now());
                Ok(())
            }
            _ => Err(io::Error::other(format!(
                "STORAGE_DIR {} is not on a mounted share (is the NFS mount up?); refusing to write to the local disk",
                self.inner.root.display()
            ))),
        }
    }
}

/// An upload being written under `.tmp/`. Dropped unfinished, the partial file goes too.
pub struct Upload {
    path: Option<PathBuf>,
    file: Option<tokio::fs::File>,
    hasher: Sha256,
    size: u64,
}

impl Upload {
    pub async fn write(&mut self, chunk: &[u8]) -> io::Result<()> {
        let file = self.file.as_mut().expect("upload is open until finish");
        file.write_all(chunk).await?;
        self.hasher.update(chunk);
        self.size += chunk.len() as u64;
        Ok(())
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    /// Flushes to the server (on NFS, `fsync` is the commit) and closes the file.
    pub async fn finish(mut self) -> io::Result<Finished> {
        let mut file = self.file.take().expect("upload is open until finish");
        file.flush().await?;
        file.sync_all().await?;
        drop(file);
        Ok(Finished {
            path: self.path.take(),
            size: self.size,
            sha256: std::mem::take(&mut self.hasher).finalize().into(),
        })
    }
}

impl Drop for Upload {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// A complete upload not yet in place. Dropped without a commit, it is removed.
pub struct Finished {
    path: Option<PathBuf>,
    pub size: u64,
    pub sha256: [u8; 32],
}

impl Finished {
    fn path(&self) -> &Path {
        self.path.as_deref().expect("finished upload has a path")
    }

    fn disarm(&mut self) {
        self.path = None;
    }
}

impl Drop for Finished {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Keys are relative, `/`-separated, and stay under the root.
pub fn validate_key(key: &str) -> anyhow::Result<()> {
    let ok = !key.is_empty()
        && key.len() <= 1024
        && !key.starts_with('/')
        && !key.contains(['\\', '\0', ':'])
        && key
            .split('/')
            .all(|seg| !seg.is_empty() && seg != "." && seg != "..");
    if ok {
        Ok(())
    } else {
        Err(anyhow!("invalid storage key {key:?}"))
    }
}

pub async fn hash_file(path: &Path) -> io::Result<(u64, [u8; 32])> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut size = 0u64;
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size += n as u64;
    }
    Ok((size, hasher.finalize().into()))
}

async fn same_or_conflict(dest: &Path, sha256: &[u8; 32], key: &str) -> Result<(), CommitError> {
    let (_, existing) = hash_file(dest).await?;
    if &existing == sha256 {
        Ok(())
    } else {
        Err(CommitError::Conflict(key.to_string()))
    }
}

async fn set_read_only(path: &Path, read_only: bool) -> io::Result<()> {
    let mut perms = tokio::fs::metadata(path).await?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Owner, group and the share's readers keep reading; nobody writes.
        perms.set_mode(if read_only { 0o444 } else { 0o644 });
    }
    #[cfg(not(unix))]
    perms.set_readonly(read_only);
    tokio::fs::set_permissions(path, perms).await
}

fn expiry(expires_in: Duration) -> i64 {
    Utc::now().timestamp() + i64::try_from(expires_in.as_secs()).unwrap_or(i64::MAX / 2)
}

fn mac(key: &[u8], context: &[u8], payload: &str) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(context);
    mac.update(payload.as_bytes());
    mac
}

fn sign<T: Serialize>(key: &[u8], context: &[u8], claims: &T) -> String {
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).expect("claims serialize"));
    let signature = URL_SAFE_NO_PAD.encode(mac(key, context, &payload).finalize().into_bytes());
    format!("{payload}.{signature}")
}

fn verify<T: DeserializeOwned>(key: &[u8], context: &[u8], token: &str) -> Result<T, TokenError> {
    let (payload, signature) = token.split_once('.').ok_or(TokenError::Malformed)?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| TokenError::Malformed)?;
    mac(key, context, payload)
        .verify_slice(&signature)
        .map_err(|_| TokenError::BadSignature)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| TokenError::Malformed)?;
    serde_json::from_slice(&bytes).map_err(|_| TokenError::Malformed)
}

/// Base64 sha256 as the PUT token carries it.
pub fn sha256_b64(hash: &[u8]) -> String {
    STANDARD.encode(hash)
}

#[cfg(target_os = "linux")]
pub fn mount_of(path: &Path) -> io::Result<Option<MountInfo>> {
    let path = std::fs::canonicalize(path)?;
    let table = std::fs::read_to_string("/proc/self/mountinfo")?;
    // The deepest mount containing the path; of two at the same point, the later one
    // (it is on top).
    Ok(parse_mountinfo(&table)
        .into_iter()
        .filter(|m| path.starts_with(&m.mount_point))
        .max_by_key(|m| m.mount_point.components().count()))
}

#[cfg(not(target_os = "linux"))]
pub fn mount_of(_path: &Path) -> io::Result<Option<MountInfo>> {
    Ok(None)
}

/// `/proc/self/mountinfo`: `id parent dev root mountpoint options [optional…] - fstype source superoptions`.
pub fn parse_mountinfo(table: &str) -> Vec<MountInfo> {
    table
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(' ').collect();
            let mount_point = fields.get(4)?;
            let dash = fields.iter().position(|f| *f == "-")?;
            Some(MountInfo {
                mount_point: PathBuf::from(unescape_mount(mount_point)),
                fs_type: fields.get(dash + 1)?.to_string(),
                source: unescape_mount(fields.get(dash + 2)?),
            })
        })
        .collect()
}

/// The kernel writes space, tab, newline and backslash as `\040`, `\011`, `\012`, `\134`.
fn unescape_mount(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 3 < bytes.len()
            && bytes[i + 1..i + 4]
                .iter()
                .all(|b| (b'0'..=b'7').contains(b))
        {
            let v = (bytes[i + 1] - b'0') * 64 + (bytes[i + 2] - b'0') * 8 + (bytes[i + 3] - b'0');
            out.push(v);
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(dir: &Path) -> FsStore {
        FsStore::new(
            &FsConfig {
                dir: dir.to_path_buf(),
                require_mount: false,
                intake_lock_years: 10,
            },
            "https://crm.example.com/",
            b"test-key-test-key-test-key-test-key!",
        )
    }

    fn scratch() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "autocrm-fs-{}",
            hex::encode(rand::random::<[u8; 8]>())
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn keys_stay_under_the_root() {
        assert!(validate_key("projektek/2026/2026-0042_Kovacs/fotok/a_1.jpg").is_ok());
        for bad in [
            "",
            "/etc/passwd",
            "a/../../b",
            "a//b",
            "a/./b",
            "C:/x",
            "a\\b",
            "a/",
        ] {
            assert!(validate_key(bad).is_err(), "{bad:?} must be refused");
        }
    }

    #[test]
    fn tokens_bind_their_purpose_and_expire() {
        let dir = scratch();
        let s = store(&dir);
        let put = s
            .presign_put(
                "a/b.jpg",
                "image/jpeg",
                3,
                "AAAA",
                None,
                Duration::from_secs(60),
            )
            .unwrap();
        assert_eq!(put.method, "PUT");
        let token = put.url.rsplit('/').next().unwrap();
        assert!(put.url.starts_with("https://crm.example.com/api/files/"));
        let now = Utc::now().timestamp();
        let claims = s.verify_put(token, now).unwrap();
        assert_eq!((claims.key.as_str(), claims.size), ("a/b.jpg", 3));
        // An upload token is not a download link, and the other way round.
        assert_eq!(s.verify_get(token, now), Err(TokenError::BadSignature));
        let get = s
            .presign_get("a/b.jpg", Duration::from_secs(60), None)
            .unwrap();
        let get_token = get.rsplit('/').next().unwrap();
        assert!(s.verify_get(get_token, now).is_ok());
        assert_eq!(
            s.verify_put(get_token, now).unwrap_err(),
            TokenError::BadSignature
        );
        assert_eq!(s.verify_get(get_token, now + 61), Err(TokenError::Expired));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn uploads_land_whole_and_never_replace_other_bytes() {
        let dir = scratch();
        let s = store(&dir);
        let key = "projektek/2026/x/fotok/bevetel/a_12345678.jpg";

        let mut up = s.begin().await.unwrap();
        up.write(b"first").await.unwrap();
        let done = up.finish().await.unwrap();
        s.commit(done, key, Commit::KeepExisting, true)
            .await
            .unwrap();
        assert_eq!(s.get_bytes(key).await.unwrap(), b"first");
        // Checked on the way in, so completing the upload need not read it back.
        assert_eq!(
            s.head(key).await.unwrap().unwrap().sha256_b64,
            Some(sha256_b64(&Sha256::digest(b"first")))
        );
        assert!(
            std::fs::metadata(s.path(key).unwrap())
                .unwrap()
                .permissions()
                .readonly()
        );

        // The same bytes again: fine. Different bytes: refused, original untouched.
        let mut again = s.begin().await.unwrap();
        again.write(b"first").await.unwrap();
        s.commit(
            again.finish().await.unwrap(),
            key,
            Commit::KeepExisting,
            true,
        )
        .await
        .unwrap();
        let mut other = s.begin().await.unwrap();
        other.write(b"other").await.unwrap();
        let err = s
            .commit(
                other.finish().await.unwrap(),
                key,
                Commit::KeepExisting,
                false,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, CommitError::Conflict(_)));
        assert_eq!(s.get_bytes(key).await.unwrap(), b"first");
        // Nor does a plain write go over write-once evidence.
        assert!(s.put_bytes(key, b"other".to_vec()).await.is_err());

        // Nothing left behind in .tmp, and deleting prunes the emptied folders.
        assert_eq!(std::fs::read_dir(dir.join(TMP_DIR)).unwrap().count(), 0);
        s.delete(key).await.unwrap();
        assert!(!dir.join("projektek").exists());
        assert!(s.head(key).await.unwrap().is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn an_abandoned_upload_leaves_nothing() {
        let dir = scratch();
        let s = store(&dir);
        let mut up = s.begin().await.unwrap();
        up.write(b"half").await.unwrap();
        drop(up);
        assert_eq!(std::fs::read_dir(dir.join(TMP_DIR)).unwrap().count(), 0);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn links_move_without_copying() {
        let dir = scratch();
        let s = store(&dir);
        s.put_bytes("orders/1/intake/abc.jpg", b"photo".to_vec())
            .await
            .unwrap();
        s.link(
            "orders/1/intake/abc.jpg",
            "projektek/2026/p/fotok/bevetel/abc_1.jpg",
        )
        .await
        .unwrap();
        // Linking again onto the same bytes is a no-op.
        s.link(
            "orders/1/intake/abc.jpg",
            "projektek/2026/p/fotok/bevetel/abc_1.jpg",
        )
        .await
        .unwrap();
        s.delete("orders/1/intake/abc.jpg").await.unwrap();
        assert!(!dir.join("orders").exists());
        assert_eq!(
            s.get_bytes("projektek/2026/p/fotok/bevetel/abc_1.jpg")
                .await
                .unwrap(),
            b"photo"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn mountinfo_finds_the_share() {
        let table = "\
22 1 8:1 / / rw,relatime shared:1 - ext4 /dev/sda1 rw
40 22 0:45 / /srv/autocrm\\040files rw,relatime shared:20 - nfs4 nas:/export/autocrm rw,vers=4.1
";
        let mounts = parse_mountinfo(table);
        assert_eq!(mounts.len(), 2);
        assert_eq!(mounts[1].mount_point, PathBuf::from("/srv/autocrm files"));
        assert_eq!(mounts[1].fs_type, "nfs4");
        assert_eq!(mounts[1].source, "nas:/export/autocrm");
    }
}

//! The share's half of presigned requests (`STORAGE_BACKEND=fs`): uploads arrive here instead
//! of at an object store, and downloads are served from here. The token in the path is the
//! only credential, exactly like an S3 presigned URL. With an object store configured these
//! routes answer 404.
//!
//! Mounted outside the 30-second timeout and the gzip layer (`api::router`): a large drawing
//! takes longer than that to arrive, and a video has to seek.

use std::path::PathBuf;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use chrono::Utc;
use futures::StreamExt;
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::AppState;
use crate::media::fs_store::{Commit, CommitError, FsStore, sha256_b64};
use crate::media::layout;

/// An upload that sends nothing for this long is abandoned (its partial file goes with it).
const IDLE: Duration = Duration::from_secs(60);
/// A download link is good for an hour or a week; the browser may keep what it fetched.
const CACHE: &str = "private, max-age=3600";

/// Shown in the browser when asked to. Everything else is a download: a file from the
/// share is served from the app's own address, and an uploaded HTML or SVG page must
/// never run there.
const INLINE: [&str; 12] = [
    "jpg", "jpeg", "png", "webp", "gif", "heic", "pdf", "mp4", "m4v", "mov", "webm", "txt",
];

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/api/files/{token}",
        get(download).head(download).put(upload),
    )
}

fn refuse(status: StatusCode, message: &str) -> Response {
    (status, message.to_string()).into_response()
}

async fn upload(
    State(state): State<AppState>,
    Path(token): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let Some(fs) = state.storage.as_fs() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // 403 like an expired S3 presignature: the phone asks for a new ticket and tries again.
    let claims = match fs.verify_put(&token, Utc::now().timestamp()) {
        Ok(c) => c,
        Err(e) => return refuse(StatusCode::FORBIDDEN, &e.to_string()),
    };
    let declared = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<i64>().ok());
    if declared.is_some_and(|n| n != claims.size) {
        return refuse(
            StatusCode::BAD_REQUEST,
            "Content-Length differs from the size the upload was issued for",
        );
    }

    let mut upload = match fs.begin().await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!(error = %e, "file share: cannot start an upload");
            return refuse(
                StatusCode::SERVICE_UNAVAILABLE,
                "file storage is unavailable",
            );
        }
    };
    let limit = u64::try_from(claims.size).unwrap_or(0);
    let mut stream = body.into_data_stream();
    loop {
        match tokio::time::timeout(IDLE, stream.next()).await {
            Err(_) => return refuse(StatusCode::REQUEST_TIMEOUT, "the upload stalled"),
            Ok(None) => break,
            Ok(Some(Err(e))) => {
                return refuse(
                    StatusCode::BAD_REQUEST,
                    &format!("the upload broke off: {e}"),
                );
            }
            Ok(Some(Ok(chunk))) => {
                if upload.size() + chunk.len() as u64 > limit {
                    return refuse(
                        StatusCode::BAD_REQUEST,
                        "more bytes than the upload was issued for",
                    );
                }
                if let Err(e) = upload.write(&chunk).await {
                    tracing::error!(error = %e, key = %claims.key, "file share: write failed");
                    return refuse(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "file storage is unavailable",
                    );
                }
            }
        }
    }
    let done = match upload.finish().await {
        Ok(d) => d,
        Err(e) => {
            tracing::error!(error = %e, key = %claims.key, "file share: write failed");
            return refuse(
                StatusCode::SERVICE_UNAVAILABLE,
                "file storage is unavailable",
            );
        }
    };
    if done.size != limit || sha256_b64(&done.sha256) != claims.sha256_b64 {
        return refuse(
            StatusCode::BAD_REQUEST,
            "BadDigest: the file differs from the one the upload was issued for",
        );
    }
    match fs
        .commit(done, &claims.key, Commit::KeepExisting, claims.read_only)
        .await
    {
        Ok(()) => StatusCode::OK.into_response(),
        Err(CommitError::Conflict(_)) => refuse(
            StatusCode::CONFLICT,
            "a different file is already stored under this name",
        ),
        Err(CommitError::Io(e)) => {
            tracing::error!(error = %e, key = %claims.key, "file share: could not file the upload");
            refuse(
                StatusCode::SERVICE_UNAVAILABLE,
                "file storage is unavailable",
            )
        }
    }
}

async fn download(
    State(state): State<AppState>,
    Path(token): Path<String>,
    req: Request,
) -> Response {
    let Some(fs) = state.storage.as_fs() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let claims = match fs.verify_get(&token, Utc::now().timestamp()) {
        Ok(c) => c,
        Err(e) => return refuse(StatusCode::FORBIDDEN, &e.to_string()),
    };
    let Some(path) = locate(&state, fs, &claims.key).await else {
        return refuse(StatusCode::NOT_FOUND, "no such file");
    };
    // Range, If-Modified-Since, HEAD and the content type by extension.
    let response = match ServeFile::new(path).oneshot(req).await {
        Ok(r) => r,
        Err(never) => match never {},
    };
    let (mut parts, body) = response.into_parts();
    if parts.status.is_success() || parts.status == StatusCode::NOT_MODIFIED {
        let ext = layout::ext_of(&claims.key);
        let inline_ok = INLINE.contains(&ext.as_str());
        let disposition = match (claims.disposition, inline_ok) {
            (Some(d), true) => Some(d),
            (Some(d), false) => Some(d.replacen("inline", "attachment", 1)),
            (None, true) => None,
            (None, false) => Some("attachment".to_string()),
        };
        let h = &mut parts.headers;
        if let Some(value) = disposition.and_then(|d| HeaderValue::from_str(&d).ok()) {
            h.insert(header::CONTENT_DISPOSITION, value);
        }
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static(CACHE));
        h.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        if !inline_ok {
            h.insert(
                header::CONTENT_SECURITY_POLICY,
                HeaderValue::from_static("default-src 'none'; sandbox"),
            );
        }
    }
    Response::from_parts(parts, Body::new(body))
}

/// The file behind a link; one that re-sorting moved after the link was issued is followed
/// to where it went.
async fn locate(state: &AppState, fs: &FsStore, key: &str) -> Option<PathBuf> {
    let mut key = key.to_string();
    for _ in 0..5 {
        let path = fs.path(&key).ok()?;
        if tokio::fs::metadata(&path).await.is_ok_and(|m| m.is_file()) {
            return Some(path);
        }
        key = sqlx::query_scalar("SELECT new_key FROM storage_moves WHERE old_key = $1")
            .bind(&key)
            .fetch_optional(&state.db)
            .await
            .ok()??;
    }
    None
}

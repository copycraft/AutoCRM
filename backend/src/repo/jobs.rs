//! The Postgres job queue. Claimed with FOR UPDATE SKIP LOCKED; leases on locked_at let
//! jobs from a crashed worker be picked up again.

use chrono::{DateTime, TimeDelta, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::PgExecutor;

/// A job locked longer than this is assumed abandoned (worker crashed) and re-claimable.
/// Must exceed the longest job's runtime.
pub const LEASE: TimeDelta = TimeDelta::minutes(15);

#[derive(Debug, Clone, Serialize)]
pub struct Job {
    pub id: i64,
    pub kind: String,
    pub payload: Value,
    pub dedupe_key: Option<String>,
    pub run_at: DateTime<Utc>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub locked_at: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
    pub failed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Enqueue a job. With a dedupe key, a second unfinished job with the same key is silently
/// skipped and `None` is returned.
pub async fn enqueue(
    db: impl PgExecutor<'_>,
    kind: &str,
    payload: Value,
    run_at: Option<DateTime<Utc>>,
    dedupe_key: Option<&str>,
) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar!(
        "INSERT INTO jobs (kind, payload, run_at, dedupe_key)
         VALUES ($1, $2, coalesce($3, now()), $4)
         ON CONFLICT (dedupe_key) WHERE dedupe_key IS NOT NULL AND completed_at IS NULL AND failed_at IS NULL
         DO NOTHING
         RETURNING id",
        kind,
        payload,
        run_at,
        dedupe_key
    )
    .fetch_optional(db)
    .await
}

/// Whether any job — pending, done or dead — ever used this key. Lets the scheduler run
/// "once per day" jobs exactly once per day.
pub async fn key_used(db: impl PgExecutor<'_>, dedupe_key: &str) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM jobs WHERE dedupe_key = $1) AS "e!""#,
        dedupe_key
    )
    .fetch_one(db)
    .await
}

/// Claims up to `limit` runnable jobs for this worker, incrementing their attempt count.
pub async fn claim(db: impl PgExecutor<'_>, worker_id: &str, limit: i64) -> sqlx::Result<Vec<Job>> {
    let lease_seconds = LEASE.num_seconds() as f64;
    sqlx::query_as!(
        Job,
        "UPDATE jobs
         SET locked_at = now(), locked_by = $1, attempts = attempts + 1
         WHERE id IN (
             SELECT id FROM jobs
             WHERE completed_at IS NULL AND failed_at IS NULL AND run_at <= now()
               AND (locked_at IS NULL OR locked_at < now() - make_interval(secs => $3))
             ORDER BY run_at, id
             FOR UPDATE SKIP LOCKED
             LIMIT $2
         )
         RETURNING id, kind, payload, dedupe_key, run_at, attempts, max_attempts, locked_at, locked_by,
                   completed_at, failed_at, last_error, created_at",
        worker_id,
        limit,
        lease_seconds
    )
    .fetch_all(db)
    .await
}

pub async fn complete(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE jobs SET completed_at = now(), locked_at = NULL, last_error = NULL WHERE id = $1",
        id
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Reschedules after a failure, or dead-letters once attempts are exhausted.
pub async fn fail(
    db: impl PgExecutor<'_>,
    id: i64,
    error: &str,
    retry_at: Option<DateTime<Utc>>,
) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE jobs
         SET locked_at = NULL, locked_by = NULL, last_error = $2,
             run_at = coalesce($3, run_at),
             failed_at = CASE WHEN $3::timestamptz IS NULL THEN now() ELSE NULL END
         WHERE id = $1",
        id,
        error,
        retry_at
    )
    .execute(db)
    .await?;
    Ok(())
}

/// Puts a job back to run later without counting it as a failed attempt.
pub async fn defer(db: impl PgExecutor<'_>, id: i64, run_at: DateTime<Utc>) -> sqlx::Result<()> {
    sqlx::query!(
        "UPDATE jobs SET locked_at = NULL, locked_by = NULL, run_at = $2, attempts = greatest(attempts - 1, 0) WHERE id = $1",
        id,
        run_at
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn list_failed(db: impl PgExecutor<'_>, limit: i64) -> sqlx::Result<Vec<Job>> {
    sqlx::query_as!(
        Job,
        "SELECT id, kind, payload, dedupe_key, run_at, attempts, max_attempts, locked_at, locked_by,
                completed_at, failed_at, last_error, created_at
         FROM jobs WHERE failed_at IS NOT NULL ORDER BY failed_at DESC LIMIT $1",
        limit
    )
    .fetch_all(db)
    .await
}

pub async fn list_pending(db: impl PgExecutor<'_>, limit: i64) -> sqlx::Result<Vec<Job>> {
    sqlx::query_as!(
        Job,
        "SELECT id, kind, payload, dedupe_key, run_at, attempts, max_attempts, locked_at, locked_by,
                completed_at, failed_at, last_error, created_at
         FROM jobs WHERE completed_at IS NULL AND failed_at IS NULL ORDER BY run_at LIMIT $1",
        limit
    )
    .fetch_all(db)
    .await
}

pub async fn count_failed(db: impl PgExecutor<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM jobs WHERE failed_at IS NOT NULL"#)
        .fetch_one(db)
        .await
}

pub async fn count_pending(db: impl PgExecutor<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM jobs WHERE completed_at IS NULL AND failed_at IS NULL"#
    )
    .fetch_one(db)
    .await
}

/// Revives a dead-lettered job with a fresh attempt budget.
pub async fn retry_failed(db: impl PgExecutor<'_>, id: i64) -> sqlx::Result<bool> {
    let r = sqlx::query!(
        "UPDATE jobs SET failed_at = NULL, attempts = 0, run_at = now(), last_error = NULL
         WHERE id = $1 AND failed_at IS NOT NULL",
        id
    )
    .execute(db)
    .await?;
    Ok(r.rows_affected() == 1)
}

/// Exponential backoff: 30s, 1m, 2m, 4m, ... capped at 6h.
pub fn backoff(attempts: i32) -> TimeDelta {
    let exp = attempts.clamp(1, 20) - 1;
    let seconds = 30i64.saturating_mul(1i64 << exp);
    TimeDelta::seconds(seconds.min(6 * 3600))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff(1), TimeDelta::seconds(30));
        assert_eq!(backoff(2), TimeDelta::seconds(60));
        assert_eq!(backoff(4), TimeDelta::seconds(240));
        assert_eq!(backoff(50), TimeDelta::hours(6));
    }
}

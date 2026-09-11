//! Thin wrapper over an S3-compatible object store (MinIO in dev, MinIO/B2 in production).

use std::time::Duration;

use anyhow::{Context, anyhow};
use aws_sdk_s3::Client;
use aws_sdk_s3::config::{
    BehaviorVersion, Credentials, Region, RequestChecksumCalculation, ResponseChecksumValidation,
};
use aws_sdk_s3::error::DisplayErrorContext;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::{ByteStream, DateTime as S3DateTime};
use aws_sdk_s3::types::{
    ChecksumMode, ObjectLockMode, ObjectLockRetention, ObjectLockRetentionMode,
};
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::config::S3Config;

#[derive(Clone)]
pub struct Storage {
    client: Client,
    bucket: String,
    intake_lock_years: u32,
}

/// A request the client must perform exactly as described: every header listed is signed.
#[derive(Debug, Clone, Serialize)]
pub struct PresignedRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub size: i64,
    pub sha256_b64: Option<String>,
}

impl Storage {
    pub fn new(cfg: &S3Config) -> Self {
        let credentials = Credentials::new(
            &cfg.access_key,
            &cfg.secret_key,
            None,
            None,
            "autocrm-config",
        );
        let mut builder = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(cfg.region.clone()))
            .credentials_provider(credentials)
            .force_path_style(cfg.force_path_style)
            // Only send/validate checksums we ask for explicitly; not every S3-compatible
            // store supports the SDK's newer default CRC checksums.
            .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
            .response_checksum_validation(ResponseChecksumValidation::WhenRequired);
        if let Some(endpoint) = &cfg.endpoint {
            builder = builder.endpoint_url(endpoint);
        }
        Storage {
            client: Client::from_conf(builder.build()),
            bucket: cfg.bucket.clone(),
            intake_lock_years: cfg.intake_lock_years,
        }
    }

    /// Retention date for write-once objects, or None if object locking is disabled.
    pub fn intake_lock_until(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        (self.intake_lock_years > 0)
            .then(|| now + chrono::TimeDelta::days(365 * i64::from(self.intake_lock_years) + 2))
    }

    /// Applies governance-mode retention to an existing object (used for migrated intake
    /// photos, which are downloaded before their category is known).
    pub async fn lock_object(&self, key: &str, until: DateTime<Utc>) -> anyhow::Result<()> {
        let retention = ObjectLockRetention::builder()
            .mode(ObjectLockRetentionMode::Governance)
            .retain_until_date(S3DateTime::from_secs(until.timestamp()))
            .build();
        self.client
            .put_object_retention()
            .bucket(&self.bucket)
            .key(key)
            .retention(retention)
            .send()
            .await
            .map_err(|e| anyhow!("locking {key}: {}", DisplayErrorContext(e)))?;
        Ok(())
    }

    /// A PUT bound to exact content: the store rejects any body whose sha256 differs from
    /// the declared one, so a ticket can only ever upload the file it was issued for.
    pub async fn presign_put(
        &self,
        key: &str,
        content_type: &str,
        byte_size: i64,
        sha256_b64: &str,
        lock_until: Option<DateTime<Utc>>,
        expires_in: Duration,
    ) -> anyhow::Result<PresignedRequest> {
        let mut req = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type)
            .content_length(byte_size)
            .checksum_sha256(sha256_b64);
        if let Some(until) = lock_until {
            req = req
                .object_lock_mode(ObjectLockMode::Governance)
                .object_lock_retain_until_date(S3DateTime::from_secs(until.timestamp()));
        }
        let presigned = req
            .presigned(PresigningConfig::expires_in(expires_in)?)
            .await
            .map_err(|e| anyhow!("presigning PUT {key}: {}", DisplayErrorContext(e)))?;
        Ok(PresignedRequest {
            method: presigned.method().to_string(),
            url: presigned.uri().to_string(),
            headers: presigned
                .headers()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        })
    }

    pub async fn presign_get(
        &self,
        key: &str,
        expires_in: Duration,
        content_disposition: Option<String>,
    ) -> anyhow::Result<String> {
        let presigned = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .set_response_content_disposition(content_disposition)
            .presigned(PresigningConfig::expires_in(expires_in)?)
            .await
            .map_err(|e| anyhow!("presigning GET {key}: {}", DisplayErrorContext(e)))?;
        Ok(presigned.uri().to_string())
    }

    pub async fn head(&self, key: &str) -> anyhow::Result<Option<ObjectInfo>> {
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .checksum_mode(ChecksumMode::Enabled)
            .send()
            .await
        {
            Ok(out) => Ok(Some(ObjectInfo {
                size: out.content_length().unwrap_or(0),
                sha256_b64: out.checksum_sha256().map(str::to_string),
            })),
            Err(e) if e.as_service_error().is_some_and(|se| se.is_not_found()) => Ok(None),
            Err(e) => Err(anyhow!("HEAD {key}: {}", DisplayErrorContext(e))),
        }
    }

    pub async fn get_bytes(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        let out = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| anyhow!("GET {key}: {}", DisplayErrorContext(e)))?;
        let bytes = out
            .body
            .collect()
            .await
            .with_context(|| format!("reading body of {key}"))?;
        Ok(bytes.into_bytes().to_vec())
    }

    pub async fn put_bytes(
        &self,
        key: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> anyhow::Result<()> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type)
            .body(ByteStream::from(bytes))
            .send()
            .await
            .map_err(|e| anyhow!("PUT {key}: {}", DisplayErrorContext(e)))?;
        Ok(())
    }

    pub async fn check(&self) -> anyhow::Result<()> {
        self.client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .map_err(|e| anyhow!("bucket {}: {}", self.bucket, DisplayErrorContext(e)))?;
        Ok(())
    }
}

/// `attachment`/`inline` Content-Disposition with a UTF-8-safe filename (RFC 6266 / 5987).
pub fn content_disposition(kind: &str, filename: &str) -> String {
    let ascii: String = filename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded: String = filename
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    format!("{kind}; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disposition_handles_hungarian_filenames() {
        assert_eq!(
            content_disposition("attachment", "terv_ő.pdf"),
            "attachment; filename=\"terv__.pdf\"; filename*=UTF-8''terv_%C5%91.pdf"
        );
        assert_eq!(
            content_disposition("inline", "a\"b.jpg"),
            "inline; filename=\"a_b.jpg\"; filename*=UTF-8''a%22b.jpg"
        );
    }
}

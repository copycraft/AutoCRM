//! Configuration from environment variables. Fails fast, reporting every problem at once.
//! Structs holding secrets deliberately do not derive Debug.

use std::fmt::Display;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

use chrono_tz::Tz;

use crate::domain::email::normalize_address;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEnv {
    Dev,
    Staging,
    Production,
}

impl FromStr for AppEnv {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "dev" => Ok(AppEnv::Dev),
            "staging" => Ok(AppEnv::Staging),
            "production" => Ok(AppEnv::Production),
            _ => Err("expected dev, staging or production".into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Pretty,
    Json,
}

impl FromStr for LogFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pretty" => Ok(LogFormat::Pretty),
            "json" => Ok(LogFormat::Json),
            _ => Err("expected pretty or json".into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtpSecurity {
    None,
    StartTls,
    Tls,
}

impl FromStr for SmtpSecurity {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(SmtpSecurity::None),
            "starttls" => Ok(SmtpSecurity::StartTls),
            "tls" => Ok(SmtpSecurity::Tls),
            _ => Err("expected none, starttls or tls".into()),
        }
    }
}

#[derive(Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub security: SmtpSecurity,
    /// Name announced in EHLO. Google's relay rejects or throttles generic names such as
    /// "localhost", so this defaults to the sending domain.
    pub helo_name: String,
    /// Connect over IPv4 only. Google's relay authorises by source IP; a server that quietly
    /// prefers IPv6 gets "relay denied" when only its IPv4 address is allowlisted.
    pub force_ipv4: bool,
}

#[derive(Clone)]
pub enum EmailTransportConfig {
    /// Writes the log row and the rendered body to the application log; never contacts a server.
    DryRun,
    Smtp(SmtpConfig),
}

#[derive(Clone)]
pub struct EmailConfig {
    pub transport: EmailTransportConfig,
    /// A real, monitored mailbox in the organisation's domain: suppliers reply to nudges.
    pub from_automatic: String,
    pub from_name: String,
    pub reply_to_default: String,
    pub message_id_domain: String,
    /// Domains the SMTP relay may send as. Staff whose address is in one of these send
    /// manual mail as themselves, so replies land straight in their own inbox.
    pub sender_domains: Vec<String>,
    /// Deliver every message to this one address instead of its real recipients. For
    /// staging and for the parallel run before cutover.
    pub redirect_to: Option<String>,
}

#[derive(Clone)]
pub struct S3Config {
    pub endpoint: Option<String>,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub force_path_style: bool,
    pub intake_lock_years: u32,
}

#[derive(Clone)]
pub struct Config {
    pub env: AppEnv,
    pub bind_addr: SocketAddr,
    pub allowed_origins: Vec<String>,
    pub cookie_secure: bool,
    pub public_base_url: String,
    pub database_url: String,
    pub database_max_connections: u32,
    pub upload_signing_key: Vec<u8>,
    pub s3: S3Config,
    pub email: EmailConfig,
    pub mnb_endpoint: String,
    pub business_tz: Tz,
    pub worker_enabled: bool,
    pub worker_id: String,
    pub log_format: LogFormat,
    pub log_dir: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid configuration:\n  - {}", .0.join("\n  - "))]
pub struct ConfigError(pub Vec<String>);

/// Hosts that non-production environments may deliver SMTP mail to without redirection.
const LOCAL_SMTP_HOSTS: [&str; 4] = ["localhost", "127.0.0.1", "::1", "mailpit"];

pub fn domain_of(address: &str) -> Option<String> {
    address
        .rsplit_once('@')
        .map(|(_, d)| d.trim().trim_end_matches('>').to_ascii_lowercase())
        .filter(|d| !d.is_empty())
}

/// Outside production, SMTP mail may only reach a local sink, or every message must be
/// redirected to one internal address. Test nudges must never reach real customers.
pub fn check_smtp_target(env: AppEnv, host: &str, redirect_to: Option<&str>) -> Result<(), String> {
    if env == AppEnv::Production || LOCAL_SMTP_HOSTS.contains(&host) || redirect_to.is_some() {
        return Ok(());
    }
    Err(format!(
        "EMAIL_MODE=smtp outside production must target a local sink ({}) or set EMAIL_REDIRECT_TO; got '{host}'. \
         Test mail must never reach real customers.",
        LOCAL_SMTP_HOSTS.join(", ")
    ))
}

/// With a relay restricted to the organisation's domains, a sender outside them is rejected
/// at send time. Catch that at startup instead.
pub fn check_sender_domains(sender_domains: &[String], addresses: &[(&str, &str)]) -> Vec<String> {
    if sender_domains.is_empty() {
        return Vec::new();
    }
    addresses
        .iter()
        .filter(|(_, addr)| !domain_of(addr).is_some_and(|d| sender_domains.contains(&d)))
        .map(|(key, addr)| {
            format!("{key} ({addr}) is not in EMAIL_SENDER_DOMAINS; the relay would reject it")
        })
        .collect()
}

struct Reader {
    errors: Vec<String>,
}

impl Reader {
    fn var(key: &str) -> Option<String> {
        std::env::var(key)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    }

    fn required(&mut self, key: &str) -> String {
        Self::var(key).unwrap_or_else(|| {
            self.errors.push(format!("{key} is required"));
            String::new()
        })
    }

    fn optional(&self, key: &str) -> Option<String> {
        Self::var(key)
    }

    fn parsed<T>(&mut self, key: &str, default: T) -> T
    where
        T: FromStr,
        T::Err: Display,
    {
        match Self::var(key) {
            None => default,
            Some(raw) => raw.parse().unwrap_or_else(|e| {
                self.errors
                    .push(format!("{key}: invalid value '{raw}': {e}"));
                default
            }),
        }
    }

    fn required_parsed<T>(&mut self, key: &str, fallback: T) -> T
    where
        T: FromStr,
        T::Err: Display,
    {
        if Self::var(key).is_none() {
            self.errors.push(format!("{key} is required"));
            return fallback;
        }
        self.parsed(key, fallback)
    }

    fn address(&mut self, key: &str, value: &str) {
        if !value.is_empty() && normalize_address(value).is_none() {
            self.errors
                .push(format!("{key}: '{value}' is not a valid email address"));
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Config, ConfigError> {
        let mut r = Reader { errors: Vec::new() };

        let env: AppEnv = r.required_parsed("APP_ENV", AppEnv::Dev);
        let bind_addr = r.parsed("BIND_ADDR", SocketAddr::from(([127, 0, 0, 1], 8080)));
        let allowed_origins: Vec<String> = r
            .optional("ALLOWED_ORIGINS")
            .map(|v| {
                v.split(',')
                    .map(|o| o.trim().trim_end_matches('/').to_string())
                    .filter(|o| !o.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let cookie_secure = r.parsed("COOKIE_SECURE", true);
        let public_base_url = r
            .required("PUBLIC_BASE_URL")
            .trim_end_matches('/')
            .to_string();

        let database_url = r.required("DATABASE_URL");
        let database_max_connections = r.parsed("DATABASE_MAX_CONNECTIONS", 10u32);

        let upload_signing_key = r.required("UPLOAD_SIGNING_KEY").into_bytes();
        if !upload_signing_key.is_empty() && upload_signing_key.len() < 32 {
            r.errors
                .push("UPLOAD_SIGNING_KEY must be at least 32 bytes".into());
        }

        let s3 = S3Config {
            endpoint: r.optional("S3_ENDPOINT"),
            region: r
                .optional("S3_REGION")
                .unwrap_or_else(|| "us-east-1".into()),
            bucket: r.required("S3_BUCKET"),
            access_key: r.required("S3_ACCESS_KEY"),
            secret_key: r.required("S3_SECRET_KEY"),
            force_path_style: r.parsed("S3_FORCE_PATH_STYLE", false),
            intake_lock_years: r.parsed("S3_INTAKE_LOCK_YEARS", 10u32),
        };

        let from_automatic = r.required("EMAIL_FROM_AUTOMATIC");
        let reply_to_default = r.required("EMAIL_REPLY_TO_DEFAULT");
        r.address("EMAIL_FROM_AUTOMATIC", &from_automatic);
        r.address("EMAIL_REPLY_TO_DEFAULT", &reply_to_default);
        let from_name = r
            .optional("EMAIL_FROM_NAME")
            .unwrap_or_else(|| "Autotherm".into());
        let message_id_domain = r
            .optional("EMAIL_MESSAGE_ID_DOMAIN")
            .or_else(|| domain_of(&from_automatic))
            .unwrap_or_else(|| "autocrm.local".into());
        let sender_domains: Vec<String> = r
            .optional("EMAIL_SENDER_DOMAINS")
            .map(|v| {
                v.split(',')
                    .map(|d| d.trim().to_ascii_lowercase())
                    .filter(|d| !d.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        let redirect_to = match r.optional("EMAIL_REDIRECT_TO") {
            Some(raw) => {
                let normalized = normalize_address(&raw);
                if normalized.is_none() {
                    r.errors.push(format!(
                        "EMAIL_REDIRECT_TO: '{raw}' is not a valid email address"
                    ));
                }
                normalized
            }
            None => None,
        };
        let sender_errors = check_sender_domains(
            &sender_domains,
            &[("EMAIL_FROM_AUTOMATIC", &from_automatic)],
        );
        r.errors.extend(sender_errors);

        let mode = r.optional("EMAIL_MODE").unwrap_or_else(|| "dry_run".into());
        let transport = match mode.as_str() {
            "dry_run" => EmailTransportConfig::DryRun,
            "smtp" => {
                let host = r.required("SMTP_HOST");
                if !host.is_empty() {
                    if let Err(e) = check_smtp_target(env, &host, redirect_to.as_deref()) {
                        r.errors.push(e);
                    }
                }
                let helo_name = r
                    .optional("SMTP_HELO_NAME")
                    .unwrap_or_else(|| message_id_domain.clone());
                if helo_name.contains(char::is_whitespace) {
                    r.errors
                        .push("SMTP_HELO_NAME must be a bare domain name".into());
                }
                EmailTransportConfig::Smtp(SmtpConfig {
                    host,
                    port: r.parsed("SMTP_PORT", 587u16),
                    username: r.optional("SMTP_USERNAME"),
                    password: r.optional("SMTP_PASSWORD"),
                    security: r.parsed("SMTP_SECURITY", SmtpSecurity::StartTls),
                    helo_name,
                    force_ipv4: r.parsed("SMTP_FORCE_IPV4", false),
                })
            }
            other => {
                r.errors.push(format!(
                    "EMAIL_MODE: expected dry_run or smtp, got '{other}'"
                ));
                EmailTransportConfig::DryRun
            }
        };
        let email = EmailConfig {
            transport,
            from_automatic,
            from_name,
            reply_to_default,
            message_id_domain,
            sender_domains,
            redirect_to,
        };

        let mnb_endpoint = r
            .optional("MNB_ENDPOINT")
            .unwrap_or_else(|| "http://www.mnb.hu/arfolyamok.asmx".into());
        let business_tz = r.parsed("BUSINESS_TIMEZONE", chrono_tz::Europe::Budapest);
        let worker_enabled = r.parsed("WORKER_ENABLED", true);
        let worker_id = r
            .optional("WORKER_ID")
            .unwrap_or_else(|| format!("worker-{}", std::process::id()));
        let log_format = r.parsed("LOG_FORMAT", LogFormat::Pretty);
        let log_dir = r.optional("LOG_DIR").map(PathBuf::from);

        if env == AppEnv::Production {
            if !cookie_secure {
                r.errors
                    .push("COOKIE_SECURE must be true in production".into());
            }
            if upload_signing_key.starts_with(b"dev-only") {
                r.errors
                    .push("UPLOAD_SIGNING_KEY still has the development value".into());
            }
            if !public_base_url.starts_with("https://") {
                r.errors
                    .push("PUBLIC_BASE_URL must be https in production".into());
            }
        }

        if !r.errors.is_empty() {
            return Err(ConfigError(r.errors));
        }
        Ok(Config {
            env,
            bind_addr,
            allowed_origins,
            cookie_secure,
            public_base_url,
            database_url,
            database_max_connections,
            upload_signing_key,
            s3,
            email,
            mnb_endpoint,
            business_tz,
            worker_enabled,
            worker_id,
            log_format,
            log_dir,
        })
    }

    pub fn is_production(&self) -> bool {
        self.env == AppEnv::Production
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_smtp_outside_production_needs_a_redirect() {
        assert!(check_smtp_target(AppEnv::Dev, "smtp-relay.gmail.com", None).is_err());
        assert!(check_smtp_target(AppEnv::Staging, "smtp-relay.gmail.com", None).is_err());
        assert!(
            check_smtp_target(
                AppEnv::Staging,
                "smtp-relay.gmail.com",
                Some("teszt@autotherm.hu")
            )
            .is_ok()
        );
        assert!(check_smtp_target(AppEnv::Dev, "localhost", None).is_ok());
        assert!(check_smtp_target(AppEnv::Production, "smtp-relay.gmail.com", None).is_ok());
    }

    #[test]
    fn automatic_sender_must_be_in_relay_domains() {
        let domains = vec!["autotherm.hu".to_string()];
        assert!(check_sender_domains(&domains, &[("FROM", "beszerzes@autotherm.hu")]).is_empty());
        assert_eq!(
            check_sender_domains(&domains, &[("FROM", "noreply@gmail.com")]).len(),
            1
        );
        assert!(check_sender_domains(&[], &[("FROM", "noreply@gmail.com")]).is_empty());
    }

    #[test]
    fn domains_are_lowercased() {
        assert_eq!(
            domain_of("Kovacs@Autotherm.HU").as_deref(),
            Some("autotherm.hu")
        );
        assert_eq!(domain_of("nope"), None);
    }
}

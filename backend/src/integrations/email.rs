//! Delivering a rendered email over SMTP — in production, Google Workspace's SMTP relay —
//! or, in dry-run mode, only logging it. Deciding *whether* to send lives in domain::email
//! and service::email; this module only speaks SMTP.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use lettre::message::header::{ContentType, Header, HeaderName, HeaderValue};
use lettre::message::{Attachment, Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::transport::smtp::extension::ClientId;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::config::{EmailConfig, EmailTransportConfig, SmtpConfig, SmtpSecurity};
use crate::domain::template::escape_html;

pub struct OutgoingAttachment {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

pub struct OutgoingEmail {
    pub message_id: String,
    pub from: String,
    pub reply_to: Option<String>,
    pub to: String,
    pub cc: Vec<String>,
    pub subject: String,
    pub body_text: String,
    pub body_html: String,
    pub attachments: Vec<OutgoingAttachment>,
    /// Marks the message as machine-generated so vacation responders don't answer it.
    pub automatic: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum SendError {
    /// The message can never be delivered as-is (bad address, 5xx). Don't retry.
    #[error("permanent: {0}")]
    Permanent(String),
    /// Nothing was accepted by the server (connection refused, 4xx). Safe to retry.
    #[error("transient: {0}")]
    Transient(String),
    /// The server may or may not have accepted the message (timeout mid-conversation).
    /// Retrying risks a duplicate, so a human decides.
    #[error("delivery unknown: {0}")]
    Unknown(String),
}

/// RFC 3834: tells mail systems this is automated, so out-of-office replies are suppressed.
#[derive(Debug, Clone)]
struct AutoSubmitted;

impl Header for AutoSubmitted {
    fn name() -> HeaderName {
        HeaderName::new_from_ascii_str("Auto-Submitted")
    }
    fn parse(_: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(AutoSubmitted)
    }
    fn display(&self) -> HeaderValue {
        HeaderValue::new(Self::name(), "auto-generated".to_owned())
    }
}

/// The Exchange/Outlook equivalent, for customers and suppliers not on Gmail.
#[derive(Debug, Clone)]
struct AutoResponseSuppress;

impl Header for AutoResponseSuppress {
    fn name() -> HeaderName {
        HeaderName::new_from_ascii_str("X-Auto-Response-Suppress")
    }
    fn parse(_: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(AutoResponseSuppress)
    }
    fn display(&self) -> HeaderValue {
        HeaderValue::new(Self::name(), "All".to_owned())
    }
}

#[derive(Clone)]
pub enum Mailer {
    DryRun,
    Smtp(SmtpMailer),
}

#[derive(Clone)]
pub struct SmtpMailer {
    config: SmtpConfig,
    redirect_to: Option<String>,
    /// Pooled transport built once. None with `force_ipv4`: the IPv4 address is resolved on
    /// every send so a changed DNS answer is picked up (at ~40 mails a day that costs nothing).
    pooled: Option<Arc<AsyncSmtpTransport<Tokio1Executor>>>,
}

fn build_transport(
    cfg: &SmtpConfig,
    connect_to: &str,
) -> anyhow::Result<AsyncSmtpTransport<Tokio1Executor>> {
    // TLS always verifies the certificate against the configured host name, even when we
    // connect to a resolved IP address.
    let tls = match cfg.security {
        SmtpSecurity::None => Tls::None,
        SmtpSecurity::StartTls => Tls::Required(TlsParameters::new(cfg.host.clone())?),
        SmtpSecurity::Tls => Tls::Wrapper(TlsParameters::new(cfg.host.clone())?),
    };
    let mut builder = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(connect_to)
        .port(cfg.port)
        .tls(tls)
        .hello_name(ClientId::Domain(cfg.helo_name.clone()))
        .timeout(Some(Duration::from_secs(60)));
    if let (Some(user), Some(pass)) = (&cfg.username, &cfg.password) {
        builder = builder.credentials(Credentials::new(user.clone(), pass.clone()));
    }
    Ok(builder.build())
}

async fn resolve_ipv4(host: &str, port: u16) -> anyhow::Result<IpAddr> {
    tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| anyhow!("resolving {host}: {e}"))?
        .find(|addr| addr.is_ipv4())
        .map(|addr| addr.ip())
        .ok_or_else(|| anyhow!("{host} has no IPv4 address"))
}

impl SmtpMailer {
    async fn transport(&self) -> Result<Arc<AsyncSmtpTransport<Tokio1Executor>>, SendError> {
        if let Some(pooled) = &self.pooled {
            return Ok(pooled.clone());
        }
        let ip = resolve_ipv4(&self.config.host, self.config.port)
            .await
            .map_err(|e| SendError::Transient(with_hint(&e.to_string())))?;
        build_transport(&self.config, &ip.to_string())
            .map(Arc::new)
            .map_err(|e| SendError::Permanent(e.to_string()))
    }
}

impl Mailer {
    pub fn from_config(cfg: &EmailConfig) -> anyhow::Result<Mailer> {
        match &cfg.transport {
            EmailTransportConfig::DryRun => Ok(Mailer::DryRun),
            EmailTransportConfig::Smtp(smtp) => {
                let pooled = if smtp.force_ipv4 {
                    None
                } else {
                    Some(Arc::new(build_transport(smtp, &smtp.host)?))
                };
                Ok(Mailer::Smtp(SmtpMailer {
                    config: smtp.clone(),
                    redirect_to: cfg.redirect_to.clone(),
                    pooled,
                }))
            }
        }
    }

    pub fn is_dry_run(&self) -> bool {
        matches!(self, Mailer::DryRun)
    }

    pub fn redirect_to(&self) -> Option<&str> {
        match self {
            Mailer::Smtp(smtp) => smtp.redirect_to.as_deref(),
            Mailer::DryRun => None,
        }
    }

    /// Connects, says EHLO (and STARTTLS/AUTH as configured) and disconnects without sending.
    pub async fn test_connection(&self) -> Result<(), String> {
        let Mailer::Smtp(smtp) = self else {
            return Ok(());
        };
        let transport = smtp.transport().await.map_err(|e| e.to_string())?;
        match transport.test_connection().await {
            Ok(true) => Ok(()),
            Ok(false) => Err("the server did not accept the connection".into()),
            Err(e) => Err(with_hint(&e.to_string())),
        }
    }

    pub async fn send(&self, mut email: OutgoingEmail) -> Result<(), SendError> {
        match self {
            Mailer::DryRun => {
                build_message(&email).map_err(|e| SendError::Permanent(e.to_string()))?;
                tracing::info!(
                    message_id = %email.message_id,
                    from = %email.from,
                    to = %email.to,
                    cc = ?email.cc,
                    subject = %email.subject,
                    automatic = email.automatic,
                    attachments = email.attachments.len(),
                    body = %email.body_text,
                    "DRY RUN: email not sent"
                );
                Ok(())
            }
            Mailer::Smtp(smtp) => {
                if let Some(redirect_to) = &smtp.redirect_to {
                    tracing::info!(original_to = %email.to, %redirect_to, "redirecting email");
                    apply_redirect(&mut email, redirect_to);
                }
                let message =
                    build_message(&email).map_err(|e| SendError::Permanent(e.to_string()))?;
                let transport = smtp.transport().await?;
                transport
                    .send(message)
                    .await
                    .map(|_| ())
                    .map_err(|e| classify(&e))
            }
        }
    }
}

fn classify(e: &lettre::transport::smtp::Error) -> SendError {
    let text = with_hint(&e.to_string());
    if e.is_permanent() {
        SendError::Permanent(text)
    } else if e.is_transient() {
        SendError::Transient(text)
    } else if e.is_timeout() {
        SendError::Unknown(text)
    } else {
        SendError::Transient(text)
    }
}

/// Rewrites a message so it reaches only `redirect_to`, stating clearly who it was meant for.
pub fn apply_redirect(email: &mut OutgoingEmail, redirect_to: &str) {
    let mut original = email.to.clone();
    if !email.cc.is_empty() {
        original = format!("{original}, cc: {}", email.cc.join(", "));
    }
    let notice = format!("TESZT – eredeti címzett: {original}");
    email.subject = format!("[{notice}] {}", email.subject);
    email.body_text = format!("*** {notice} ***\n\n{}", email.body_text);
    let banner = format!(
        "<p style=\"background:#fff3cd;border:1px solid #e0c36c;padding:8px\"><strong>{}</strong></p>\n",
        escape_html(&notice)
    );
    let insert_at = email.body_html.find("<body").and_then(|start| {
        email.body_html[start..]
            .find('>')
            .map(|end| start + end + 1)
    });
    match insert_at {
        Some(pos) => email.body_html.insert_str(pos, &banner),
        None => email.body_html.insert_str(0, &banner),
    }
    email.to = redirect_to.to_string();
    email.cc.clear();
}

/// Plain-language hints for the errors Google Workspace's SMTP relay commonly returns, so
/// the email log says what to fix rather than only an SMTP code.
pub fn diagnose(error: &str) -> Option<&'static str> {
    let e = error.to_ascii_lowercase();
    if e.contains("5.7.0") && (e.contains("relay") || e.contains("denied")) {
        Some(
            "Google refused to relay: the server's public IP is not in the SMTP relay allowlist (check its IPv6 address too, or set SMTP_FORCE_IPV4=true), or the From address is outside the allowed domains",
        )
    } else if e.contains("5.7.1") {
        Some(
            "sender not permitted: with 'Only registered Apps users' the From address must be an existing Workspace user or alias",
        )
    } else if e.contains("535") || e.contains("5.7.8") || e.contains("authentication") {
        Some(
            "authentication failed: check SMTP_USERNAME/SMTP_PASSWORD, or use the IP allowlist without authentication",
        )
    } else if e.contains("421") || e.contains("4.7.0") {
        Some(
            "temporarily refused: Google may be rate limiting, or rejecting the EHLO name (set SMTP_HELO_NAME to your domain); it will be retried",
        )
    } else if e.contains("certificate") {
        Some("TLS certificate verification failed: check SMTP_HOST is the server's real name")
    } else if e.contains("refused")
        || e.contains("timed out")
        || e.contains("resolv")
        || e.contains("unreachable")
    {
        Some(
            "could not reach the SMTP server: check SMTP_HOST/SMTP_PORT and that the hosting provider does not block outbound port 587",
        )
    } else {
        None
    }
}

fn with_hint(error: &str) -> String {
    match diagnose(error) {
        Some(hint) => format!("{error} — {hint}"),
        None => error.to_string(),
    }
}

fn mailbox(s: &str) -> anyhow::Result<Mailbox> {
    s.parse::<Mailbox>()
        .map_err(|e| anyhow!("invalid address '{s}': {e}"))
}

fn build_message(email: &OutgoingEmail) -> anyhow::Result<Message> {
    let mut builder = Message::builder()
        .message_id(Some(email.message_id.clone()))
        .from(mailbox(&email.from)?)
        .to(mailbox(&email.to)?)
        .subject(email.subject.clone());
    if let Some(reply_to) = &email.reply_to {
        builder = builder.reply_to(mailbox(reply_to)?);
    }
    for cc in &email.cc {
        builder = builder.cc(mailbox(cc)?);
    }
    if email.automatic {
        builder = builder.header(AutoSubmitted).header(AutoResponseSuppress);
    }
    let alternative =
        MultiPart::alternative_plain_html(email.body_text.clone(), email.body_html.clone());
    let body = if email.attachments.is_empty() {
        alternative
    } else {
        let mut mixed = MultiPart::mixed().multipart(alternative);
        for a in &email.attachments {
            let content_type = ContentType::parse(&a.content_type)
                .unwrap_or_else(|_| ContentType::parse("application/octet-stream").expect("valid"));
            mixed = mixed.singlepart(
                Attachment::new(a.filename.clone()).body(a.bytes.clone(), content_type),
            );
        }
        mixed
    };
    Ok(builder.multipart(body)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> OutgoingEmail {
        OutgoingEmail {
            message_id: "<email-1@autotherm.hu>".into(),
            from: "beszerzes@autotherm.hu".into(),
            reply_to: Some("iroda@autotherm.hu".into()),
            to: "michael@mueller.example".into(),
            cc: vec!["anna@mueller.example".into()],
            subject: "Emlékeztető: ATP tanúsítvány".into(),
            body_text: "Szia".into(),
            body_html: "<!doctype html><html><body style=\"x\">\n<p>Szia</p></body></html>".into(),
            attachments: vec![OutgoingAttachment {
                filename: "terv.pdf".into(),
                content_type: "application/pdf".into(),
                bytes: b"%PDF".to_vec(),
            }],
            automatic: true,
        }
    }

    fn formatted(email: &OutgoingEmail) -> String {
        String::from_utf8(build_message(email).unwrap().formatted()).unwrap()
    }

    #[test]
    fn builds_multipart_message_with_both_parts() {
        let text = formatted(&sample());
        assert!(text.contains("Message-ID: <email-1@autotherm.hu>"));
        assert!(text.contains("text/plain"));
        assert!(text.contains("text/html"));
        assert!(text.contains("terv.pdf"));
    }

    #[test]
    fn automatic_mail_suppresses_auto_replies_manual_does_not() {
        let text = formatted(&sample());
        assert!(text.contains("Auto-Submitted: auto-generated"));
        assert!(text.contains("X-Auto-Response-Suppress: All"));
        let mut manual = sample();
        manual.automatic = false;
        assert!(!formatted(&manual).contains("Auto-Submitted"));
    }

    #[test]
    fn bad_address_is_rejected() {
        let mut e = sample();
        e.to = "not an address".into();
        assert!(build_message(&e).is_err());
    }

    #[test]
    fn redirect_keeps_the_original_recipients_visible() {
        let mut e = sample();
        apply_redirect(&mut e, "teszt@autotherm.hu");
        assert_eq!(e.to, "teszt@autotherm.hu");
        assert!(e.cc.is_empty());
        assert!(e.subject.starts_with(
            "[TESZT – eredeti címzett: michael@mueller.example, cc: anna@mueller.example]"
        ));
        assert!(e.body_text.starts_with("*** TESZT"));
        assert!(
            e.body_html
                .contains("<body style=\"x\"><p style=\"background:#fff3cd"),
            "{}",
            e.body_html
        );
    }

    #[test]
    fn google_relay_errors_get_hints() {
        assert!(
            diagnose("permanent error (550): 5.7.0 Mail relay denied [203.0.113.7]")
                .unwrap()
                .contains("allowlist")
        );
        assert!(
            diagnose("permanent error (535): 5.7.8 Username and Password not accepted")
                .unwrap()
                .contains("authentication")
        );
        assert!(
            diagnose("transient error (421): 4.7.0 Try again later")
                .unwrap()
                .contains("retried")
        );
        assert!(
            diagnose("Connection refused (os error 111)")
                .unwrap()
                .contains("reach")
        );
        assert_eq!(
            diagnose("permanent error (550): 5.1.1 The email account does not exist"),
            None
        );
    }

    #[tokio::test]
    async fn dry_run_never_fails_for_valid_mail() {
        assert!(Mailer::DryRun.send(sample()).await.is_ok());
    }
}

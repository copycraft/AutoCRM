//! SMTP delivery against an in-process fake SMTP server: what goes over the wire, how
//! relay rejections are classified, redirection, IPv4-only mode, and staff sending as
//! themselves end to end through the queue.

mod common;

use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use autocrm::AppState;
use autocrm::config::{EmailConfig, EmailTransportConfig, SmtpConfig, SmtpSecurity};
use autocrm::domain::email::EmailStatus;
use autocrm::domain::role::Role;
use autocrm::integrations::email::{Mailer, OutgoingEmail, SendError};
use autocrm::repo::emails;
use autocrm::repo::sessions::SessionKind;
use autocrm::repo::users;
use autocrm::service::auth::AuthUser;
use autocrm::service::email::{ComposeRequest, Delivery, deliver, send_manual};

#[derive(Debug, Clone, Default)]
struct Received {
    ehlo: String,
    mail_from: String,
    rcpt_to: Vec<String>,
    data: String,
}

#[derive(Clone, Copy)]
enum Behaviour {
    Accept,
    /// What Google's relay answers when the source IP isn't allowlisted.
    DenyRelay,
    TryLater,
}

async fn fake_smtp(behaviour: Behaviour) -> (u16, mpsc::UnboundedReceiver<Received>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let tx = tx.clone();
            tokio::spawn(async move {
                let (read, mut write) = socket.into_split();
                let mut reader = BufReader::new(read);
                let mut current = Received::default();
                write.write_all(b"220 fake.smtp ESMTP ready\r\n").await.ok();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                        break;
                    }
                    let upper = line.to_ascii_uppercase();
                    let reply = if upper.starts_with("EHLO") || upper.starts_with("HELO") {
                        current.ehlo = line.trim().to_string();
                        "250 fake.smtp\r\n"
                    } else if upper.starts_with("MAIL FROM:") {
                        current.mail_from = line.trim()[10..].to_string();
                        match behaviour {
                            Behaviour::TryLater => {
                                "421 4.7.0 Try again later, closing connection.\r\n"
                            }
                            _ => "250 2.1.0 OK\r\n",
                        }
                    } else if upper.starts_with("RCPT TO:") {
                        current.rcpt_to.push(line.trim()[8..].to_string());
                        match behaviour {
                            Behaviour::DenyRelay => {
                                "550 5.7.0 Mail relay denied [203.0.113.7]. Invalid credentials for relay.\r\n"
                            }
                            _ => "250 2.1.5 OK\r\n",
                        }
                    } else if upper.starts_with("DATA") {
                        write.write_all(b"354 Go ahead\r\n").await.ok();
                        let mut data = String::new();
                        loop {
                            let mut l = String::new();
                            if reader.read_line(&mut l).await.unwrap_or(0) == 0 || l == ".\r\n" {
                                break;
                            }
                            data.push_str(&l);
                        }
                        current.data = data;
                        tx.send(current.clone()).ok();
                        current.mail_from.clear();
                        current.rcpt_to.clear();
                        "250 2.0.0 OK queued\r\n"
                    } else if upper.starts_with("QUIT") {
                        write.write_all(b"221 2.0.0 bye\r\n").await.ok();
                        break;
                    } else if upper.starts_with("RSET") || upper.starts_with("NOOP") {
                        "250 2.0.0 OK\r\n"
                    } else {
                        "502 5.5.1 Unrecognized command\r\n"
                    };
                    write.write_all(reply.as_bytes()).await.ok();
                    if reply.starts_with("421") {
                        break;
                    }
                }
            });
        }
    });
    (port, rx)
}

fn email_config(port: u16, host: &str, force_ipv4: bool, redirect_to: Option<&str>) -> EmailConfig {
    EmailConfig {
        transport: EmailTransportConfig::Smtp(SmtpConfig {
            host: host.into(),
            port,
            username: None,
            password: None,
            security: SmtpSecurity::None,
            helo_name: "autotherm.hu".into(),
            force_ipv4,
        }),
        from_automatic: "beszerzes@autotherm.hu".into(),
        from_name: "Autotherm".into(),
        reply_to_default: "iroda@autotherm.hu".into(),
        message_id_domain: "autotherm.hu".into(),
        sender_domains: vec!["autotherm.hu".into()],
        redirect_to: redirect_to.map(str::to_string),
    }
}

fn nudge() -> OutgoingEmail {
    OutgoingEmail {
        message_id: "<email-7.abc@autotherm.hu>".into(),
        from: "Autotherm <beszerzes@autotherm.hu>".into(),
        reply_to: Some("iroda@autotherm.hu".into()),
        to: "michael@supplier.example".into(),
        cc: vec!["anna@supplier.example".into()],
        bcc: vec![],
        subject: "Reminder: ATP certificate".into(),
        body_text: "Please send the ATP certificate.".into(),
        body_html: "<html><body><p>Please send the ATP certificate.</p></body></html>".into(),
        attachments: vec![],
        automatic: true,
    }
}

async fn next(rx: &mut mpsc::UnboundedReceiver<Received>) -> Received {
    tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await
        .expect("fake server received nothing")
        .unwrap()
}

#[tokio::test]
async fn delivers_with_domain_ehlo_envelope_and_auto_reply_suppression() {
    let (port, mut rx) = fake_smtp(Behaviour::Accept).await;
    let mailer = Mailer::from_config(&email_config(port, "127.0.0.1", false, None)).unwrap();
    mailer.test_connection().await.unwrap();
    mailer.send(nudge()).await.unwrap();

    let got = next(&mut rx).await;
    assert_eq!(got.ehlo, "EHLO autotherm.hu");
    assert!(
        got.mail_from.contains("<beszerzes@autotherm.hu>"),
        "{}",
        got.mail_from
    );
    assert_eq!(got.rcpt_to.len(), 2);
    assert!(got.data.contains("Message-ID: <email-7.abc@autotherm.hu>"));
    assert!(got.data.contains("Reply-To: iroda@autotherm.hu"));
    assert!(got.data.contains("Auto-Submitted: auto-generated"));
    assert!(got.data.contains("X-Auto-Response-Suppress: All"));
}

#[tokio::test]
async fn redirect_delivers_only_to_the_test_inbox() {
    let (port, mut rx) = fake_smtp(Behaviour::Accept).await;
    let mailer = Mailer::from_config(&email_config(
        port,
        "127.0.0.1",
        false,
        Some("teszt@autotherm.hu"),
    ))
    .unwrap();
    mailer.send(nudge()).await.unwrap();

    // The envelope is what decides delivery: only the test inbox, never the original to/cc.
    let got = next(&mut rx).await;
    assert_eq!(got.rcpt_to, vec!["<teszt@autotherm.hu>".to_string()]);
    assert!(!got.data.contains("Cc:"), "{}", got.data);
}

#[tokio::test]
async fn relay_denied_is_permanent_and_explained() {
    let (port, _rx) = fake_smtp(Behaviour::DenyRelay).await;
    let mailer = Mailer::from_config(&email_config(port, "127.0.0.1", false, None)).unwrap();
    match mailer.send(nudge()).await {
        Err(SendError::Permanent(message)) => {
            assert!(message.contains("5.7.0"), "{message}");
            assert!(message.contains("allowlist"), "{message}");
        }
        other => panic!("expected permanent failure, got {other:?}"),
    }
}

#[tokio::test]
async fn temporary_refusal_is_retryable() {
    let (port, _rx) = fake_smtp(Behaviour::TryLater).await;
    let mailer = Mailer::from_config(&email_config(port, "127.0.0.1", false, None)).unwrap();
    assert!(matches!(
        mailer.send(nudge()).await,
        Err(SendError::Transient(_))
    ));
}

#[tokio::test]
async fn ipv4_only_mode_resolves_the_host_name() {
    let (port, mut rx) = fake_smtp(Behaviour::Accept).await;
    // "localhost" may resolve to ::1 first; the fake server only listens on 127.0.0.1.
    let mailer = Mailer::from_config(&email_config(port, "localhost", true, None)).unwrap();
    mailer.send(nudge()).await.unwrap();
    assert_eq!(next(&mut rx).await.ehlo, "EHLO autotherm.hu");
}

#[tokio::test]
async fn unreachable_server_fails_the_connection_test_with_a_hint() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let mailer = Mailer::from_config(&email_config(port, "127.0.0.1", false, None)).unwrap();
    let error = mailer.test_connection().await.unwrap_err();
    assert!(error.contains("could not reach"), "{error}");
}

#[sqlx::test(migrations = "./migrations")]
async fn staff_send_as_themselves_through_the_queue(pool: PgPool) {
    let (port, mut rx) = fake_smtp(Behaviour::Accept).await;
    let smtp_config = email_config(port, "127.0.0.1", false, None);
    let mut config = common::config();
    config.email = smtp_config.clone();
    let state = AppState {
        config: Arc::new(config),
        ..common::state(pool.clone())
    };

    let user = users::insert(
        &pool,
        "kovacs@autotherm.hu",
        "Kovács János",
        Role::Office,
        "$argon2id$x",
        false,
    )
    .await
    .unwrap();
    let staff = AuthUser {
        user_id: user.id,
        session_id: 0,
        session_kind: SessionKind::Web,
        email: user.email.clone(),
        display_name: user.display_name.clone(),
        role: Role::Office,
        must_change_password: false,
    };

    let id = send_manual(
        &state,
        &staff,
        &ComposeRequest {
            order_id: None,
            lead_id: None,
            partner_id: None,
            to: "customer@example.hu".into(),
            cc: vec![],
            template_key: None,
            subject: Some("Árajánlat".into()),
            body: Some("Tisztelt Ügyfél!\n\nMellékelten küldjük.\n\n{{user.name}}".into()),
            body_markdown: false,
            hero: None,
            attachment_document_ids: vec![],
            embed_document_ids: vec![],
        },
    )
    .await
    .unwrap();

    let mailer = Mailer::from_config(&smtp_config).unwrap();
    assert!(matches!(
        deliver(&state, &mailer, id).await.unwrap(),
        Delivery::Done
    ));

    let row = emails::find(&pool, id).await.unwrap().unwrap();
    assert_eq!(row.status, EmailStatus::Sent);
    assert!(
        row.from_address.ends_with("<kovacs@autotherm.hu>"),
        "{}",
        row.from_address
    );
    assert_eq!(row.reply_to, None);

    let got = next(&mut rx).await;
    assert!(
        got.mail_from.contains("<kovacs@autotherm.hu>"),
        "{}",
        got.mail_from
    );
    assert_eq!(got.rcpt_to, vec!["<customer@example.hu>".to_string()]);
    assert!(!got.data.contains("Reply-To:"));
    assert!(
        !got.data.contains("Auto-Submitted"),
        "a person's email is not automatic"
    );
}

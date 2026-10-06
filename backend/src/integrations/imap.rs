//! Reads new messages from the sales mailbox over IMAP (TLS). Only fetches: nothing is
//! moved, flagged or deleted, so the mailbox stays exactly as the salespeople left it.

use std::sync::Arc;

use anyhow::{Context, anyhow};
use futures::TryStreamExt;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::{self, ClientConfig, RootCertStore, pki_types::ServerName};

use crate::config::ImapConfig;

/// What one read brought back: the folder's UIDVALIDITY (a new value means the UIDs were
/// renumbered and the cursor must start over) and the new messages with their UIDs.
pub struct Fetched {
    pub uid_validity: u32,
    pub messages: Vec<(u32, Vec<u8>)>,
}

/// At most this many messages per read; the rest come next time.
const MAX_PER_READ: usize = 200;

/// Messages with a UID above `after_uid` (all recent ones when `after_uid` is None or the
/// UIDVALIDITY changed — the caller decides that by passing None).
pub async fn fetch_new(cfg: &ImapConfig, after_uid: Option<u32>) -> anyhow::Result<Fetched> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls = ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .context("TLS setup")?
        .with_root_certificates(roots)
        .with_no_client_auth();
    let tcp = TcpStream::connect((cfg.host.as_str(), cfg.port))
        .await
        .with_context(|| format!("connecting to {}:{}", cfg.host, cfg.port))?;
    let name = ServerName::try_from(cfg.host.clone()).context("IMAP host name")?;
    let stream = TlsConnector::from(Arc::new(tls))
        .connect(name, tcp)
        .await
        .context("TLS handshake with the IMAP server")?;

    let mut client = async_imap::Client::new(stream);
    // The server speaks first.
    let _greeting = client.read_response().await;
    let mut session = client
        .login(&cfg.user, &cfg.password)
        .await
        .map_err(|(e, _)| anyhow!("IMAP login failed: {e}"))?;
    let mailbox = session.select(&cfg.folder).await.context("selecting the folder")?;
    let uid_validity = mailbox.uid_validity.unwrap_or(0);

    // First read: start from the newest few rather than the whole history.
    let query = match after_uid {
        Some(uid) => format!("UID {}:*", uid + 1),
        None => {
            let next = mailbox.uid_next.unwrap_or(1);
            format!("UID {}:*", next.saturating_sub(50).max(1))
        }
    };
    let mut uids: Vec<u32> = session
        .uid_search(&query)
        .await
        .context("searching the folder")?
        .into_iter()
        // "n:*" always includes the highest UID, even when it is below n.
        .filter(|u| after_uid.is_none_or(|after| *u > after))
        .collect();
    uids.sort_unstable();
    uids.truncate(MAX_PER_READ);

    let mut messages = Vec::with_capacity(uids.len());
    if !uids.is_empty() {
        let set = uids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        let fetches: Vec<_> = session
            .uid_fetch(&set, "BODY.PEEK[]")
            .await
            .context("fetching messages")?
            .try_collect()
            .await
            .context("reading messages")?;
        for f in &fetches {
            if let (Some(uid), Some(body)) = (f.uid, f.body()) {
                messages.push((uid, body.to_vec()));
            }
        }
        messages.sort_by_key(|(uid, _)| *uid);
    }
    let _ = session.logout().await;
    Ok(Fetched {
        uid_validity,
        messages,
    })
}

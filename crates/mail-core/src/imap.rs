use std::pin::Pin;
use std::task::{Context, Poll};

use async_imap::types::Flag;
use async_imap::{Client, Session};
use futures::TryStreamExt;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;

use crate::mime::parse_rfc822;
use crate::models::{Account, Folder, MessageSummary, TlsMode};
use crate::{Error, Result};

#[derive(Debug)]
pub(crate) enum ImapIo {
    Plain(TcpStream),
    Tls(tokio_native_tls::TlsStream<TcpStream>),
}

impl AsyncRead for ImapIo {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_read(cx, buf),
            Self::Tls(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for ImapIo {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_write(cx, buf),
            Self::Tls(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_flush(cx),
            Self::Tls(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Plain(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Tls(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

fn tls_connector(account: &Account) -> Result<tokio_native_tls::TlsConnector> {
    let mut builder = native_tls::TlsConnector::builder();
    if account.accept_invalid_certs {
        builder.danger_accept_invalid_certs(true);
        builder.danger_accept_invalid_hostnames(true);
    }
    Ok(tokio_native_tls::TlsConnector::from(builder.build()?))
}

pub(crate) async fn connect(account: &Account, password: &str) -> Result<Session<ImapIo>> {
    if account.imap_host.trim().is_empty() {
        return Err(Error::InvalidConfig("IMAP host is empty".into()));
    }
    if account.username.trim().is_empty() {
        return Err(Error::InvalidConfig("username is empty".into()));
    }

    let tcp = TcpStream::connect((account.imap_host.as_str(), account.imap_port)).await?;
    let _ = tcp.set_nodelay(true);

    let (io, greeting_already_read) = match account.tls_mode {
        TlsMode::None => (ImapIo::Plain(tcp), false),
        TlsMode::Implicit => {
            let tls = tls_connector(account)?
                .connect(&account.imap_host, tcp)
                .await
                .map_err(|e| Error::Tls(e.to_string()))?;
            (ImapIo::Tls(tls), false)
        }
        TlsMode::StartTls => {
            let mut client = Client::new(ImapIo::Plain(tcp));
            client
                .read_response()
                .await
                .map_err(|e| Error::Imap(e.to_string()))?;
            client
                .run_command_and_check_ok("STARTTLS", None)
                .await
                .map_err(|e| Error::Imap(e.to_string()))?;
            let ImapIo::Plain(plain) = client.into_inner() else {
                return Err(Error::Imap(
                    "expected plaintext stream before STARTTLS".into(),
                ));
            };
            let tls = tls_connector(account)?
                .connect(&account.imap_host, plain)
                .await
                .map_err(|e| Error::Tls(e.to_string()))?;
            (ImapIo::Tls(tls), true)
        }
    };

    let mut client = Client::new(io);
    if !greeting_already_read {
        client
            .read_response()
            .await
            .map_err(|e| Error::Imap(e.to_string()))?;
    }

    client
        .login(&account.username, password)
        .await
        .map_err(|(e, _)| {
            let msg = e.to_string();
            if msg.to_lowercase().contains("auth") || msg.to_lowercase().contains("login") {
                Error::AuthFailed
            } else {
                Error::Imap(msg)
            }
        })
}

pub(crate) async fn list_folders(session: &mut Session<ImapIo>) -> Result<Vec<Folder>> {
    let stream = session
        .list(Some(""), Some("*"))
        .await
        .map_err(|e| Error::Imap(e.to_string()))?;
    let names: Vec<_> = stream
        .try_collect()
        .await
        .map_err(|e| Error::Imap(e.to_string()))?;
    Ok(names
        .into_iter()
        .map(|name| Folder {
            name: name.name().to_string(),
            delimiter: name.delimiter().map(str::to_string),
        })
        .collect())
}

pub(crate) async fn fetch_summaries(
    session: &mut Session<ImapIo>,
    folder: &str,
) -> Result<(Option<u32>, Vec<(MessageSummary, Option<Vec<u8>>)>)> {
    let mailbox = session
        .select(folder)
        .await
        .map_err(|e| Error::Imap(e.to_string()))?;
    if mailbox.exists == 0 {
        return Ok((mailbox.uid_validity, Vec::new()));
    }

    let stream = session
        .uid_fetch("1:*", "(UID FLAGS RFC822.HEADER)")
        .await
        .map_err(|e| Error::Imap(e.to_string()))?;
    let fetches: Vec<_> = stream
        .try_collect()
        .await
        .map_err(|e| Error::Imap(e.to_string()))?;

    let mut out = Vec::with_capacity(fetches.len());
    for fetch in fetches {
        let Some(uid) = fetch.uid else {
            continue;
        };
        let unseen = !fetch.flags().any(|flag| matches!(flag, Flag::Seen));
        let header = fetch.header().unwrap_or_default();
        let parsed = parse_rfc822(header);
        let summary = MessageSummary {
            uid,
            folder: folder.to_string(),
            from: parsed.from,
            subject: parsed.subject,
            date: parsed.date,
            unseen,
        };
        out.push((summary, Some(header.to_vec())));
    }
    Ok((mailbox.uid_validity, out))
}

pub(crate) async fn fetch_raw(
    session: &mut Session<ImapIo>,
    folder: &str,
    uid: u32,
) -> Result<Vec<u8>> {
    let mailbox = session
        .select(folder)
        .await
        .map_err(|e| Error::Imap(e.to_string()))?;
    if mailbox.exists == 0 {
        return Err(Error::MessageNotFound {
            folder: folder.to_string(),
            uid,
        });
    }
    let stream = session
        .uid_fetch(uid.to_string(), "(UID FLAGS BODY.PEEK[])")
        .await
        .map_err(|e| Error::Imap(e.to_string()))?;
    let fetches: Vec<_> = stream
        .try_collect()
        .await
        .map_err(|e| Error::Imap(e.to_string()))?;
    let fetch = fetches
        .into_iter()
        .next()
        .ok_or_else(|| Error::MessageNotFound {
            folder: folder.to_string(),
            uid,
        })?;
    fetch
        .body()
        .or_else(|| fetch.header())
        .map(|b| b.to_vec())
        .ok_or_else(|| Error::MessageNotFound {
            folder: folder.to_string(),
            uid,
        })
}

pub(crate) async fn logout(session: &mut Session<ImapIo>) {
    let _ = session.logout().await;
}

fn uid_set(uids: &[u32]) -> Result<String> {
    if uids.is_empty() {
        return Err(Error::InvalidConfig("UID list is empty".into()));
    }
    Ok(uids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(","))
}

fn flags_query(op: &str, flags: &[crate::ImapFlag]) -> String {
    let tokens: Vec<&str> = flags.iter().map(|f| f.as_imap_token()).collect();
    format!("{op} ({})", tokens.join(" "))
}

/// CREATE a mailbox. Used today for tests; later also for a hidden rules folder.
pub(crate) async fn create_folder(session: &mut Session<ImapIo>, name: &str) -> Result<()> {
    session
        .create(name)
        .await
        .map_err(|e| Error::Imap(e.to_string()))
}

/// UID STORE +FLAGS / -FLAGS. Callers must pass a selected mailbox via `folder`.
pub(crate) async fn store_flags(
    session: &mut Session<ImapIo>,
    folder: &str,
    uids: &[u32],
    add: &[crate::ImapFlag],
    remove: &[crate::ImapFlag],
) -> Result<()> {
    session
        .select(folder)
        .await
        .map_err(|e| Error::Imap(e.to_string()))?;
    let set = uid_set(uids)?;
    if !add.is_empty() {
        let stream = session
            .uid_store(&set, flags_query("+FLAGS.SILENT", add))
            .await
            .map_err(|e| Error::Imap(e.to_string()))?;
        let _: Vec<_> = stream
            .try_collect()
            .await
            .map_err(|e| Error::Imap(e.to_string()))?;
    }
    if !remove.is_empty() {
        let stream = session
            .uid_store(&set, flags_query("-FLAGS.SILENT", remove))
            .await
            .map_err(|e| Error::Imap(e.to_string()))?;
        let _: Vec<_> = stream
            .try_collect()
            .await
            .map_err(|e| Error::Imap(e.to_string()))?;
    }
    Ok(())
}

/// UID MOVE, with COPY+\\Deleted+EXPUNGE fallback when MOVE is unavailable.
pub(crate) async fn move_uids(
    session: &mut Session<ImapIo>,
    from: &str,
    uids: &[u32],
    to: &str,
) -> Result<()> {
    session
        .select(from)
        .await
        .map_err(|e| Error::Imap(e.to_string()))?;
    let set = uid_set(uids)?;
    if session.uid_mv(&set, to).await.is_ok() {
        return Ok(());
    }
    session
        .uid_copy(&set, to)
        .await
        .map_err(|e| Error::Imap(e.to_string()))?;
    store_flags(session, from, uids, &[crate::ImapFlag::Deleted], &[]).await?;
    let stream = session
        .uid_expunge(&set)
        .await
        .map_err(|e| Error::Imap(e.to_string()))?;
    let expunge_result: Result<Vec<_>> = stream
        .try_collect()
        .await
        .map_err(|e| Error::Imap(e.to_string()));
    if expunge_result.is_err() {
        let _ = session.expunge().await;
    }
    Ok(())
}

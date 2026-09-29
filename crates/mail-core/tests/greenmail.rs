//! Integration tests against a local GreenMail server.
//!
//! Start the server first:
//!   docker compose up -d
//!   # or ./scripts/start-greenmail.sh
//!
//! Optional env:
//!   GREENMAIL_HOST (default 127.0.0.1)
//!   GREENMAIL_IMAP_PORT (default 3143)
//!   GREENMAIL_SMTP_PORT (default 3025)
//!   GREENMAIL_USER (default testuser)
//!   GREENMAIL_PASS (default testpass)

use std::time::Duration;

use lettre::message::{header, MultiPart, SinglePart};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use mail_core::credentials::MemoryCredentialStore;
use mail_core::models::{ImapFlag, NewAccount, TlsMode};
use mail_core::{Cache, MailService};
use serial_test::serial;
use tokio::net::TcpStream;
use tokio::time::sleep;

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn host() -> String {
    env_or("GREENMAIL_HOST", "127.0.0.1")
}

fn imap_port() -> u16 {
    env_or("GREENMAIL_IMAP_PORT", "3143")
        .parse()
        .expect("IMAP port")
}

fn smtp_port() -> u16 {
    env_or("GREENMAIL_SMTP_PORT", "3025")
        .parse()
        .expect("SMTP port")
}

fn username() -> String {
    env_or("GREENMAIL_USER", "testuser")
}

fn password() -> String {
    env_or("GREENMAIL_PASS", "testpass")
}

async fn wait_for_greenmail() {
    let addr = format!("{}:{}", host(), imap_port());
    for _ in 0..50 {
        if TcpStream::connect(&addr).await.is_ok() {
            return;
        }
        sleep(Duration::from_millis(200)).await;
    }
    panic!(
        "GreenMail is not running at {addr}. Start it with `docker compose up -d` or `./scripts/start-greenmail.sh`"
    );
}

async fn smtp_mailer() -> AsyncSmtpTransport<Tokio1Executor> {
    AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host())
        .port(smtp_port())
        .build()
}

async fn send_sample_mail() {
    let mailer = smtp_mailer().await;

    let plain = Message::builder()
        .from("Alice <alice@example.com>".parse().unwrap())
        .to("Test User <testuser@localhost>".parse().unwrap())
        .subject("Welcome to mymail")
        .body(String::from(
            "This is a plain-text sample used by integration tests.",
        ))
        .unwrap();
    mailer.send(plain).await.expect("send plain");

    let japanese = Message::builder()
        .from("山田太郎 <taro@example.com>".parse().unwrap())
        .to("Test User <testuser@localhost>".parse().unwrap())
        .subject("会議の件")
        .body(String::from("来週の会議は火曜の10時です。全文検索テスト。"))
        .unwrap();
    mailer.send(japanese).await.expect("send japanese");

    let html = Message::builder()
        .from("News <news@example.com>".parse().unwrap())
        .to("Test User <testuser@localhost>".parse().unwrap())
        .subject("HTML newsletter")
        .multipart(
            MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .header(header::ContentType::TEXT_PLAIN)
                        .body(String::from("HTML fallback text")),
                )
                .singlepart(
                    SinglePart::builder()
                        .header(header::ContentType::TEXT_HTML)
                        .body(String::from(
                            "<html><body><h1>Hello HTML</h1>\
                             <script>alert('xss')</script>\
                             <img src=\"https://evil.example/tracker.png\">\
                             <p>リモート画像はデフォルトでブロックされます。</p>\
                             </body></html>",
                        )),
                ),
        )
        .unwrap();
    mailer.send(html).await.expect("send html");
}

async fn make_service() -> MailService {
    wait_for_greenmail().await;
    send_sample_mail().await;
    let cache = Cache::open_in_memory().unwrap();
    MailService::open_with_store(cache, Box::new(MemoryCredentialStore::new()))
}

#[tokio::test]
#[serial]
async fn connect_list_folders_and_read_bodies() {
    let service = make_service().await;
    let account = service
        .add_account(
            NewAccount {
                display_name: "GreenMail".into(),
                imap_host: host(),
                imap_port: imap_port(),
                tls_mode: TlsMode::None,
                username: username(),
                accept_invalid_certs: true,
            },
            password(),
        )
        .await
        .expect("add account");

    let folders = service
        .list_folders(&account.id)
        .await
        .expect("list folders");
    assert!(
        folders.iter().any(|f| f.name.eq_ignore_ascii_case("INBOX")),
        "expected INBOX, got {folders:?}"
    );

    let inbox = folders
        .iter()
        .find(|f| f.name.eq_ignore_ascii_case("INBOX"))
        .unwrap()
        .name
        .clone();

    let messages = service
        .sync_folder(&account.id, &inbox)
        .await
        .expect("sync folder");
    assert!(
        messages.len() >= 3,
        "expected at least 3 seeded messages, got {}",
        messages.len()
    );
    assert!(messages.iter().any(|m| m.subject.contains("Welcome")));
    assert!(messages.iter().any(|m| m.subject.contains("会議")));
    assert!(messages.iter().any(|m| m.subject.contains("HTML")));
    assert!(messages.iter().any(|m| m.unseen));

    let japanese = messages
        .iter()
        .find(|m| m.subject.contains("会議"))
        .unwrap();
    let body = service
        .get_message(&account.id, &inbox, japanese.uid)
        .await
        .expect("get japanese body");
    assert!(
        body.text
            .as_deref()
            .unwrap_or_default()
            .contains("全文検索テスト"),
        "japanese body missing: {body:?}"
    );

    let html_msg = messages
        .iter()
        .find(|m| m.subject.contains("HTML"))
        .unwrap();
    let html_body = service
        .get_message(&account.id, &inbox, html_msg.uid)
        .await
        .expect("get html body");
    assert!(
        html_body
            .html
            .as_deref()
            .unwrap_or_default()
            .contains("<h1>Hello HTML</h1>"),
        "html body missing: {html_body:?}"
    );

    let hits = service.search(&account.id, "全文検索").expect("search");
    assert!(
        hits.iter().any(|m| m.subject.contains("会議")),
        "FTS5 search missed Japanese body, hits={hits:?}"
    );
}

#[tokio::test]
#[serial]
async fn rejects_bad_password() {
    wait_for_greenmail().await;
    let cache = Cache::open_in_memory().unwrap();
    let service = MailService::open_with_store(cache, Box::new(MemoryCredentialStore::new()));
    let err = service
        .add_account(
            NewAccount {
                display_name: "bad".into(),
                imap_host: host(),
                imap_port: imap_port(),
                tls_mode: TlsMode::None,
                username: username(),
                accept_invalid_certs: true,
            },
            "definitely-wrong-password".into(),
        )
        .await
        .expect_err("login should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("auth") || msg.contains("IMAP") || msg.contains("login"),
        "unexpected error: {msg}"
    );
}

#[tokio::test]
#[serial]
async fn move_and_flag_messages() {
    let service = make_service().await;
    let account = service
        .add_account(
            NewAccount {
                display_name: "GreenMail".into(),
                imap_host: host(),
                imap_port: imap_port(),
                tls_mode: TlsMode::None,
                username: username(),
                accept_invalid_certs: true,
            },
            password(),
        )
        .await
        .expect("add account");

    let dest = format!("Archive-{}", account.id.split('-').next().unwrap_or("x"));
    service
        .create_folder(&account.id, &dest)
        .await
        .unwrap_or_else(|e| panic!("create {dest}: {e}"));

    let messages = service
        .sync_folder(&account.id, "INBOX")
        .await
        .expect("sync INBOX");
    assert!(!messages.is_empty(), "need mail to move");
    let uid = messages[0].uid;

    service
        .set_flags(
            &account.id,
            "INBOX",
            &[uid],
            &[ImapFlag::Seen, ImapFlag::Flagged],
            &[],
        )
        .await
        .expect("set flags");
    let after_flag = service.list_messages(&account.id, "INBOX").unwrap();
    let marked = after_flag
        .iter()
        .find(|m| m.uid == uid)
        .expect("uid in cache");
    assert!(!marked.unseen, "\\Seen should clear unseen in the cache");

    service
        .move_messages(&account.id, "INBOX", &[uid], &dest)
        .await
        .expect("move");
    let inbox_after = service
        .sync_folder(&account.id, "INBOX")
        .await
        .expect("resync INBOX");
    assert!(
        inbox_after.iter().all(|m| m.uid != uid),
        "moved UID should leave INBOX"
    );
    let archive = service
        .sync_folder(&account.id, &dest)
        .await
        .expect("sync archive");
    assert!(
        !archive.is_empty(),
        "destination folder should contain the moved message"
    );
}

//! Headless mail engine: IMAP, MIME parsing, SQLite/FTS5 cache, credential storage.
//!
//! IMAP mutations used by a future client-side rules engine (not implemented here)
//! live on [`MailService`]: [`MailService::move_messages`], [`MailService::set_flags`],
//! and [`MailService::create_folder`]. Rules JSON can later be stored as a message in
//! a hidden IMAP folder (last-write-wins) via CREATE + APPEND + FETCH.

pub mod cache;
pub mod credentials;
pub mod error;
pub(crate) mod imap;
pub mod mime;
pub mod models;
pub mod service;

pub use cache::Cache;
pub use credentials::{CredentialStore, FileCredentialStore, MemoryCredentialStore};
pub use error::{Error, Result};
pub use models::{Account, Folder, ImapFlag, MessageBody, MessageSummary, NewAccount, TlsMode};
pub use service::MailService;

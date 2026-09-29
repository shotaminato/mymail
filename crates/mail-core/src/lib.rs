//! Headless mail engine: IMAP, MIME parsing, SQLite/FTS5 cache, credential storage.

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
pub use models::{Account, Folder, MessageBody, MessageSummary, NewAccount, TlsMode};
pub use service::MailService;

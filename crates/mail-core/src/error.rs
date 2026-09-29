use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IMAP error: {0}")]
    Imap(String),
    #[error("TLS error: {0}")]
    Tls(String),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("account not found: {0}")]
    AccountNotFound(String),
    #[error("message not found (folder={folder}, uid={uid})")]
    MessageNotFound { folder: String, uid: u32 },
    #[error("authentication failed")]
    AuthFailed,
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("{0}")]
    Other(String),
}

impl From<native_tls::Error> for Error {
    fn from(value: native_tls::Error) -> Self {
        Self::Tls(value.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::Other(value.to_string())
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TlsMode {
    #[default]
    None,
    StartTls,
    Implicit,
}

impl TlsMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::StartTls => "starttls",
            Self::Implicit => "implicit",
        }
    }

    pub fn parse(value: &str) -> crate::Result<Self> {
        match value {
            "none" => Ok(Self::None),
            "starttls" => Ok(Self::StartTls),
            "implicit" => Ok(Self::Implicit),
            other => Err(crate::Error::InvalidConfig(format!(
                "unknown TLS mode: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub display_name: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub tls_mode: TlsMode,
    pub username: String,
    pub accept_invalid_certs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAccount {
    pub display_name: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub tls_mode: TlsMode,
    pub username: String,
    pub accept_invalid_certs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub name: String,
    pub delimiter: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSummary {
    pub uid: u32,
    pub folder: String,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub unseen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageBody {
    pub uid: u32,
    pub folder: String,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub unseen: bool,
    pub text: Option<String>,
    pub html: Option<String>,
}

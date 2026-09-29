use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::{Error, Result};

/// Password storage backend. The OS keychain (`keyring` crate) is the intended
/// production implementation; this milestone uses an in-process / JSON file store
/// so the rest of the app can be built and tested without platform keychain setup.
pub trait CredentialStore: Send + Sync {
    fn get_password(&self, account_id: &str) -> Result<Option<String>>;
    fn set_password(&self, account_id: &str, password: &str) -> Result<()>;
    fn delete_password(&self, account_id: &str) -> Result<()>;
}

#[derive(Default)]
pub struct MemoryCredentialStore {
    inner: Mutex<HashMap<String, String>>,
}

impl MemoryCredentialStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CredentialStore for MemoryCredentialStore {
    fn get_password(&self, account_id: &str) -> Result<Option<String>> {
        Ok(self.inner.lock().unwrap().get(account_id).cloned())
    }

    fn set_password(&self, account_id: &str, password: &str) -> Result<()> {
        self.inner
            .lock()
            .unwrap()
            .insert(account_id.to_string(), password.to_string());
        Ok(())
    }

    fn delete_password(&self, account_id: &str) -> Result<()> {
        self.inner.lock().unwrap().remove(account_id);
        Ok(())
    }
}

/// JSON file store. Not encrypted — placeholder until OS keychain is wired up.
pub struct FileCredentialStore {
    path: PathBuf,
    inner: Mutex<HashMap<String, String>>,
}

impl FileCredentialStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let map = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            serde_json::from_str(&raw).unwrap_or_default()
        } else {
            HashMap::new()
        };
        Ok(Self {
            path,
            inner: Mutex::new(map),
        })
    }

    fn persist(&self, map: &HashMap<String, String>) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(map)?)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

impl CredentialStore for FileCredentialStore {
    fn get_password(&self, account_id: &str) -> Result<Option<String>> {
        Ok(self.inner.lock().unwrap().get(account_id).cloned())
    }

    fn set_password(&self, account_id: &str, password: &str) -> Result<()> {
        let mut map = self.inner.lock().unwrap();
        map.insert(account_id.to_string(), password.to_string());
        self.persist(&map)
    }

    fn delete_password(&self, account_id: &str) -> Result<()> {
        let mut map = self.inner.lock().unwrap();
        map.remove(account_id);
        self.persist(&map)
    }
}

pub fn require_password(store: &dyn CredentialStore, account_id: &str) -> Result<String> {
    store
        .get_password(account_id)?
        .ok_or_else(|| Error::Other(format!("no password stored for account {account_id}")))
}

use std::path::Path;
use std::sync::Mutex;

use uuid::Uuid;

use crate::cache::Cache;
use crate::credentials::{require_password, CredentialStore, FileCredentialStore};
use crate::imap;
use crate::mime::parse_rfc822;
use crate::models::{Account, Folder, MessageBody, MessageSummary, NewAccount};
use crate::{Error, Result};

pub struct MailService {
    cache: Mutex<Cache>,
    creds: Box<dyn CredentialStore>,
}

impl MailService {
    pub fn open(db_path: impl AsRef<Path>, creds_path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            cache: Mutex::new(Cache::open(db_path)?),
            creds: Box::new(FileCredentialStore::open(creds_path)?),
        })
    }

    pub fn open_with_store(cache: Cache, creds: Box<dyn CredentialStore>) -> Self {
        Self {
            cache: Mutex::new(cache),
            creds,
        }
    }

    pub fn list_accounts(&self) -> Result<Vec<Account>> {
        self.cache.lock().unwrap().list_accounts()
    }

    pub async fn add_account(&self, new: NewAccount, password: String) -> Result<Account> {
        if password.is_empty() {
            return Err(Error::InvalidConfig("password is empty".into()));
        }
        let probe = Account {
            id: "probe".into(),
            display_name: new.display_name.clone(),
            imap_host: new.imap_host.clone(),
            imap_port: new.imap_port,
            tls_mode: new.tls_mode,
            username: new.username.clone(),
            accept_invalid_certs: new.accept_invalid_certs,
        };
        let mut session = imap::connect(&probe, &password).await?;
        imap::logout(&mut session).await;

        let id = Uuid::new_v4().to_string();
        self.creds.set_password(&id, &password)?;
        let account = self.cache.lock().unwrap().insert_account(&id, &new)?;
        Ok(account)
    }

    pub fn remove_account(&self, account_id: &str) -> Result<()> {
        self.cache.lock().unwrap().delete_account(account_id)?;
        self.creds.delete_password(account_id)?;
        Ok(())
    }

    pub async fn list_folders(&self, account_id: &str) -> Result<Vec<Folder>> {
        let account = self.cache.lock().unwrap().get_account(account_id)?;
        let password = require_password(self.creds.as_ref(), account_id)?;
        let mut session = imap::connect(&account, &password).await?;
        let folders = imap::list_folders(&mut session).await;
        imap::logout(&mut session).await;
        folders
    }

    pub async fn sync_folder(&self, account_id: &str, folder: &str) -> Result<Vec<MessageSummary>> {
        let account = self.cache.lock().unwrap().get_account(account_id)?;
        let password = require_password(self.creds.as_ref(), account_id)?;
        let mut session = imap::connect(&account, &password).await?;
        let result = imap::fetch_summaries(&mut session, folder).await;
        imap::logout(&mut session).await;
        let (uid_validity, rows) = result?;

        {
            let cache = self.cache.lock().unwrap();
            if let Some(uid_validity) = uid_validity {
                let previous = cache.uid_validity(account_id, folder)?;
                if previous.is_some() && previous != Some(uid_validity) {
                    cache.clear_folder(account_id, folder)?;
                }
                cache.set_uid_validity(account_id, folder, uid_validity)?;
            }
            for (summary, _) in &rows {
                cache.upsert_summary(account_id, summary, None, None)?;
            }
        }

        self.list_messages(account_id, folder)
    }

    pub fn list_messages(&self, account_id: &str, folder: &str) -> Result<Vec<MessageSummary>> {
        self.cache.lock().unwrap().list_messages(account_id, folder)
    }

    pub async fn get_message(
        &self,
        account_id: &str,
        folder: &str,
        uid: u32,
    ) -> Result<MessageBody> {
        if let Some(existing) = self
            .cache
            .lock()
            .unwrap()
            .get_message(account_id, folder, uid)?
        {
            if existing.text.is_some() || existing.html.is_some() {
                return Ok(existing);
            }
        }

        let account = self.cache.lock().unwrap().get_account(account_id)?;
        let password = require_password(self.creds.as_ref(), account_id)?;
        let mut session = imap::connect(&account, &password).await?;
        let raw = imap::fetch_raw(&mut session, folder, uid).await;
        imap::logout(&mut session).await;
        let raw = raw?;
        let parsed = parse_rfc822(&raw);

        let cache = self.cache.lock().unwrap();
        let unseen = cache
            .get_message(account_id, folder, uid)?
            .map(|m| m.unseen)
            .unwrap_or(true);
        cache.update_body(account_id, folder, uid, &parsed, unseen)?;
        cache
            .get_message(account_id, folder, uid)?
            .ok_or_else(|| Error::MessageNotFound {
                folder: folder.to_string(),
                uid,
            })
    }

    pub fn search(&self, account_id: &str, query: &str) -> Result<Vec<MessageSummary>> {
        self.cache.lock().unwrap().search(account_id, query)
    }
}

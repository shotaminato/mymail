use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::models::{Account, MessageBody, MessageSummary, NewAccount, TlsMode};
use crate::Result;

const SCHEMA: &str = r#"
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;

CREATE TABLE IF NOT EXISTS accounts (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    imap_host TEXT NOT NULL,
    imap_port INTEGER NOT NULL,
    tls_mode TEXT NOT NULL,
    username TEXT NOT NULL,
    accept_invalid_certs INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS folder_state (
    account_id TEXT NOT NULL,
    folder TEXT NOT NULL,
    uid_validity INTEGER,
    PRIMARY KEY (account_id, folder),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY,
    account_id TEXT NOT NULL,
    folder TEXT NOT NULL,
    uid INTEGER NOT NULL,
    from_addr TEXT NOT NULL DEFAULT '',
    subject TEXT NOT NULL DEFAULT '',
    date TEXT NOT NULL DEFAULT '',
    date_unix INTEGER,
    unseen INTEGER NOT NULL DEFAULT 1,
    body_text TEXT,
    body_html TEXT,
    UNIQUE (account_id, folder, uid),
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
);

CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    subject,
    from_addr,
    body_text,
    content='messages',
    content_rowid='id',
    tokenize='trigram'
);

CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, subject, from_addr, body_text)
    VALUES (new.id, new.subject, new.from_addr, coalesce(new.body_text, ''));
END;

CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, subject, from_addr, body_text)
    VALUES ('delete', old.id, old.subject, old.from_addr, coalesce(old.body_text, ''));
END;

CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE ON messages BEGIN
    INSERT INTO messages_fts(messages_fts, rowid, subject, from_addr, body_text)
    VALUES ('delete', old.id, old.subject, old.from_addr, coalesce(old.body_text, ''));
    INSERT INTO messages_fts(rowid, subject, from_addr, body_text)
    VALUES (new.id, new.subject, new.from_addr, coalesce(new.body_text, ''));
END;
"#;

pub struct Cache {
    conn: Connection,
}

impl Cache {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    pub fn insert_account(&self, id: &str, new: &NewAccount) -> Result<Account> {
        self.conn.execute(
            "INSERT INTO accounts (id, display_name, imap_host, imap_port, tls_mode, username, accept_invalid_certs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                new.display_name,
                new.imap_host,
                new.imap_port,
                new.tls_mode.as_str(),
                new.username,
                new.accept_invalid_certs as i64
            ],
        )?;
        Ok(Account {
            id: id.to_string(),
            display_name: new.display_name.clone(),
            imap_host: new.imap_host.clone(),
            imap_port: new.imap_port,
            tls_mode: new.tls_mode,
            username: new.username.clone(),
            accept_invalid_certs: new.accept_invalid_certs,
        })
    }

    pub fn list_accounts(&self) -> Result<Vec<Account>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, display_name, imap_host, imap_port, tls_mode, username, accept_invalid_certs
             FROM accounts ORDER BY display_name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Account {
                id: row.get(0)?,
                display_name: row.get(1)?,
                imap_host: row.get(2)?,
                imap_port: row.get::<_, i64>(3)? as u16,
                tls_mode: TlsMode::parse(&row.get::<_, String>(4)?).unwrap_or_default(),
                username: row.get(5)?,
                accept_invalid_certs: row.get::<_, i64>(6)? != 0,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_account(&self, id: &str) -> Result<Account> {
        self.conn
            .query_row(
                "SELECT id, display_name, imap_host, imap_port, tls_mode, username, accept_invalid_certs
                 FROM accounts WHERE id = ?1",
                [id],
                |row| {
                    Ok(Account {
                        id: row.get(0)?,
                        display_name: row.get(1)?,
                        imap_host: row.get(2)?,
                        imap_port: row.get::<_, i64>(3)? as u16,
                        tls_mode: TlsMode::parse(&row.get::<_, String>(4)?).unwrap_or_default(),
                        username: row.get(5)?,
                        accept_invalid_certs: row.get::<_, i64>(6)? != 0,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| crate::Error::AccountNotFound(id.to_string()))
    }

    pub fn delete_account(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM accounts WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn uid_validity(&self, account_id: &str, folder: &str) -> Result<Option<u32>> {
        Ok(self
            .conn
            .query_row(
                "SELECT uid_validity FROM folder_state WHERE account_id = ?1 AND folder = ?2",
                params![account_id, folder],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten()
            .map(|v| v as u32))
    }

    pub fn set_uid_validity(
        &self,
        account_id: &str,
        folder: &str,
        uid_validity: u32,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO folder_state (account_id, folder, uid_validity)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(account_id, folder) DO UPDATE SET uid_validity = excluded.uid_validity",
            params![account_id, folder, uid_validity as i64],
        )?;
        Ok(())
    }

    pub fn clear_folder(&self, account_id: &str, folder: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM messages WHERE account_id = ?1 AND folder = ?2",
            params![account_id, folder],
        )?;
        Ok(())
    }

    pub fn upsert_summary(
        &self,
        account_id: &str,
        summary: &MessageSummary,
        text: Option<&str>,
        html: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO messages (account_id, folder, uid, from_addr, subject, date, unseen, body_text, body_html)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(account_id, folder, uid) DO UPDATE SET
                from_addr = excluded.from_addr,
                subject = excluded.subject,
                date = excluded.date,
                unseen = excluded.unseen,
                body_text = COALESCE(excluded.body_text, messages.body_text),
                body_html = COALESCE(excluded.body_html, messages.body_html)",
            params![
                account_id,
                summary.folder,
                summary.uid,
                summary.from,
                summary.subject,
                summary.date,
                summary.unseen as i64,
                text,
                html
            ],
        )?;
        Ok(())
    }

    pub fn update_body(
        &self,
        account_id: &str,
        folder: &str,
        uid: u32,
        parsed: &crate::mime::ParsedMail,
        unseen: bool,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE messages SET from_addr=?1, subject=?2, date=?3, date_unix=?4, unseen=?5, body_text=?6, body_html=?7
             WHERE account_id=?8 AND folder=?9 AND uid=?10",
            params![
                parsed.from,
                parsed.subject,
                parsed.date,
                parsed.date_unix,
                unseen as i64,
                parsed.text,
                parsed.html,
                account_id,
                folder,
                uid
            ],
        )?;
        Ok(())
    }

    pub fn list_messages(&self, account_id: &str, folder: &str) -> Result<Vec<MessageSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT uid, folder, from_addr, subject, date, unseen
             FROM messages
             WHERE account_id = ?1 AND folder = ?2
             ORDER BY COALESCE(date_unix, uid) DESC, uid DESC",
        )?;
        let rows = stmt.query_map(params![account_id, folder], |row| {
            Ok(MessageSummary {
                uid: row.get::<_, i64>(0)? as u32,
                folder: row.get(1)?,
                from: row.get(2)?,
                subject: row.get(3)?,
                date: row.get(4)?,
                unseen: row.get::<_, i64>(5)? != 0,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_message(
        &self,
        account_id: &str,
        folder: &str,
        uid: u32,
    ) -> Result<Option<MessageBody>> {
        self.conn
            .query_row(
                "SELECT uid, folder, from_addr, subject, date, unseen, body_text, body_html
                 FROM messages WHERE account_id=?1 AND folder=?2 AND uid=?3",
                params![account_id, folder, uid],
                |row| {
                    Ok(MessageBody {
                        uid: row.get::<_, i64>(0)? as u32,
                        folder: row.get(1)?,
                        from: row.get(2)?,
                        subject: row.get(3)?,
                        date: row.get(4)?,
                        unseen: row.get::<_, i64>(5)? != 0,
                        text: row.get(6)?,
                        html: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn search(&self, account_id: &str, query: &str) -> Result<Vec<MessageSummary>> {
        let needle = query.trim();
        if needle.is_empty() {
            return Ok(Vec::new());
        }
        // Trigram FTS5 accelerates LIKE/GLOB, which is the right way to search
        // Japanese without word segmentation (including 1–2 character queries).
        let like = format!("%{}%", escape_like(needle));
        let mut stmt = self.conn.prepare(
            "SELECT uid, folder, from_addr, subject, date, unseen
             FROM messages
             WHERE account_id = ?1
               AND id IN (
                    SELECT rowid FROM messages_fts
                    WHERE subject LIKE ?2 ESCAPE '\\'
                       OR from_addr LIKE ?2 ESCAPE '\\'
                       OR body_text LIKE ?2 ESCAPE '\\'
               )
             ORDER BY COALESCE(date_unix, uid) DESC, uid DESC",
        )?;
        let rows = stmt.query_map(params![account_id, like], |row| {
            Ok(MessageSummary {
                uid: row.get::<_, i64>(0)? as u32,
                folder: row.get(1)?,
                from: row.get(2)?,
                subject: row.get(3)?,
                date: row.get(4)?,
                unseen: row.get::<_, i64>(5)? != 0,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

fn escape_like(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mime::ParsedMail;

    fn seed(cache: &Cache) {
        let account = NewAccount {
            display_name: "Test".into(),
            imap_host: "127.0.0.1".into(),
            imap_port: 3143,
            tls_mode: TlsMode::None,
            username: "testuser".into(),
            accept_invalid_certs: true,
        };
        cache.insert_account("acc1", &account).unwrap();
        cache
            .upsert_summary(
                "acc1",
                &MessageSummary {
                    uid: 1,
                    folder: "INBOX".into(),
                    from: "Alice <alice@example.com>".into(),
                    subject: "Hello world".into(),
                    date: "2025-04-01T09:00:00+09:00".into(),
                    unseen: true,
                },
                Some("english body about invoices"),
                None,
            )
            .unwrap();
        cache
            .update_body(
                "acc1",
                "INBOX",
                1,
                &ParsedMail {
                    from: "Alice <alice@example.com>".into(),
                    subject: "Hello world".into(),
                    date: "2025-04-01T09:00:00+09:00".into(),
                    date_unix: Some(1),
                    text: Some("english body about invoices".into()),
                    html: None,
                },
                true,
            )
            .unwrap();
        cache
            .upsert_summary(
                "acc1",
                &MessageSummary {
                    uid: 2,
                    folder: "INBOX".into(),
                    from: "山田 <taro@example.com>".into(),
                    subject: "会議の件".into(),
                    date: "2025-04-01T10:00:00+09:00".into(),
                    unseen: true,
                },
                Some("来週の会議は火曜です。全文検索テスト。"),
                None,
            )
            .unwrap();
        cache
            .update_body(
                "acc1",
                "INBOX",
                2,
                &ParsedMail {
                    from: "山田 <taro@example.com>".into(),
                    subject: "会議の件".into(),
                    date: "2025-04-01T10:00:00+09:00".into(),
                    date_unix: Some(2),
                    text: Some("来週の会議は火曜です。全文検索テスト。".into()),
                    html: None,
                },
                true,
            )
            .unwrap();
    }

    #[test]
    fn english_and_japanese_search() {
        let cache = Cache::open_in_memory().unwrap();
        seed(&cache);

        let en = cache.search("acc1", "invoice").unwrap();
        assert_eq!(en.len(), 1);
        assert_eq!(en[0].subject, "Hello world");

        let ja = cache.search("acc1", "会議").unwrap();
        assert_eq!(ja.len(), 1);
        assert_eq!(ja[0].subject, "会議の件");

        let ja_body = cache.search("acc1", "全文検索").unwrap();
        assert_eq!(ja_body.len(), 1);
    }

    #[test]
    fn list_and_get_message() {
        let cache = Cache::open_in_memory().unwrap();
        seed(&cache);
        let listed = cache.list_messages("acc1", "INBOX").unwrap();
        assert_eq!(listed.len(), 2);
        let body = cache.get_message("acc1", "INBOX", 2).unwrap().unwrap();
        assert!(body.text.unwrap().contains("火曜"));
    }
}

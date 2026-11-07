use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS channels (
    id            INTEGER PRIMARY KEY,
    name          TEXT NOT NULL,
    kind          TEXT NOT NULL,          -- dm | group_dm | server | thread | unknown
    guild_id      INTEGER,
    guild_name    TEXT,
    message_count INTEGER NOT NULL DEFAULT 0,
    first_ts      INTEGER,
    last_ts       INTEGER
);

-- id is the Discord snowflake and doubles as SQLite's rowid (fast lookups, no extra index).
CREATE TABLE IF NOT EXISTS messages (
    id          INTEGER PRIMARY KEY,
    channel_id  INTEGER NOT NULL,
    ts          INTEGER NOT NULL,         -- unix ms, derived from the snowflake
    content     TEXT NOT NULL,
    attachments TEXT NOT NULL             -- space-separated URLs, as Discord writes them
);
CREATE INDEX IF NOT EXISTS messages_channel_ts ON messages(channel_id, ts);

CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    content,
    content = 'messages',
    content_rowid = 'id',
    tokenize = 'unicode61 remove_diacritics 2'
);

-- Findings store positions only. The sensitive text itself is never duplicated.
CREATE TABLE IF NOT EXISTS findings (
    id          INTEGER PRIMARY KEY,
    message_id  INTEGER NOT NULL,
    kind        TEXT NOT NULL,
    severity    INTEGER NOT NULL,
    start_byte  INTEGER NOT NULL,
    end_byte    INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS findings_kind ON findings(kind, message_id);
CREATE INDEX IF NOT EXISTS findings_message ON findings(message_id);
"#;

/// The local database. One file, owned by the user.
pub struct Store {
    pub(crate) conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        // secure_delete overwrites deleted content with zeros instead of leaving it on disk.
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA secure_delete = ON;
             PRAGMA temp_store = MEMORY;
             PRAGMA busy_timeout = 5000;",
        )?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    pub fn has_data(&self) -> Result<bool> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))?;
        Ok(n > 0)
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare_cached("SELECT value FROM meta WHERE key = ?1")?;
        let mut rows = stmt.query([key])?;
        Ok(match rows.next()? {
            Some(row) => Some(row.get(0)?),
            None => None,
        })
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO meta(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (key, value),
        )?;
        Ok(())
    }

    /// Removes every imported message, finding and setting, then compacts the file
    /// so the old content is not left behind in free pages.
    pub fn wipe(&mut self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM findings;
             DELETE FROM messages;
             DELETE FROM channels;
             DELETE FROM meta;
             INSERT INTO messages_fts(messages_fts) VALUES('delete-all');",
        )?;
        self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")?;
        Ok(())
    }
}

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

/// Bump when the schema changes in a way old files can't be read with.
const SCHEMA_VERSION: i64 = 2;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- One row per imported source (a platform's export, or the PC scan).
CREATE TABLE IF NOT EXISTS sources (
    platform    TEXT PRIMARY KEY,         -- discord | reddit | telegram | x | pc
    origin      TEXT NOT NULL,            -- the file or folders it came from
    imported_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS channels (
    id            INTEGER PRIMARY KEY,
    platform      TEXT NOT NULL,
    ext_id        TEXT NOT NULL,          -- the platform's own ID for it
    name          TEXT NOT NULL,
    kind          TEXT NOT NULL,          -- dm | group_dm | server | thread | community | posts | channel | saved | folder | unknown
    guild_id      TEXT,
    guild_name    TEXT,
    message_count INTEGER NOT NULL DEFAULT 0,
    first_ts      INTEGER,
    last_ts       INTEGER,
    UNIQUE (platform, ext_id)
);

-- `uid` is "<platform>:<the platform's own message ID>", stable across re-imports,
-- so review decisions survive importing a newer export.
CREATE TABLE IF NOT EXISTS messages (
    id          INTEGER PRIMARY KEY,
    uid         TEXT NOT NULL UNIQUE,
    channel_id  INTEGER NOT NULL,
    ts          INTEGER NOT NULL,         -- unix ms
    content     TEXT NOT NULL,
    attachments TEXT NOT NULL,            -- space-separated URLs; for files, the file's full path
    link        TEXT                      -- where to open it on the platform, if it has a web address
);
CREATE INDEX IF NOT EXISTS messages_channel_ts ON messages(channel_id, ts, id);

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
    end_byte    INTEGER NOT NULL,
    fp          TEXT NOT NULL DEFAULT ''  -- SHA-256 of the normalised value, to group repeats of the same value
);
CREATE INDEX IF NOT EXISTS findings_kind ON findings(kind, message_id);
CREATE INDEX IF NOT EXISTS findings_message ON findings(message_id);

-- What the user decided about a finding. Keyed by message uid and position,
-- so it survives rescans and re-imports.
CREATE TABLE IF NOT EXISTS reviews (
    uid         TEXT NOT NULL,
    kind        TEXT NOT NULL,
    start_byte  INTEGER NOT NULL,
    status      TEXT NOT NULL,            -- deleted | ok | later
    PRIMARY KEY (uid, kind, start_byte)
);

-- "Never flag this again" rules. Stores a SHA-256 fingerprint, never the text itself.
CREATE TABLE IF NOT EXISTS ignored (
    kind        TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    PRIMARY KEY (kind, fingerprint)
);
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
            "PRAGMA busy_timeout = 5000;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA secure_delete = ON;
             PRAGMA temp_store = MEMORY;",
        )?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let has_tables: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'messages'",
            [],
            |r| r.get(0),
        )?;
        if has_tables && version < SCHEMA_VERSION {
            // A file from an older Veil. Its data is all re-importable, so start clean.
            conn.execute_batch(
                "DROP TABLE IF EXISTS messages_fts; DROP TABLE IF EXISTS findings; DROP TABLE IF EXISTS reviews;
                 DROP TABLE IF EXISTS ignored; DROP TABLE IF EXISTS messages; DROP TABLE IF EXISTS channels;
                 DROP TABLE IF EXISTS sources; DROP TABLE IF EXISTS meta; VACUUM;",
            )?;
        }
        conn.execute_batch(SCHEMA)?;
        conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))?;
        let has_fp: bool = conn.query_row("SELECT COUNT(*) > 0 FROM pragma_table_info('findings') WHERE name = 'fp'", [], |r| r.get(0))?;
        let mut store = Self { conn };
        if !has_fp {
            // Added after the first release: fill it in for findings from earlier scans.
            store.conn.execute_batch("ALTER TABLE findings ADD COLUMN fp TEXT NOT NULL DEFAULT ''")?;
            store.backfill_fingerprints()?;
        }
        store.conn.execute_batch("CREATE INDEX IF NOT EXISTS findings_fp ON findings(kind, fp)")?;
        Ok(store)
    }

    pub fn has_data(&self) -> Result<bool> {
        let n: i64 = self.conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))?;
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

    /// Removes one source's messages and findings. Review decisions are kept,
    /// so importing a newer export of the same platform picks them up again.
    pub fn remove_source(&mut self, platform: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM findings WHERE message_id IN
               (SELECT m.id FROM messages m JOIN channels c ON c.id = m.channel_id WHERE c.platform = ?1)",
            [platform],
        )?;
        tx.execute(
            "DELETE FROM messages WHERE channel_id IN (SELECT id FROM channels WHERE platform = ?1)",
            [platform],
        )?;
        tx.execute("DELETE FROM channels WHERE platform = ?1", [platform])?;
        tx.execute("DELETE FROM sources WHERE platform = ?1", [platform])?;
        tx.commit()?;
        self.conn.execute_batch("INSERT INTO messages_fts(messages_fts) VALUES('rebuild');")?;
        if !self.has_data()? {
            self.conn.execute("DELETE FROM meta WHERE key = 'demo'", [])?;
        }
        Ok(())
    }

    /// Removes every imported message, finding and setting, then compacts the file
    /// so the old content is not left behind in free pages.
    pub fn wipe(&mut self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM findings;
             DELETE FROM reviews;
             DELETE FROM ignored;
             DELETE FROM messages;
             DELETE FROM channels;
             DELETE FROM sources;
             DELETE FROM meta;
             INSERT INTO messages_fts(messages_fts) VALUES('delete-all');",
        )?;
        self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")?;
        Ok(())
    }
}

//! Read-only views for the UI. IDs are sent as strings because Discord IDs are
//! bigger than JavaScript numbers can hold exactly.

use crate::Store;
use anyhow::Result;
use rusqlite::{params, Row};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ChannelView {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub guild_name: Option<String>,
    pub message_count: i64,
    pub first_ts: Option<i64>,
    pub last_ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageView {
    pub id: String,
    pub channel_id: String,
    pub channel_name: String,
    pub ts: i64,
    pub content: String,
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessagesPage {
    pub total: i64,
    pub offset: i64,
    pub items: Vec<MessageView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchPage {
    pub total: i64,
    pub offset: i64,
    pub items: Vec<MessageView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct YearCount {
    pub year: i64,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Stats {
    pub messages: i64,
    pub channels: i64,
    pub servers: i64,
    pub dms: i64,
    pub group_dms: i64,
    pub with_attachments: i64,
    pub first_ts: Option<i64>,
    pub last_ts: Option<i64>,
    pub imported_at_ms: Option<i64>,
    pub per_year: Vec<YearCount>,
    pub top_channels: Vec<ChannelView>,
}

const CHANNEL_COLS: &str = "id, name, kind, guild_name, message_count, first_ts, last_ts";

fn channel_row(r: &Row) -> rusqlite::Result<ChannelView> {
    Ok(ChannelView {
        id: r.get::<_, i64>(0)?.to_string(),
        name: r.get(1)?,
        kind: r.get(2)?,
        guild_name: r.get(3)?,
        message_count: r.get(4)?,
        first_ts: r.get(5)?,
        last_ts: r.get(6)?,
    })
}

const MESSAGE_COLS: &str =
    "m.id, m.channel_id, COALESCE(c.name, ''), m.ts, m.content, m.attachments";

fn message_row(r: &Row) -> rusqlite::Result<MessageView> {
    let att: String = r.get(5)?;
    Ok(MessageView {
        id: r.get::<_, i64>(0)?.to_string(),
        channel_id: r.get::<_, i64>(1)?.to_string(),
        channel_name: r.get(2)?,
        ts: r.get(3)?,
        content: r.get(4)?,
        attachments: att.split_whitespace().map(str::to_string).collect(),
    })
}

/// Turns what a person types into a safe FTS5 query: every word is quoted
/// (so symbols can't break the query) and the last word matches as a prefix.
pub(crate) fn fts_query(input: &str) -> Option<String> {
    let words: Vec<String> = input
        .split_whitespace()
        .map(|w| format!("\"{}\"", w.replace('"', "\"\"")))
        .collect();
    if words.is_empty() {
        return None;
    }
    let mut q = words.join(" ");
    q.push('*');
    Some(q)
}

fn parse_id(s: &str) -> Result<i64> {
    Ok(s.trim().parse()?)
}

impl Store {
    pub fn stats(&self) -> Result<Stats> {
        let c = &self.conn;
        let one = |sql: &str| -> Result<i64> { Ok(c.query_row(sql, [], |r| r.get(0))?) };
        let (first_ts, last_ts): (Option<i64>, Option<i64>) =
            c.query_row("SELECT MIN(ts), MAX(ts) FROM messages", [], |r| Ok((r.get(0)?, r.get(1)?)))?;

        let mut stmt = c.prepare(
            "SELECT CAST(strftime('%Y', ts / 1000, 'unixepoch') AS INTEGER) AS y, COUNT(*)
             FROM messages GROUP BY y ORDER BY y",
        )?;
        let per_year = stmt
            .query_map([], |r| Ok(YearCount { year: r.get(0)?, count: r.get(1)? }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut stmt = c.prepare(&format!(
            "SELECT {CHANNEL_COLS} FROM channels ORDER BY message_count DESC LIMIT 8"
        ))?;
        let top_channels = stmt.query_map([], channel_row)?.collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(Stats {
            messages: one("SELECT COUNT(*) FROM messages")?,
            channels: one("SELECT COUNT(*) FROM channels")?,
            servers: one("SELECT COUNT(DISTINCT guild_id) FROM channels WHERE guild_id IS NOT NULL")?,
            dms: one("SELECT COUNT(*) FROM channels WHERE kind = 'dm'")?,
            group_dms: one("SELECT COUNT(*) FROM channels WHERE kind = 'group_dm'")?,
            with_attachments: one("SELECT COUNT(*) FROM messages WHERE attachments <> ''")?,
            first_ts,
            last_ts,
            imported_at_ms: self.get_meta("imported_at_ms")?.and_then(|s| s.parse().ok()),
            per_year,
            top_channels,
        })
    }

    /// All channels, busiest first. `filter` matches the name, case-insensitively.
    pub fn channels(&self, filter: Option<&str>) -> Result<Vec<ChannelView>> {
        let pattern = format!("%{}%", filter.unwrap_or("").trim().replace(['%', '_'], ""));
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {CHANNEL_COLS} FROM channels
             WHERE name LIKE ?1 OR COALESCE(guild_name, '') LIKE ?1
             ORDER BY message_count DESC, name"
        ))?;
        let rows = stmt.query_map([pattern], channel_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Messages in one channel, oldest first, a page at a time.
    pub fn messages(&self, channel_id: &str, offset: i64, limit: i64) -> Result<MessagesPage> {
        let cid = parse_id(channel_id)?;
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE channel_id = ?1",
            [cid],
            |r| r.get(0),
        )?;
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {MESSAGE_COLS} FROM messages m LEFT JOIN channels c ON c.id = m.channel_id
             WHERE m.channel_id = ?1 ORDER BY m.ts, m.id LIMIT ?2 OFFSET ?3"
        ))?;
        let items = stmt
            .query_map(params![cid, limit.clamp(1, 2000), offset.max(0)], message_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(MessagesPage { total, offset, items })
    }

    /// Position of a message within its channel (same order as `messages`), for jumping to it.
    pub fn message_index(&self, channel_id: &str, message_id: &str) -> Result<i64> {
        let (cid, mid) = (parse_id(channel_id)?, parse_id(message_id)?);
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM messages a, messages b
             WHERE b.id = ?2 AND a.channel_id = ?1 AND (a.ts < b.ts OR (a.ts = b.ts AND a.id < b.id))",
            params![cid, mid],
            |r| r.get(0),
        )?)
    }

    /// Full-text search across everything, newest first. Optional channel scope.
    pub fn search(&self, text: &str, channel_id: Option<&str>, offset: i64, limit: i64) -> Result<SearchPage> {
        let Some(q) = fts_query(text) else {
            return Ok(SearchPage { total: 0, offset, items: vec![] });
        };
        let cid = channel_id.map(parse_id).transpose()?;
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages_fts f JOIN messages m ON m.id = f.rowid
             WHERE messages_fts MATCH ?1 AND (?2 IS NULL OR m.channel_id = ?2)",
            params![q, cid],
            |r| r.get(0),
        )?;
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {MESSAGE_COLS} FROM messages_fts f
             JOIN messages m ON m.id = f.rowid
             LEFT JOIN channels c ON c.id = m.channel_id
             WHERE messages_fts MATCH ?1 AND (?2 IS NULL OR m.channel_id = ?2)
             ORDER BY m.ts DESC LIMIT ?3 OFFSET ?4"
        ))?;
        let items = stmt
            .query_map(params![q, cid, limit.clamp(1, 2000), offset.max(0)], message_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(SearchPage { total, offset, items })
    }
}

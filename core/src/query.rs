//! Read-only views for the UI. IDs are sent as strings because some are bigger
//! than JavaScript numbers can hold exactly.

use crate::import::platform_label;
use crate::scan::{mask_text, merge_spans};
use crate::Store;
use anyhow::Result;
use rusqlite::{params, Row};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ChannelView {
    pub id: String,
    pub platform: String,
    pub name: String,
    pub kind: String,
    pub guild_name: Option<String>,
    pub message_count: i64,
    pub first_ts: Option<i64>,
    pub last_ts: Option<i64>,
}

/// A piece of a message. Sensitive pieces arrive masked unless revealed.
#[derive(Debug, Clone, Serialize)]
pub struct Segment {
    pub text: String,
    pub sensitive: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageView {
    pub id: String,
    pub channel_id: String,
    pub channel_name: String,
    pub platform: String,
    pub ts: i64,
    /// Full text, with sensitive parts masked unless revealed.
    pub content: String,
    pub segments: Vec<Segment>,
    pub has_sensitive: bool,
    pub attachments: Vec<String>,
    pub link: Option<String>,
    pub file_path: Option<String>,
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
pub struct SourceView {
    pub platform: String,
    pub label: &'static str,
    pub origin: String,
    pub imported_at: i64,
    pub channels: i64,
    pub messages: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Stats {
    pub messages: i64,
    pub channels: i64,
    pub servers: i64,
    pub dms: i64,
    pub group_dms: i64,
    pub files: i64,
    pub with_attachments: i64,
    pub first_ts: Option<i64>,
    pub last_ts: Option<i64>,
    pub imported_at_ms: Option<i64>,
    pub per_year: Vec<YearCount>,
    pub top_channels: Vec<ChannelView>,
    pub sources: Vec<SourceView>,
}

const CHANNEL_COLS: &str = "id, platform, name, kind, guild_name, message_count, first_ts, last_ts";

fn channel_row(r: &Row) -> rusqlite::Result<ChannelView> {
    Ok(ChannelView {
        id: r.get::<_, i64>(0)?.to_string(),
        platform: r.get(1)?,
        name: r.get(2)?,
        kind: r.get(3)?,
        guild_name: r.get(4)?,
        message_count: r.get(5)?,
        first_ts: r.get(6)?,
        last_ts: r.get(7)?,
    })
}

const MESSAGE_COLS: &str = "m.id, m.channel_id, c.name, c.platform, m.ts, m.content, m.attachments, m.link";
const MESSAGE_FROM: &str = "FROM messages m JOIN channels c ON c.id = m.channel_id";

struct RawMessage {
    id: i64,
    channel_id: i64,
    channel_name: String,
    platform: String,
    ts: i64,
    content: String,
    attachments: String,
    link: Option<String>,
}

fn message_row(r: &Row) -> rusqlite::Result<RawMessage> {
    Ok(RawMessage {
        id: r.get(0)?,
        channel_id: r.get(1)?,
        channel_name: r.get(2)?,
        platform: r.get(3)?,
        ts: r.get(4)?,
        content: r.get(5)?,
        attachments: r.get(6)?,
        link: r.get(7)?,
    })
}

/// Turns what a person types into a safe FTS5 query: every word is quoted
/// (so symbols can't break the query) and the last word matches as a prefix.
pub(crate) fn fts_query(input: &str) -> Option<String> {
    let words: Vec<String> = input.split_whitespace().map(|w| format!("\"{}\"", w.replace('"', "\"\""))).collect();
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
    pub fn sources(&self) -> Result<Vec<SourceView>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.platform, s.origin, s.imported_at,
                    (SELECT COUNT(*) FROM channels c WHERE c.platform = s.platform),
                    (SELECT COALESCE(SUM(message_count), 0) FROM channels c WHERE c.platform = s.platform)
             FROM sources s ORDER BY s.imported_at",
        )?;
        let rows = stmt
            .query_map([], |r| {
                let platform: String = r.get(0)?;
                Ok(SourceView {
                    label: platform_label(&platform),
                    platform,
                    origin: r.get(1)?,
                    imported_at: r.get(2)?,
                    channels: r.get(3)?,
                    messages: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn is_demo(&self) -> Result<bool> {
        Ok(self.get_meta("demo")?.as_deref() == Some("1"))
    }

    pub fn stats(&self) -> Result<Stats> {
        let c = &self.conn;
        let one = |sql: &str| -> Result<i64> { Ok(c.query_row(sql, [], |r| r.get(0))?) };
        // Messages only: files from the PC scan would skew dates and counts.
        let (first_ts, last_ts): (Option<i64>, Option<i64>) = c.query_row(
            &format!("SELECT MIN(m.ts), MAX(m.ts) {MESSAGE_FROM} WHERE c.platform <> 'pc' AND m.ts > 0"),
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let mut stmt = c.prepare(&format!(
            "SELECT CAST(strftime('%Y', m.ts / 1000, 'unixepoch') AS INTEGER) AS y, COUNT(*)
             {MESSAGE_FROM} WHERE c.platform <> 'pc' AND m.ts > 0 GROUP BY y ORDER BY y"
        ))?;
        let per_year = stmt
            .query_map([], |r| Ok(YearCount { year: r.get(0)?, count: r.get(1)? }))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut stmt = c.prepare(&format!(
            "SELECT {CHANNEL_COLS} FROM channels WHERE platform <> 'pc' ORDER BY message_count DESC LIMIT 8"
        ))?;
        let top_channels = stmt.query_map([], channel_row)?.collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(Stats {
            messages: one(&format!("SELECT COUNT(*) {MESSAGE_FROM} WHERE c.platform <> 'pc'"))?,
            channels: one("SELECT COUNT(*) FROM channels WHERE platform <> 'pc'")?,
            servers: one("SELECT COUNT(DISTINCT platform || guild_id) FROM channels WHERE guild_id IS NOT NULL")?,
            dms: one("SELECT COUNT(*) FROM channels WHERE kind = 'dm'")?,
            group_dms: one("SELECT COUNT(*) FROM channels WHERE kind = 'group_dm'")?,
            files: one(&format!("SELECT COUNT(*) {MESSAGE_FROM} WHERE c.platform = 'pc'"))?,
            with_attachments: one(&format!(
                "SELECT COUNT(*) {MESSAGE_FROM} WHERE c.platform <> 'pc' AND m.attachments <> ''"
            ))?,
            first_ts,
            last_ts,
            imported_at_ms: self.get_meta("imported_at_ms")?.and_then(|s| s.parse().ok()),
            per_year,
            top_channels,
            sources: self.sources()?,
        })
    }

    /// All channels (and scanned folders), busiest first. `filter` matches the name, case-insensitively.
    pub fn channels(&self, filter: Option<&str>) -> Result<Vec<ChannelView>> {
        let pattern = format!("%{}%", filter.unwrap_or("").trim().replace(['%', '_'], ""));
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {CHANNEL_COLS} FROM channels
             WHERE name LIKE ?1 OR COALESCE(guild_name, '') LIKE ?1
             ORDER BY platform = 'pc', message_count DESC, name"
        ))?;
        let rows = stmt.query_map([pattern], channel_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Messages in one channel, oldest first, a page at a time.
    pub fn messages(&self, channel_id: &str, offset: i64, limit: i64) -> Result<MessagesPage> {
        let cid = parse_id(channel_id)?;
        let total: i64 =
            self.conn.query_row("SELECT COUNT(*) FROM messages WHERE channel_id = ?1", [cid], |r| r.get(0))?;
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {MESSAGE_COLS} {MESSAGE_FROM} WHERE m.channel_id = ?1 ORDER BY m.ts, m.id LIMIT ?2 OFFSET ?3"
        ))?;
        let raw = stmt
            .query_map(params![cid, limit.clamp(1, 2000), offset.max(0)], message_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(MessagesPage { total, offset, items: self.finish(raw, false)? })
    }

    /// One message with its sensitive parts shown in full.
    pub fn reveal_message(&self, message_id: &str) -> Result<Option<MessageView>> {
        let mid = parse_id(message_id)?;
        let mut stmt = self.conn.prepare_cached(&format!("SELECT {MESSAGE_COLS} {MESSAGE_FROM} WHERE m.id = ?1"))?;
        let raw = stmt.query_map([mid], message_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(self.finish(raw, true)?.pop())
    }

    /// Splits each message into plain and sensitive pieces, masking the sensitive ones.
    fn finish(&self, raw: Vec<RawMessage>, reveal: bool) -> Result<Vec<MessageView>> {
        raw.into_iter()
            .map(|m| {
                let spans = merge_spans(&m.content, self.spans_for(m.id)?);
                let mut segments = Vec::new();
                let mut pos = 0;
                for (s, e) in &spans {
                    if *s > pos {
                        segments.push(Segment { text: m.content[pos..*s].to_string(), sensitive: false });
                    }
                    let part = &m.content[*s..*e];
                    segments.push(Segment { text: if reveal { part.to_string() } else { mask_text(part) }, sensitive: true });
                    pos = *e;
                }
                if pos < m.content.len() || segments.is_empty() {
                    segments.push(Segment { text: m.content[pos..].to_string(), sensitive: false });
                }
                let is_file = m.platform == "pc";
                Ok(MessageView {
                    id: m.id.to_string(),
                    channel_id: m.channel_id.to_string(),
                    channel_name: m.channel_name,
                    content: segments.iter().map(|s| s.text.as_str()).collect(),
                    has_sensitive: !spans.is_empty(),
                    segments,
                    attachments: if is_file { vec![] } else { m.attachments.split_whitespace().map(str::to_string).collect() },
                    file_path: is_file.then_some(m.attachments),
                    link: m.link,
                    platform: m.platform,
                    ts: m.ts,
                })
            })
            .collect()
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
             JOIN channels c ON c.id = m.channel_id
             WHERE messages_fts MATCH ?1 AND (?2 IS NULL OR m.channel_id = ?2)
             ORDER BY m.ts DESC LIMIT ?3 OFFSET ?4"
        ))?;
        let raw = stmt
            .query_map(params![q, cid, limit.clamp(1, 2000), offset.max(0)], message_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(SearchPage { total, offset, items: self.finish(raw, false)? })
    }
}

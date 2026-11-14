//! Where the calendar lives: an encrypted SQLite file (same cipher as Clean's), holding each
//! schedule as its original CSV text, so nothing about the Desk Reminders format is lost.

use crate::engine::{Alert, AlertMode, Quiet};
use crate::format::{parse, RowInput, Schedule, HEADER};
use anyhow::{anyhow, bail, Context, Result};
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The colours schedules get, in order: yours blue, then green, purple…
pub const PALETTE: [&str; 9] = ["#5B8DEF", "#4FB477", "#9B7BE0", "#E08A4F", "#3FB8B8", "#E06C9F", "#D4B24C", "#8A9BB0", "#E05C5C"];

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS schedules (
    id          INTEGER PRIMARY KEY,
    mine        INTEGER NOT NULL DEFAULT 0,  -- your own schedule (exactly one)
    owned       INTEGER NOT NULL DEFAULT 0,  -- a shared schedule you made, so you can edit it
    csv         TEXT NOT NULL,
    colour      TEXT NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 1,
    sound       TEXT,
    my_groups   TEXT NOT NULL DEFAULT '[]',
    quiet_allow INTEGER NOT NULL DEFAULT 0,
    source      TEXT,                        -- where it was imported from
    added_ms    INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS alerts (
    key   TEXT NOT NULL,
    date  TEXT NOT NULL DEFAULT '',          -- '' = every time
    mode  TEXT NOT NULL,
    PRIMARY KEY (key, date)
);
CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

#[derive(Debug, Clone, Serialize)]
pub struct ScheduleView {
    pub id: i64,
    pub name: String,
    pub mine: bool,
    pub owned: bool,
    pub editable: bool,
    pub colour: String,
    pub enabled: bool,
    pub sound: Option<String>,
    pub groups: Vec<String>,
    pub my_groups: Vec<String>,
    pub quiet_allow: bool,
    pub timezone: String,
    pub updated: String,
    pub source: Option<String>,
    pub reminders: usize,
    pub problems: Vec<crate::format::Problem>,
}

pub struct Loaded {
    pub view: ScheduleView,
    pub schedule: Schedule,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub quiet: Quiet,
    pub sound: String,
    pub sound_on: bool,
    pub volume: u8,
    pub corner: String,
    pub popup_seconds: u32,
    /// Hide reminder text on pop-ups (only the schedule name shows)
    pub discreet: bool,
    /// Keep pop-ups out of screen shares, recordings and streams (Windows 10 2004 and later)
    pub hide_from_capture: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            quiet: Quiet { on: false, from: 23 * 60, to: 8 * 60 },
            sound: "Soft chime".into(),
            sound_on: true,
            volume: 40,
            corner: "bottom-right".into(),
            popup_seconds: 25,
            discreet: false,
            hide_from_capture: true,
        }
    }
}

pub struct CalendarStore {
    conn: Connection,
}

fn apply_key(conn: &Connection, key: &[u8; 32]) -> Result<()> {
    let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
    conn.execute_batch(&format!("PRAGMA cipher = 'chacha20'; PRAGMA kdf_iter = 1; PRAGMA key = '{hex}';"))?;
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

impl CalendarStore {
    pub fn open_encrypted(path: &Path, key: &[u8; 32]) -> Result<Self> {
        let conn = Connection::open(path)?;
        apply_key(&conn, key)?;
        if conn.query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)).is_err() {
            bail!("The calendar didn't open with this key.");
        }
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA secure_delete = ON; PRAGMA busy_timeout = 5000;")?;
        conn.execute_batch(SCHEMA)?;
        let store = Self { conn };
        let has_mine: bool = store.conn.query_row("SELECT COUNT(*) > 0 FROM schedules WHERE mine = 1", [], |r| r.get(0))?;
        if !has_mine {
            store.conn.execute(
                "INSERT INTO schedules(mine, owned, csv, colour, added_ms) VALUES (1, 1, ?1, ?2, ?3)",
                params![format!("{HEADER}\n"), PALETTE[0], now_ms()],
            )?;
        }
        Ok(store)
    }

    pub fn mine_id(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT id FROM schedules WHERE mine = 1", [], |r| r.get(0))?)
    }

    pub fn load(&self) -> Result<Vec<Loaded>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, mine, owned, csv, colour, enabled, sound, my_groups, quiet_allow, source FROM schedules ORDER BY mine DESC, added_ms",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, bool>(1)?,
                r.get::<_, bool>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, bool>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, bool>(8)?,
                r.get::<_, Option<String>>(9)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, mine, owned, csv, colour, enabled, sound, my_groups, quiet_allow, source) = row?;
            let schedule = parse(&csv);
            let name = if mine {
                if schedule.name.is_empty() { "My schedule".to_string() } else { schedule.name.clone() }
            } else if schedule.name.is_empty() {
                "Shared schedule".to_string()
            } else {
                schedule.name.clone()
            };
            let view = ScheduleView {
                id,
                name,
                mine,
                owned,
                editable: mine || owned,
                colour,
                enabled,
                sound,
                groups: schedule.groups.clone(),
                my_groups: serde_json::from_str(&my_groups).unwrap_or_default(),
                quiet_allow,
                timezone: schedule.timezone.clone(),
                updated: schedule.updated.clone(),
                source,
                reminders: schedule.rows.iter().filter(|r| !r.skip).count(),
                problems: schedule.problems.clone(),
            };
            out.push(Loaded { view, schedule });
        }
        Ok(out)
    }

    fn csv(&self, id: i64) -> Result<(String, bool)> {
        self.conn
            .query_row("SELECT csv, mine OR owned FROM schedules WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?
            .ok_or_else(|| anyhow!("That schedule no longer exists."))
    }

    fn save_csv(&self, id: i64, csv: &str) -> Result<()> {
        self.conn.execute("UPDATE schedules SET csv = ?2 WHERE id = ?1", params![id, csv])?;
        Ok(())
    }

    fn editable_lines(&self, id: i64) -> Result<(Vec<String>, u8)> {
        let (csv, editable) = self.csv(id)?;
        if !editable {
            bail!("This is someone else's shared schedule, so it can't be changed here. You can mute or hide its events.");
        }
        let delim = parse(&csv).delimiter;
        let mut lines: Vec<String> = csv.lines().map(str::to_string).collect();
        if !parse(&csv).has_header {
            lines.push(HEADER.into());
        }
        Ok((lines, delim))
    }

    fn touch_updated(lines: &mut Vec<String>, mine: bool) {
        if mine {
            return;
        }
        let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        if let Some(l) = lines.iter_mut().find(|l| l.trim_start().starts_with("#!") && l.to_lowercase().contains("updated")) {
            *l = format!("#! updated = {stamp}");
        } else {
            lines.insert(0, format!("#! updated = {stamp}"));
        }
    }

    fn is_mine(&self, id: i64) -> Result<bool> {
        Ok(self.conn.query_row("SELECT mine FROM schedules WHERE id = ?1", [id], |r| r.get(0))?)
    }

    pub fn add_row(&self, id: i64, row: &RowInput) -> Result<()> {
        row.check().map_err(|e| anyhow!(e))?;
        let (mut lines, delim) = self.editable_lines(id)?;
        lines.push(row.to_line(delim));
        Self::touch_updated(&mut lines, self.is_mine(id)?);
        self.save_csv(id, &(lines.join("\n") + "\n"))
    }

    pub fn add_rows(&self, id: i64, rows: &[RowInput]) -> Result<()> {
        for r in rows {
            r.check().map_err(|e| anyhow!(e))?;
        }
        let (mut lines, delim) = self.editable_lines(id)?;
        lines.extend(rows.iter().map(|r| r.to_line(delim)));
        Self::touch_updated(&mut lines, self.is_mine(id)?);
        self.save_csv(id, &(lines.join("\n") + "\n"))
    }

    /// Replaces the reminder on `line` (1-based, as the engine reports it).
    pub fn update_row(&self, id: i64, line: usize, row: &RowInput) -> Result<()> {
        row.check().map_err(|e| anyhow!(e))?;
        let (mut lines, delim) = self.editable_lines(id)?;
        let slot = lines.get_mut(line.wrapping_sub(1)).ok_or_else(|| anyhow!("That reminder moved. Refresh and try again."))?;
        *slot = row.to_line(delim);
        Self::touch_updated(&mut lines, self.is_mine(id)?);
        self.save_csv(id, &(lines.join("\n") + "\n"))
    }

    /// Removes the whole reminder.
    pub fn remove_row(&self, id: i64, line: usize) -> Result<()> {
        let (mut lines, _) = self.editable_lines(id)?;
        if line == 0 || line > lines.len() {
            bail!("That reminder moved. Refresh and try again.");
        }
        lines.remove(line - 1);
        Self::touch_updated(&mut lines, self.is_mine(id)?);
        self.save_csv(id, &(lines.join("\n") + "\n"))
    }

    /// Removes one occurrence of a repeating reminder: a skip row for that date and time.
    pub fn skip_once(&self, id: i64, zone_date: NaiveDate, zone_time: &str) -> Result<()> {
        let r = RowInput { days: zone_date.format("%Y-%m-%d").to_string(), time: zone_time.into(), skip: true, ..Default::default() };
        self.add_row(id, &r)
    }

    /// Adds a schedule file (a shared one, or Desk Reminders' own schedule when `as_mine`).
    pub fn import(&self, text: &str, name_hint: &str, source: Option<&str>, as_mine: bool) -> Result<i64> {
        let s = parse(text);
        if !s.has_header {
            bail!("This doesn't look like a schedule file: it has no Days,Time,… header row.");
        }
        if as_mine {
            let id = self.mine_id()?;
            let (current, _) = self.csv(id)?;
            let cur = parse(&current);
            if cur.rows.is_empty() {
                self.save_csv(id, text)?;
            } else {
                let mut lines: Vec<String> = current.lines().map(str::to_string).collect();
                let delim = cur.delimiter;
                for r in &s.rows {
                    let input = RowInput {
                        days: r.days_text.clone(),
                        time: r.time_text.clone(),
                        skip: r.skip,
                        title: r.title.clone(),
                        text: r.text.clone(),
                        notice: r.notice,
                        for_groups: r.for_groups.clone(),
                    };
                    lines.push(input.to_line(delim));
                }
                self.save_csv(id, &(lines.join("\n") + "\n"))?;
            }
            return Ok(id);
        }
        let mut text = text.to_string();
        if s.name.is_empty() {
            let name = name_hint.trim();
            text = format!("#! name = {}\n{text}", if name.is_empty() { "Shared schedule" } else { name });
        }
        let n: usize = self.conn.query_row("SELECT COUNT(*) FROM schedules", [], |r| r.get::<_, i64>(0))? as usize;
        let colour = PALETTE[n % PALETTE.len()];
        self.conn.execute(
            "INSERT INTO schedules(mine, owned, csv, colour, quiet_allow, source, added_ms) VALUES (0, 0, ?1, ?2, 1, ?3, ?4)",
            params![text, colour, source, now_ms()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// A new shared schedule you organise (and can edit).
    pub fn create_shared(&self, name: &str, timezone: &str, groups: &[String]) -> Result<i64> {
        if name.trim().is_empty() {
            bail!("Give the schedule a name.");
        }
        if !timezone.is_empty() && crate::format::resolve_zone(timezone).is_none() {
            bail!("\"{timezone}\" isn't a time zone the calendar knows.");
        }
        let mut text = format!("#! name = {}\n", name.trim());
        if !timezone.is_empty() {
            text.push_str(&format!("#! timezone = {timezone}\n"));
        }
        if !groups.is_empty() {
            text.push_str(&format!("#! groups = {}\n", groups.join(" | ")));
        }
        text.push_str(HEADER);
        text.push('\n');
        let id = self.import(&text, name, None, false)?;
        self.conn.execute("UPDATE schedules SET owned = 1, my_groups = ?2 WHERE id = ?1", params![id, serde_json::to_string(groups)?])?;
        Ok(id)
    }

    pub fn export(&self, id: i64) -> Result<String> {
        Ok(self.csv(id)?.0)
    }

    pub fn set_options(&self, id: i64, colour: Option<&str>, enabled: Option<bool>, sound: Option<Option<&str>>, my_groups: Option<&[String]>, quiet_allow: Option<bool>) -> Result<()> {
        if let Some(c) = colour {
            self.conn.execute("UPDATE schedules SET colour = ?2 WHERE id = ?1", params![id, c])?;
        }
        if let Some(e) = enabled {
            self.conn.execute("UPDATE schedules SET enabled = ?2 WHERE id = ?1", params![id, e])?;
        }
        if let Some(s) = sound {
            self.conn.execute("UPDATE schedules SET sound = ?2 WHERE id = ?1", params![id, s])?;
        }
        if let Some(g) = my_groups {
            self.conn.execute("UPDATE schedules SET my_groups = ?2 WHERE id = ?1", params![id, serde_json::to_string(g)?])?;
        }
        if let Some(q) = quiet_allow {
            self.conn.execute("UPDATE schedules SET quiet_allow = ?2 WHERE id = ?1", params![id, q])?;
        }
        Ok(())
    }

    pub fn rename(&self, id: i64, name: &str) -> Result<()> {
        let (csv, _) = self.csv(id)?;
        let mut lines: Vec<String> = csv.lines().filter(|l| !(l.trim_start().starts_with("#!") && l.to_lowercase().replace(' ', "").starts_with("#!name"))).map(str::to_string).collect();
        if !name.trim().is_empty() {
            lines.insert(0, format!("#! name = {}", name.trim()));
        }
        self.save_csv(id, &(lines.join("\n") + "\n"))
    }

    pub fn remove_schedule(&self, id: i64) -> Result<()> {
        if self.is_mine(id)? {
            bail!("Your own schedule can't be removed. You can remove its reminders instead.");
        }
        self.conn.execute("DELETE FROM schedules WHERE id = ?1", [id])?;
        self.conn.execute("DELETE FROM alerts WHERE key LIKE ?1", [format!("{id}|%")])?;
        Ok(())
    }

    pub fn alerts(&self) -> Result<Vec<Alert>> {
        let mut stmt = self.conn.prepare("SELECT key, date, mode FROM alerts")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (key, date, mode) = row?;
            let mode = match mode.as_str() {
                "mute" => AlertMode::Mute,
                "shout" => AlertMode::Shout,
                "hide" => AlertMode::Hide,
                _ => continue,
            };
            out.push(Alert { key, date: NaiveDate::parse_from_str(&date, "%Y-%m-%d").ok(), mode });
        }
        Ok(out)
    }

    /// Sets or clears (mode None) a mute, "notify me too" or hide, for one day or every time.
    pub fn set_alert(&self, key: &str, date: Option<NaiveDate>, mode: Option<AlertMode>) -> Result<()> {
        let d = date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default();
        match mode {
            None => {
                self.conn.execute("DELETE FROM alerts WHERE key = ?1 AND date = ?2", params![key, d])?;
            }
            Some(m) => {
                let m = match m {
                    AlertMode::Mute => "mute",
                    AlertMode::Shout => "shout",
                    AlertMode::Hide => "hide",
                };
                self.conn.execute(
                    "INSERT INTO alerts(key, date, mode) VALUES (?1, ?2, ?3) ON CONFLICT(key, date) DO UPDATE SET mode = excluded.mode",
                    params![key, d, m],
                )?;
            }
        }
        Ok(())
    }

    pub fn settings(&self) -> Result<Settings> {
        let v: Option<String> = self.conn.query_row("SELECT value FROM settings WHERE key = 'settings'", [], |r| r.get(0)).optional()?;
        Ok(v.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default())
    }

    pub fn set_settings(&self, s: &Settings) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings(key, value) VALUES ('settings', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [serde_json::to_string(s)?],
        )?;
        Ok(())
    }

    /// Every reminder and notice between two dates (inclusive), in local time.
    pub fn occurrences_between<L: chrono::TimeZone>(&self, from: NaiveDate, to: NaiveDate, local: &L) -> Result<Vec<crate::engine::Occurrence>> {
        let loaded = self.load()?;
        let alerts = self.alerts()?;
        let sources: Vec<crate::engine::Source> = loaded
            .iter()
            .map(|l| crate::engine::Source {
                id: l.view.id,
                name: &l.view.name,
                schedule: &l.schedule,
                mine: l.view.mine,
                enabled: l.view.enabled,
                my_groups: &l.view.my_groups,
                colour: &l.view.colour,
                quiet_allow: l.view.quiet_allow,
            })
            .collect();
        Ok(crate::engine::occurrences(&sources, from, to, &alerts, local))
    }

    /// Brings in a Desk Reminders folder: schedule.csv, the shared folder, and the settings that
    /// carry over (colours, your groups, quiet hours, sound). Returns how many schedules came in.
    pub fn import_desk_reminders(&self, folder: &Path) -> Result<usize> {
        let mut count = 0;
        let mine = folder.join("schedule.csv");
        if !mine.exists() {
            bail!("No schedule.csv in that folder. Choose the folder Desk Reminders runs from.");
        }
        let settings: serde_json::Value = std::fs::read_to_string(folder.join("settings.json"))
            .ok()
            .and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok())
            .unwrap_or(serde_json::Value::Null);
        self.import(&read_text(&mine)?, "", Some(&mine.display().to_string()), true)?;
        count += 1;
        if let Ok(dir) = std::fs::read_dir(folder.join("shared")) {
            let mut files: Vec<_> = dir.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv"))).collect();
            files.sort();
            for f in files {
                let file_name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let stem = f.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let Ok(id) = self.import(&read_text(&f)?, &stem, Some(&f.display().to_string()), false) else { continue };
                count += 1;
                let groups: Vec<String> = settings["MyGroups"][&file_name]
                    .as_array()
                    .map(|a| a.iter().filter_map(|g| g.as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                let disabled = settings["Disabled"].as_array().is_some_and(|a| a.iter().any(|x| x.as_str() == Some(&file_name)));
                self.set_options(id, None, Some(!disabled), None, Some(&groups), None)?;
            }
        }
        let mut s = self.settings()?;
        if let Some(q) = settings.get("Quiet") {
            let clock = |k: &str| q[k].as_str().and_then(crate::format::parse_clock);
            if let (Some(from), Some(to)) = (clock("From"), clock("To")) {
                s.quiet = Quiet { on: q["On"].as_bool().unwrap_or(false), from, to };
            }
        }
        if let Some(v) = settings["Volume"].as_u64() {
            s.volume = v.min(100) as u8;
        }
        if let Some(on) = settings["SoundOn"].as_bool() {
            s.sound_on = on;
        }
        if let Some(snd) = settings["Sound"].as_str() {
            s.sound = snd.trim_end_matches(".wav").to_string();
        }
        self.set_settings(&s)?;
        Ok(count)
    }
}

/// Desk Reminders files are UTF-8, sometimes with a byte-order mark; Excel may save them as Windows-1252.
pub fn read_text(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("Couldn't read {}", path.display()))?;
    Ok(match String::from_utf8(bytes) {
        Ok(s) => s.trim_start_matches('\u{feff}').to_string(),
        Err(e) => e.into_bytes().iter().map(|&b| b as char).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{occurrences, Source};

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn edit_rows_and_skip_one_day() {
        let s = CalendarStore::open_in_memory().unwrap();
        let mine = s.mine_id().unwrap();
        s.add_row(mine, &RowInput { days: "Weekdays".into(), time: "every 60 min 10:00-12:00".into(), title: "Stand up".into(), ..Default::default() }).unwrap();
        s.add_row(mine, &RowInput { days: "Every day".into(), time: "08:00".into(), title: "Breakfast".into(), notice: 10, ..Default::default() }).unwrap();
        assert!(s.add_row(mine, &RowInput { days: "Weekdays".into(), time: "9".into(), title: "x".into(), ..Default::default() }).is_err());
        let loaded = s.load().unwrap();
        assert_eq!(loaded[0].view.reminders, 2);
        let line = loaded[0].schedule.rows.iter().find(|r| r.title == "Stand up").unwrap().line;
        s.skip_once(mine, d(2026, 6, 1), "11:00").unwrap();
        let loaded = s.load().unwrap();
        let src = [Source { id: mine, name: "My schedule", schedule: &loaded[0].schedule, mine: true, enabled: true, my_groups: &[], colour: "", quiet_allow: false }];
        let mon = occurrences(&src, d(2026, 6, 1), d(2026, 6, 1), &[], &chrono_tz::UTC);
        assert_eq!(mon.iter().filter(|o| o.title == "Stand up").count(), 2, "11:00 skipped");
        s.update_row(mine, line, &RowInput { days: "Weekdays".into(), time: "every 30 min 10:00-11:00".into(), title: "Move".into(), ..Default::default() }).unwrap();
        s.remove_row(mine, s.load().unwrap()[0].schedule.rows.iter().find(|r| r.title == "Breakfast").unwrap().line).unwrap();
        let titles: Vec<String> = s.load().unwrap()[0].schedule.rows.iter().filter(|r| !r.skip).map(|r| r.title.clone()).collect();
        assert_eq!(titles, ["Move"]);
    }

    #[test]
    fn shared_schedules_are_read_only_unless_yours() {
        let s = CalendarStore::open_in_memory().unwrap();
        let id = s.import("Days,Time,Colour,Title,Text,Notice,For\nWed,20:00,,Meeting,,30,\n", "Team Schedule", None, false).unwrap();
        let v = s.load().unwrap();
        assert_eq!(v[1].view.name, "Team Schedule");
        assert!(!v[1].view.editable);
        assert!(s.add_row(id, &RowInput { days: "Mon".into(), time: "10:00".into(), title: "x".into(), ..Default::default() }).is_err());
        let own = s.create_shared("Project team", "Europe/Amsterdam", &["Design".into()]).unwrap();
        s.add_row(own, &RowInput { days: "Thu".into(), time: "20:00".into(), title: "Review".into(), for_groups: vec!["Design".into()], ..Default::default() }).unwrap();
        let text = s.export(own).unwrap();
        assert!(text.contains("#! timezone = Europe/Amsterdam") && text.contains("#! updated = ") && text.contains("Review,,,Design"));
        assert!(s.remove_schedule(s.mine_id().unwrap()).is_err());
        s.remove_schedule(id).unwrap();
        assert_eq!(s.load().unwrap().len(), 2);
    }

    #[test]
    fn imports_a_desk_reminders_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("schedule.csv"), "\u{feff}Days,Time,Colour,Title,Text,Notice,For\nEvery day,08:30,,Breakfast,Eat.,,\n").unwrap();
        std::fs::create_dir(dir.path().join("shared")).unwrap();
        std::fs::write(dir.path().join("shared/Team.csv"), "#! groups = A | B\nDays,Time,Colour,Title,Text,Notice,For\nWed,20:00,,Meeting,,30,A\n").unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"Sound":"Warm bell.wav","Volume":70,"Quiet":{"On":true,"From":"22:30","To":"07:00"},"MyGroups":{"Team.csv":["A"]}}"#,
        )
        .unwrap();
        let s = CalendarStore::open_in_memory().unwrap();
        assert_eq!(s.import_desk_reminders(dir.path()).unwrap(), 2);
        let v = s.load().unwrap();
        assert_eq!(v[0].view.reminders, 1);
        assert_eq!(v[1].view.name, "Team");
        assert_eq!(v[1].view.my_groups, ["A"]);
        let st = s.settings().unwrap();
        assert_eq!((st.sound.as_str(), st.volume, st.quiet.on, st.quiet.from), ("Warm bell", 70, true, 22 * 60 + 30));
    }

    #[test]
    fn alerts_round_trip_and_encryption() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("calendar.db");
        {
            let s = CalendarStore::open_encrypted(&path, &[3u8; 32]).unwrap();
            s.set_alert("1|Wed|20:00|Meeting", Some(d(2026, 6, 3)), Some(AlertMode::Mute)).unwrap();
            s.set_alert("1|Wed|20:00|Meeting", None, Some(AlertMode::Shout)).unwrap();
            s.set_alert("1|Wed|20:00|Meeting", None, None).unwrap();
            s.add_row(s.mine_id().unwrap(), &RowInput { days: "Every day".into(), time: "08:00".into(), title: "Secret plan".into(), ..Default::default() }).unwrap();
        }
        let raw = std::fs::read(&path).unwrap();
        assert!(!raw.windows(11).any(|w| w == b"Secret plan"));
        assert!(CalendarStore::open_encrypted(&path, &[4u8; 32]).is_err());
        let s = CalendarStore::open_encrypted(&path, &[3u8; 32]).unwrap();
        let a = s.alerts().unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].mode, AlertMode::Mute);
    }
}

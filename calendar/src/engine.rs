//! Which reminders fall on which day, and what pops up when. Same rules as Desk Reminders:
//!
//! * a one-off time beats a repeating reminder at the same minute (within one schedule);
//! * a `skip` row removes whatever falls at that time;
//! * `skip` + `all day` clears the usual schedule for that day; rows for that exact date still run;
//! * shared schedules are written in their own time zone and converted to local time;
//! * events for other groups show, but only pop up if asked ("notify me too");
//! * the bell mutes an event, that day or every time.

use crate::format::{resolve_zone, Days, Schedule, When};
use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Event,
    Notice,
}

/// One schedule, as the engine needs it.
pub struct Source<'a> {
    pub id: i64,
    pub name: &'a str,
    pub schedule: &'a Schedule,
    pub mine: bool,
    pub enabled: bool,
    pub my_groups: &'a [String],
    pub colour: &'a str,
    pub quiet_allow: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Occurrence {
    /// When it pops up, local time
    pub at: NaiveDateTime,
    /// When the event itself is, local time (differs from `at` for notices)
    pub event_at: NaiveDateTime,
    pub kind: Kind,
    pub source_id: i64,
    pub source_name: String,
    pub mine: bool,
    pub colour: String,
    pub title: String,
    pub text: String,
    pub notice: u32,
    pub for_groups: Vec<String>,
    /// For you: your schedule, everyone, or one of your groups
    pub for_me: bool,
    /// Whether it will pop up, after mutes and "notify me too"
    pub notify: bool,
    pub quiet_allow: bool,
    /// Identifies the reminder for mutes: schedule, days, time, title
    pub key: String,
    /// The file line, to edit or remove it
    pub line: usize,
    pub days_text: String,
    pub time_text: String,
    pub one_off: bool,
    /// The date in the schedule's own time zone
    pub zone_date: NaiveDate,
    /// HH:MM in the schedule's own time zone
    pub zone_time: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertMode {
    /// Don't pop up
    Mute,
    /// Pop up although it's for another group
    Shout,
    /// Don't show at all (shared schedules only)
    Hide,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub key: String,
    /// None = every time
    pub date: Option<NaiveDate>,
    pub mode: AlertMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct Quiet {
    pub on: bool,
    pub from: u16,
    pub to: u16,
}

impl Quiet {
    pub fn covers(&self, t: NaiveDateTime) -> bool {
        if !self.on || self.from == self.to {
            return false;
        }
        let m = (t.time().signed_duration_since(NaiveTime::MIN).num_minutes()) as u16;
        if self.from < self.to {
            m >= self.from && m < self.to
        } else {
            m >= self.from || m < self.to
        }
    }
}

struct Entry<'a> {
    minute: u16,
    row: &'a crate::format::Row,
    one_off: bool,
}

/// One day of one schedule, in that schedule's own time.
fn day_entries(s: &Schedule, date: NaiveDate) -> Vec<Entry<'_>> {
    let mut todays: Vec<&crate::format::Row> = s.rows.iter().filter(|r| r.days.matches(date)).collect();
    if todays.iter().any(|r| r.skip && r.time == When::AllDay) {
        todays.retain(|r| r.days.is_date() && r.time != When::AllDay);
    }
    let skips: Vec<u16> = todays.iter().filter(|r| r.skip).flat_map(|r| r.time.minutes()).collect();
    let mut by_minute: std::collections::BTreeMap<u16, (Vec<Entry>, Vec<Entry>)> = Default::default();
    for r in todays.iter().filter(|r| !r.skip) {
        for m in r.time.minutes() {
            let e = Entry { minute: m, row: r, one_off: matches!(r.days, Days::Date(_)) && !r.time.repeats() };
            let slot = by_minute.entry(m).or_default();
            if r.time.repeats() {
                slot.1.push(e);
            } else {
                slot.0.push(e);
            }
        }
    }
    let mut out = Vec::new();
    for (m, (once, rep)) in by_minute {
        if skips.contains(&m) {
            continue;
        }
        out.extend(if once.is_empty() { rep } else { once });
    }
    out
}

fn hhmm(m: u16) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

pub fn alert_key(source_id: i64, days_text: &str, time_text: &str, title: &str) -> String {
    format!("{source_id}|{days_text}|{time_text}|{title}")
}

/// Every reminder and notice that falls on the local dates `from..=to`, from all enabled schedules,
/// in local time, sorted. `local` is the computer's time zone.
pub fn occurrences<L: TimeZone>(sources: &[Source], from: NaiveDate, to: NaiveDate, alerts: &[Alert], local: &L) -> Vec<Occurrence> {
    let mut out = Vec::new();
    for src in sources.iter().filter(|s| s.enabled) {
        let zone = (!src.schedule.timezone.is_empty()).then(|| resolve_zone(&src.schedule.timezone)).flatten();
        let mut d = from - Duration::days(2);
        while d <= to + Duration::days(1) {
            for e in day_entries(src.schedule, d) {
                let wall = d.and_time(NaiveTime::MIN) + Duration::minutes(e.minute as i64);
                let at = match zone {
                    Some(z) => match z.from_local_datetime(&wall).earliest() {
                        Some(t) => t.with_timezone(local).naive_local(),
                        None => continue, // doesn't exist there (clocks going forward)
                    },
                    None => wall,
                };
                let r = e.row;
                let key = alert_key(src.id, &r.days_text, &r.time_text, &r.title);
                let event_date = at.date();
                let mode_for = |date: NaiveDate| {
                    let day = alerts.iter().find(|a| a.key == key && a.date == Some(date)).map(|a| a.mode);
                    day.or_else(|| alerts.iter().find(|a| a.key == key && a.date.is_none()).map(|a| a.mode))
                };
                let mode = mode_for(event_date);
                if !src.mine && mode == Some(AlertMode::Hide) {
                    continue;
                }
                let for_me = src.mine || r.for_groups.is_empty() || r.for_groups.iter().any(|g| src.my_groups.contains(g));
                let notify = match mode {
                    Some(AlertMode::Mute) => false,
                    Some(AlertMode::Shout) => true,
                    _ => for_me,
                };
                let kinds: &[Kind] = if r.notice > 0 { &[Kind::Event, Kind::Notice] } else { &[Kind::Event] };
                for &kind in kinds {
                    let when = if kind == Kind::Notice { at - Duration::minutes(r.notice as i64) } else { at };
                    if when.date() < from || when.date() > to {
                        continue;
                    }
                    out.push(Occurrence {
                        at: when,
                        event_at: at,
                        kind,
                        source_id: src.id,
                        source_name: src.name.to_string(),
                        mine: src.mine,
                        colour: src.colour.to_string(),
                        title: r.title.clone(),
                        text: r.text.clone(),
                        notice: r.notice,
                        for_groups: r.for_groups.clone(),
                        for_me,
                        notify,
                        quiet_allow: src.quiet_allow,
                        key: key.clone(),
                        line: r.line,
                        days_text: r.days_text.clone(),
                        time_text: r.time_text.clone(),
                        one_off: e.one_off,
                        zone_date: d,
                        zone_time: hhmm(e.minute),
                    });
                }
            }
            d += Duration::days(1);
        }
    }
    out.sort_by(|a, b| a.at.cmp(&b.at).then(b.mine.cmp(&a.mine)).then(a.source_name.cmp(&b.source_name)));
    out
}

/// What pops up in the minutes after `after` up to and including `upto`. Catches up at most a few
/// minutes, so waking a computer from sleep doesn't bring a pile of old reminders.
pub fn due(all: &[Occurrence], after: NaiveDateTime, upto: NaiveDateTime, quiet: &Quiet) -> Vec<Occurrence> {
    let floor = after.max(upto - Duration::minutes(3));
    all.iter()
        .filter(|o| o.at > floor && o.at <= upto && o.notify && (!quiet.covers(o.at) || o.quiet_allow))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::parse;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn src<'a>(id: i64, s: &'a Schedule, mine: bool, groups: &'a [String]) -> Source<'a> {
        Source { id, name: if mine { "My schedule" } else { "Team" }, schedule: s, mine, enabled: true, my_groups: groups, colour: "#1F4E8C", quiet_allow: !mine }
    }

    #[test]
    fn the_desk_reminders_example() {
        let s = parse(
            "Days,Time,Colour,Title,Text,Notice,For\nEvery day,07:30,,Breakfast,Eat.,,\nWeekdays,every 60 min 10:00-17:00,,Stand up,Stretch.,,\nWeekdays,12:00,,Lunch,Away from the screen.,,\n2026-12-25,all day,skip,,,,\n2026-12-25,10:00,,Presents,Open them!,,\nWeekdays,15:00,skip,,,,\n",
        );
        let none: Vec<String> = vec![];
        let sources = [src(1, &s, true, &none)];
        let tue = occurrences(&sources, d(2026, 12, 22), d(2026, 12, 22), &[], &chrono_tz::UTC);
        let titles: Vec<String> = tue.iter().map(|o| format!("{} {}", o.at.format("%H:%M"), o.title)).collect();
        assert_eq!(
            titles,
            ["07:30 Breakfast", "10:00 Stand up", "11:00 Stand up", "12:00 Lunch", "13:00 Stand up", "14:00 Stand up", "16:00 Stand up", "17:00 Stand up"]
        );
        let xmas = occurrences(&sources, d(2026, 12, 25), d(2026, 12, 25), &[], &chrono_tz::UTC);
        assert_eq!(xmas.iter().map(|o| o.title.as_str()).collect::<Vec<_>>(), ["Presents"]);
    }

    #[test]
    fn time_zones_groups_notices_and_mutes() {
        let s = parse(
            "#! timezone = Europe/Amsterdam\n#! groups = Team 1 | Team 2\nDays,Time,Colour,Title,Text,Notice,For\nWed,20:00,,Team meeting,,30,Team 1\nThu,20:00,,Team meeting,,30,Team 2\nSun,19:00,,Meeting,,,\n",
        );
        let mine = vec!["Team 1".to_string()];
        let sources = [src(7, &s, false, &mine)];
        // Week of 2 Dec 2026: Amsterdam is UTC+1 in winter
        let week = occurrences(&sources, d(2026, 11, 30), d(2026, 12, 6), &[], &chrono_tz::Europe::London);
        let wed: Vec<_> = week.iter().filter(|o| o.title == "Team meeting" && o.event_at.date() == d(2026, 12, 2)).collect();
        assert_eq!(wed.len(), 2, "event and its notice");
        assert_eq!(wed[0].at.format("%H:%M").to_string(), "18:30", "notice, London time");
        assert_eq!(wed[1].at.format("%H:%M").to_string(), "19:00");
        assert!(wed.iter().all(|o| o.for_me && o.notify));
        let thu = week.iter().find(|o| o.event_at.date() == d(2026, 12, 3) && o.kind == Kind::Event).unwrap();
        assert!(!thu.for_me && !thu.notify, "another group's event shows but doesn't pop up");

        let alerts = vec![
            Alert { key: thu.key.clone(), date: Some(d(2026, 12, 3)), mode: AlertMode::Shout },
            Alert { key: wed[1].key.clone(), date: None, mode: AlertMode::Mute },
        ];
        let week = occurrences(&sources, d(2026, 11, 30), d(2026, 12, 6), &alerts, &chrono_tz::Europe::London);
        assert!(week.iter().find(|o| o.event_at.date() == d(2026, 12, 3) && o.kind == Kind::Event).unwrap().notify);
        assert!(week.iter().filter(|o| o.title == "Team meeting" && o.event_at.date() == d(2026, 12, 2)).all(|o| !o.notify));
    }

    #[test]
    fn due_and_quiet_hours() {
        let s = parse("Days,Time,Colour,Title,Text,Notice,For\nEvery day,23:30,,Late,,,\nEvery day,09:00,,Morning,,,\n");
        let none: Vec<String> = vec![];
        let sources = [src(1, &s, true, &none)];
        let all = occurrences(&sources, d(2026, 6, 1), d(2026, 6, 1), &[], &chrono_tz::UTC);
        let t = |h, m| d(2026, 6, 1).and_hms_opt(h, m, 0).unwrap();
        let quiet = Quiet { on: true, from: 23 * 60, to: 8 * 60 };
        assert_eq!(due(&all, t(8, 59), t(9, 0), &quiet).len(), 1);
        assert_eq!(due(&all, t(9, 0), t(9, 1), &quiet).len(), 0, "not twice");
        assert_eq!(due(&all, t(23, 29), t(23, 30), &quiet).len(), 0, "quiet hours");
        assert_eq!(due(&all, t(6, 0), t(9, 30), &Quiet::default()).len(), 0, "too late after a long sleep");
    }
}

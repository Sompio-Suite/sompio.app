//! The schedule file format from Desk Reminders, read exactly the same way:
//!
//! ```csv
//! #! name = Guild Schedule
//! #! timezone = Europe/Amsterdam
//! #! groups = Raid Team 1 | Raid Team 2
//! Days,Time,Colour,Title,Text,Notice,For
//! Weekdays,every 60 min 10:00-17:00,,Stand up,Stretch.,,
//! 2026-12-25,all day,skip,,,,
//! ```
//!
//! Sompio adds two kinds of days Desk Reminders doesn't have, for check-ups:
//! `monthly 1` (the 1st of every month) and `quarterly 1` (1 January, April, July, October).

use chrono::{Datelike, NaiveDate};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Days {
    All,
    /// Bit 0 = Monday … bit 6 = Sunday
    Week(u8),
    Date(NaiveDate),
    Range(NaiveDate, NaiveDate),
    Monthly(u32),
    Quarterly(u32),
    /// Every year on this month and day (birthdays, anniversaries)
    Yearly(u32, u32),
}

impl Days {
    pub fn matches(&self, d: NaiveDate) -> bool {
        match *self {
            Days::All => true,
            Days::Week(mask) => mask & (1 << d.weekday().num_days_from_monday()) != 0,
            Days::Date(x) => x == d,
            Days::Range(a, b) => a <= d && d <= b,
            Days::Monthly(n) => d.day() == n.min(last_day(d)),
            Days::Quarterly(n) => matches!(d.month(), 1 | 4 | 7 | 10) && d.day() == n.min(last_day(d)),
            // 29 February falls on the 28th in other years
            Days::Yearly(m, n) => d.month() == m && d.day() == n.min(last_day(d)),
        }
    }
    pub fn is_date(&self) -> bool {
        matches!(self, Days::Date(_))
    }
}

fn last_day(d: NaiveDate) -> u32 {
    let (y, m) = (d.year(), d.month());
    let next = if m == 12 { NaiveDate::from_ymd_opt(y + 1, 1, 1) } else { NaiveDate::from_ymd_opt(y, m + 1, 1) };
    next.and_then(|n| n.pred_opt()).map(|p| p.day()).unwrap_or(28)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum When {
    /// Minutes after midnight
    At(u16),
    Every { step: u16, from: u16, to: u16 },
    AllDay,
}

impl When {
    pub fn minutes(&self) -> Vec<u16> {
        match *self {
            When::At(m) => vec![m],
            When::Every { step, from, to } => (from..=to).step_by(step as usize).collect(),
            When::AllDay => vec![],
        }
    }
    pub fn repeats(&self) -> bool {
        matches!(self, When::Every { .. })
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    /// 1-based line in the file
    pub line: usize,
    pub days: Days,
    pub days_text: String,
    pub time: When,
    pub time_text: String,
    pub skip: bool,
    pub title: String,
    pub text: String,
    /// Minutes before, for a heads-up pop-up
    pub notice: u32,
    /// Groups it's for; empty = everyone
    pub for_groups: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Problem {
    pub line: usize,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct Schedule {
    pub name: String,
    pub timezone: String,
    pub updated: String,
    pub groups: Vec<String>,
    pub rows: Vec<Row>,
    pub problems: Vec<Problem>,
    pub delimiter: u8,
    pub has_header: bool,
}

const DAY_NAMES: [(&str, u8); 14] = [
    ("mon", 0), ("monday", 0), ("tue", 1), ("tuesday", 1), ("wed", 2), ("wednesday", 2), ("thu", 3),
    ("thursday", 3), ("fri", 4), ("friday", 4), ("sat", 5), ("saturday", 5), ("sun", 6), ("sunday", 6),
];

fn day_index(s: &str) -> Option<u8> {
    DAY_NAMES.iter().find(|(n, _)| *n == s).map(|(_, i)| *i)
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

pub fn parse_days(spec: &str) -> Option<Days> {
    let s = squash(spec);
    match s.as_str() {
        "every day" | "everyday" | "daily" | "*" => return Some(Days::All),
        "weekdays" => return Some(Days::Week(0b0011111)),
        "weekends" => return Some(Days::Week(0b1100000)),
        _ => {}
    }
    if let Some(d) = parse_date(&s) {
        return Some(Days::Date(d));
    }
    for sep in [" to ", ".."] {
        if let Some((a, b)) = s.split_once(sep) {
            let (a, b) = (parse_date(a)?, parse_date(b)?);
            return (a <= b).then_some(Days::Range(a, b));
        }
    }
    if let Some(rest) = s.strip_prefix("yearly") {
        let (m, d) = rest.trim().trim_start_matches("on").trim().split_once('-')?;
        let (m, d): (u32, u32) = (m.trim().parse().ok()?, d.trim().parse().ok()?);
        return ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some(Days::Yearly(m, d));
    }
    for (word, quarterly) in [("monthly", false), ("quarterly", true)] {
        if let Some(rest) = s.strip_prefix(word) {
            let digits: String = rest.trim().trim_start_matches("on").trim().chars().take_while(|c| c.is_ascii_digit()).collect();
            let n: u32 = digits.parse().ok()?;
            if (1..=31).contains(&n) {
                return Some(if quarterly { Days::Quarterly(n) } else { Days::Monthly(n) });
            }
            return None;
        }
    }
    let mut mask = 0u8;
    for tok in s.split(',') {
        let t = tok.trim();
        if let Some((a, b)) = t.split_once('-') {
            let (mut i, j) = (day_index(a.trim())?, day_index(b.trim())?);
            loop {
                mask |= 1 << i;
                if i == j {
                    break;
                }
                i = (i + 1) % 7;
            }
        } else {
            mask |= 1 << day_index(t)?;
        }
    }
    (mask != 0).then_some(Days::Week(mask))
}

pub fn parse_clock(s: &str) -> Option<u16> {
    let (h, m) = s.trim().split_once(':')?;
    if h.is_empty() || h.len() > 2 || m.len() != 2 {
        return None;
    }
    let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
    (h <= 23 && m <= 59).then_some(h * 60 + m)
}

pub fn parse_time(spec: &str) -> Option<When> {
    let s = squash(spec);
    if s == "all day" {
        return Some(When::AllDay);
    }
    if let Some(m) = parse_clock(&s) {
        return Some(When::At(m));
    }
    let (step, range) = if let Some(rest) = s.strip_prefix("every ") {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        let after = rest[digits.len()..].trim_start();
        let after = after.strip_prefix("minutes").or_else(|| after.strip_prefix("min"))?;
        (digits.parse::<u16>().ok()?, after.trim().to_string())
    } else if let Some(rest) = s.strip_prefix("hourly ") {
        (60, rest.trim().to_string())
    } else {
        return None;
    };
    let (a, b) = range.split_once('-')?;
    let (from, to) = (parse_clock(a)?, parse_clock(b)?);
    (step >= 5 && from <= to).then_some(When::Every { step, from, to })
}

fn header_index(header: &[String], names: &[&str]) -> Option<usize> {
    header.iter().position(|h| names.iter().any(|n| h.trim().eq_ignore_ascii_case(n)))
}

/// Reads a schedule file. Rows that can't be read are listed in `problems`; the rest keep working.
pub fn parse(text: &str) -> Schedule {
    let mut s = Schedule { delimiter: b',', ..Default::default() };
    let mut header: Option<Vec<String>> = None;
    let text = text.trim_start_matches('\u{feff}');
    for (n, line) in text.lines().enumerate() {
        let line_no = n + 1;
        let trim = line.trim().trim_matches('"').trim();
        if let Some(directive) = trim.strip_prefix("#!") {
            let d = directive.trim_end_matches([',', ';', '"']).trim();
            if let Some(i) = d.find(['=', ':']) {
                let key = d[..i].trim().to_lowercase().replace(' ', "");
                let val = d[i + 1..].trim_end_matches([',', ';', '"']).trim().to_string();
                match key.as_str() {
                    "name" => s.name = val,
                    "timezone" | "tz" => s.timezone = val,
                    "updated" => s.updated = val,
                    "groups" => s.groups = val.split('|').map(|g| g.trim().to_string()).filter(|g| !g.is_empty()).collect(),
                    _ => {}
                }
            }
            continue;
        }
        if trim.is_empty() || trim.starts_with('#') || line.chars().all(|c| matches!(c, ',' | ';' | '"') || c.is_whitespace()) {
            continue;
        }
        let Some(h) = &header else {
            if line.contains(';') && !line.contains(',') {
                s.delimiter = b';';
            }
            header = Some(split(line, s.delimiter));
            continue;
        };
        let fields = split(line, s.delimiter);
        let get = |names: &[&str]| header_index(h, names).and_then(|i| fields.get(i)).map(|v| v.trim().to_string()).unwrap_or_default();
        let days_text = get(&["Days"]);
        let time_text = get(&["Time"]);
        let colour = get(&["Colour", "Color"]).to_lowercase();
        let title = get(&["Title"]);
        let notice_text = get(&["Notice"]);
        let for_text = get(&["For"]);
        let mut problem = |m: String| s.problems.push(Problem { line: line_no, message: m });
        let Some(days) = parse_days(&days_text) else {
            problem(format!("Days \"{days_text}\" not understood. Use Every day, Weekdays, Weekends, Mon-Fri, a date like 2026-12-24, or 2026-12-01 to 2026-12-31."));
            continue;
        };
        let Some(time) = parse_time(&time_text) else {
            problem(format!("Time \"{time_text}\" not understood. Use 24-hour time like 09:30, or every 60 min 10:00-17:00."));
            continue;
        };
        let skip = colour == "skip";
        if time == When::AllDay && !skip {
            problem("\"all day\" only works with Colour skip.".into());
            continue;
        }
        if !skip && title.is_empty() {
            problem("Title is empty.".into());
            continue;
        }
        let notice = if notice_text.is_empty() {
            0
        } else {
            match notice_text.parse::<u32>() {
                Ok(n) if n <= 1440 => n,
                _ => {
                    problem(format!("Notice \"{notice_text}\" must be a number of minutes (0-1440)."));
                    continue;
                }
            }
        };
        let for_groups: Vec<String> = for_text
            .split('|')
            .map(|g| g.trim().to_string())
            .filter(|g| !g.is_empty() && !g.eq_ignore_ascii_case("everyone"))
            .collect();
        for g in &for_groups {
            if !s.groups.contains(g) {
                s.groups.push(g.clone());
            }
        }
        s.rows.push(Row { line: line_no, days, days_text, time, time_text, skip, title, text: get(&["Text"]), notice, for_groups });
    }
    s.has_header = header.is_some();
    if !s.has_header && !text.trim().is_empty() {
        s.problems.push(Problem { line: 0, message: "No header row (Days,Time,Colour,Title,Text,Notice,For).".into() });
    }
    if !s.timezone.is_empty() && resolve_zone(&s.timezone).is_none() {
        s.problems.push(Problem {
            line: 0,
            message: format!("Time zone \"{}\" not recognised, so times are used as your local time.", s.timezone),
        });
    }
    s
}

fn split(line: &str, delim: u8) -> Vec<String> {
    let mut r = csv::ReaderBuilder::new().has_headers(false).delimiter(delim).flexible(true).from_reader(line.as_bytes());
    r.records().next().and_then(|x| x.ok()).map(|rec| rec.iter().map(str::to_string).collect()).unwrap_or_default()
}

/// Time zone names: IANA names, plus the Windows names Desk Reminders users may have written.
pub fn resolve_zone(name: &str) -> Option<chrono_tz::Tz> {
    let n = name.trim();
    if let Ok(tz) = n.parse::<chrono_tz::Tz>() {
        return Some(tz);
    }
    let windows = [
        ("utc", "UTC"),
        ("gmt", "UTC"),
        ("w. europe standard time", "Europe/Amsterdam"),
        ("romance standard time", "Europe/Paris"),
        ("gmt standard time", "Europe/London"),
        ("central european standard time", "Europe/Warsaw"),
        ("central europe standard time", "Europe/Prague"),
        ("gtb standard time", "Europe/Bucharest"),
        ("fle standard time", "Europe/Helsinki"),
        ("turkey standard time", "Europe/Istanbul"),
        ("russian standard time", "Europe/Moscow"),
        ("eastern standard time", "America/New_York"),
        ("central standard time", "America/Chicago"),
        ("mountain standard time", "America/Denver"),
        ("pacific standard time", "America/Los_Angeles"),
        ("e. south america standard time", "America/Sao_Paulo"),
        ("tokyo standard time", "Asia/Tokyo"),
        ("korea standard time", "Asia/Seoul"),
        ("china standard time", "Asia/Shanghai"),
        ("aus eastern standard time", "Australia/Sydney"),
    ];
    let l = n.to_lowercase();
    windows.iter().find(|(w, _)| *w == l).and_then(|(_, iana)| iana.parse().ok())
}

/// One reminder as typed in the app, before it becomes a line in the file.
#[derive(Debug, Clone, Default, serde::Deserialize, Serialize)]
pub struct RowInput {
    pub days: String,
    pub time: String,
    #[serde(default)]
    pub skip: bool,
    pub title: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub notice: u32,
    #[serde(default)]
    pub for_groups: Vec<String>,
}

impl RowInput {
    /// Checks it the same way the file reader would.
    pub fn check(&self) -> Result<(), String> {
        if parse_days(&self.days).is_none() {
            return Err(format!("\"{}\" isn't a day the calendar understands.", self.days));
        }
        match parse_time(&self.time) {
            None => return Err(format!("\"{}\" isn't a time the calendar understands. Use 24-hour time like 09:30.", self.time)),
            Some(When::AllDay) if !self.skip => return Err("\"All day\" is only for clearing a day.".into()),
            _ => {}
        }
        if !self.skip && self.title.trim().is_empty() {
            return Err("Give the reminder a title.".into());
        }
        if self.notice > 1440 {
            return Err("A notice can be at most 1440 minutes (a day) before.".into());
        }
        Ok(())
    }

    pub fn to_line(&self, delim: u8) -> String {
        let notice = if self.notice > 0 { self.notice.to_string() } else { String::new() };
        let colour = if self.skip { "skip" } else { "" };
        let fields = [
            self.days.trim(),
            self.time.trim(),
            colour,
            self.title.trim(),
            self.text.trim(),
            notice.as_str(),
            &self.for_groups.join("|"),
        ];
        let d = delim as char;
        fields
            .iter()
            .map(|f| {
                let f = f.replace(['\r', '\n'], " ");
                if f.contains(d) || f.contains('"') {
                    format!("\"{}\"", f.replace('"', "\"\""))
                } else {
                    f
                }
            })
            .collect::<Vec<_>>()
            .join(&d.to_string())
    }
}

pub const HEADER: &str = "Days,Time,Colour,Title,Text,Notice,For";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_like_desk_reminders() {
        let d = |s| parse_days(s).unwrap();
        assert_eq!(d("Every day"), Days::All);
        assert_eq!(d("weekdays"), Days::Week(0b0011111));
        assert_eq!(d("Fri-Mon"), Days::Week(0b1110001));
        assert_eq!(d("Mon,Wed,Fri"), Days::Week(0b0010101));
        assert_eq!(d("2026-12-24"), Days::Date(NaiveDate::from_ymd_opt(2026, 12, 24).unwrap()));
        assert!(matches!(d("2026-12-01 to 2026-12-31"), Days::Range(..)));
        assert!(parse_days("2026-12-31 to 2026-12-01").is_none());
        assert!(parse_days("someday").is_none());
        assert_eq!(d("monthly 1"), Days::Monthly(1));
        assert_eq!(d("Quarterly on 15"), Days::Quarterly(15));
        assert_eq!(d("Yearly 03-14"), Days::Yearly(3, 14));
        assert!(Days::Yearly(2, 29).matches(NaiveDate::from_ymd_opt(2027, 2, 28).unwrap()));
        // The 31st falls back to the month's last day
        assert!(Days::Monthly(31).matches(NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()));
        assert!(Days::Quarterly(1).matches(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap()));
        assert!(!Days::Quarterly(1).matches(NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()));
    }

    #[test]
    fn times_like_desk_reminders() {
        assert_eq!(parse_time("08:30"), Some(When::At(510)));
        assert_eq!(parse_time("8:05"), Some(When::At(485)));
        assert_eq!(parse_time("every 30 min 09:00-10:00").unwrap().minutes(), vec![540, 570, 600]);
        assert_eq!(parse_time("every 90 minutes 08:00 - 11:00").unwrap().minutes(), vec![480, 570, 660]);
        assert_eq!(parse_time("hourly 08:00-10:00").unwrap().minutes(), vec![480, 540, 600]);
        assert!(parse_time("every 2 min 09:00-10:00").is_none());
        assert!(parse_time("24:00").is_none());
        assert_eq!(parse_time("All day"), Some(When::AllDay));
    }

    #[test]
    fn reads_a_full_file() {
        let s = parse(
            "\u{feff}#! name = Guild Schedule\n#! timezone = Europe/Amsterdam\n#! groups = Raid Team 1 | Raid Team 2\nDays,Time,Colour,Title,Text,Notice,For\n# a note\nWed,20:00,,Raid night,\"Be online, repaired.\",30,Raid Team 1\nThu,20:00,,Raid,x,30,Raid Team 2|Raid Team 3\nSun,25:00,,Bad,,,\nEvery day,09:00,,,,,\n2026-12-25,all day,skip,,,,\n",
        );
        assert_eq!(s.name, "Guild Schedule");
        assert_eq!(s.timezone, "Europe/Amsterdam");
        assert_eq!(s.groups, vec!["Raid Team 1", "Raid Team 2", "Raid Team 3"]);
        assert_eq!(s.rows.len(), 3);
        assert_eq!(s.rows[0].text, "Be online, repaired.");
        assert_eq!(s.rows[0].notice, 30);
        assert_eq!(s.problems.iter().map(|p| p.line).collect::<Vec<_>>(), vec![8, 9]);
    }

    #[test]
    fn semicolons_and_color_header() {
        let s = parse("Days;Time;Color;Title;Text\nEvery day;08:30;;Breakfast;Eat, then go\n");
        assert_eq!(s.delimiter, b';');
        assert_eq!(s.rows[0].text, "Eat, then go");
    }

    #[test]
    fn row_input_round_trip() {
        let r = RowInput { days: "Mon,Wed".into(), time: "09:00".into(), title: "Call \"Sam\"".into(), text: "a, b".into(), notice: 15, ..Default::default() };
        let line = r.to_line(b',');
        let s = parse(&format!("{HEADER}\n{line}\n"));
        assert_eq!(s.rows[0].title, "Call \"Sam\"");
        assert_eq!(s.rows[0].text, "a, b");
        assert_eq!(s.rows[0].days, Days::Week(0b101));
        assert!(RowInput { days: "x".into(), ..r.clone() }.check().is_err());
        assert!(resolve_zone("W. Europe Standard Time").is_some());
    }
}

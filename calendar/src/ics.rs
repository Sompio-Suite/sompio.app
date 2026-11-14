//! Reads a calendar file (.ics) exported from a phone or another calendar app (Google, Apple,
//! Outlook) and turns its events into reminders. Only the file is read; nothing is fetched.
//!
//! Times are turned into this PC's local time. Repeats Sompio understands come along (daily,
//! weekly on set days, monthly on a date, yearly); events in the past and repeats it can't follow
//! (like "every second week") are left out and counted.

use crate::format::RowInput;
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, TimeZone};

#[derive(Debug, Default)]
pub struct IcsImport {
    pub rows: Vec<RowInput>,
    /// Events that already happened
    pub past: usize,
    /// Repeats Sompio can't follow
    pub unsupported: usize,
}

/// Lines can be folded: a line starting with a space or tab continues the one before.
fn unfold(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        if let (Some(rest), Some(last)) = (line.strip_prefix([' ', '\t']), out.last_mut()) {
            last.push_str(rest);
        } else {
            out.push(line.trim_end_matches('\r').to_string());
        }
    }
    out
}

/// "DTSTART;TZID=Europe/Amsterdam:20261118T150000" -> ("DTSTART", [("TZID", "Europe/Amsterdam")], "20261118T150000")
fn split(line: &str) -> Option<(String, Vec<(String, String)>, String)> {
    let colon = line.find(':')?;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let mut parts = head.split(';');
    let name = parts.next()?.to_ascii_uppercase();
    let params = parts.filter_map(|p| p.split_once('=')).map(|(k, v)| (k.to_ascii_uppercase(), v.trim_matches('"').to_string())).collect();
    Some((name, params, value.to_string()))
}

fn unescape(s: &str) -> String {
    s.replace("\\n", " ").replace("\\N", " ").replace("\\,", ",").replace("\\;", ";").replace("\\\\", "\\").trim().to_string()
}

enum Start {
    Day(NaiveDate),
    At(NaiveDateTime),
}

fn parse_start(value: &str, params: &[(String, String)]) -> Option<Start> {
    let v = value.trim();
    if v.len() == 8 {
        return NaiveDate::parse_from_str(v, "%Y%m%d").ok().map(Start::Day);
    }
    let utc = v.ends_with('Z');
    let naive = NaiveDateTime::parse_from_str(v.trim_end_matches('Z'), "%Y%m%dT%H%M%S").ok()?;
    let local = if utc {
        chrono::Utc.from_utc_datetime(&naive).with_timezone(&Local).naive_local()
    } else if let Some(tz) = params.iter().find(|(k, _)| k == "TZID").and_then(|(_, z)| z.parse::<chrono_tz::Tz>().ok()) {
        tz.from_local_datetime(&naive).earliest()?.with_timezone(&Local).naive_local()
    } else {
        naive // "floating" time: the same clock time wherever you are
    };
    Some(Start::At(local))
}

const DAYS: [(&str, &str); 7] = [("MO", "Mon"), ("TU", "Tue"), ("WE", "Wed"), ("TH", "Thu"), ("FR", "Fri"), ("SA", "Sat"), ("SU", "Sun")];

/// The Days column for a repeat rule, or None when Sompio can't follow it.
fn days_for(rule: &str, first: NaiveDate) -> Option<String> {
    let get = |key: &str| rule.split(';').filter_map(|p| p.split_once('=')).find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.to_string());
    if get("INTERVAL").is_some_and(|i| i != "1") || get("BYSETPOS").is_some() {
        return None;
    }
    match get("FREQ")?.to_ascii_uppercase().as_str() {
        "DAILY" => Some("Every day".into()),
        "WEEKLY" => {
            let days = get("BYDAY").unwrap_or_else(|| DAYS[first.weekday().num_days_from_monday() as usize].0.to_string());
            let names: Option<Vec<&str>> = days.split(',').map(|d| DAYS.iter().find(|(k, _)| d.trim().eq_ignore_ascii_case(k)).map(|(_, n)| *n)).collect();
            Some(names?.join(","))
        }
        "MONTHLY" if get("BYDAY").is_none() => {
            let day = get("BYMONTHDAY").and_then(|d| d.parse::<u32>().ok()).unwrap_or(first.day());
            (1..=31).contains(&day).then(|| format!("Monthly {day}"))
        }
        "YEARLY" if get("BYDAY").is_none() => Some(format!("Yearly {:02}-{:02}", first.month(), first.day())),
        _ => None,
    }
}

/// Turns the events in a calendar file into reminders, from `today` on.
pub fn read(text: &str, today: NaiveDate) -> IcsImport {
    let mut out = IcsImport::default();
    let mut ev: Option<Vec<(String, Vec<(String, String)>, String)>> = None;
    for line in unfold(text) {
        let up = line.trim().to_ascii_uppercase();
        if up == "BEGIN:VEVENT" {
            ev = Some(Vec::new());
            continue;
        }
        if up == "END:VEVENT" {
            if let Some(props) = ev.take() {
                event(&props, today, &mut out);
            }
            continue;
        }
        if let (Some(props), Some(p)) = (ev.as_mut(), split(&line)) {
            props.push(p);
        }
    }
    out
}

fn event(props: &[(String, Vec<(String, String)>, String)], today: NaiveDate, out: &mut IcsImport) {
    let get = |name: &str| props.iter().find(|(n, _, _)| n == name);
    if get("STATUS").is_some_and(|(_, _, v)| v.eq_ignore_ascii_case("CANCELLED")) || get("RECURRENCE-ID").is_some() {
        return; // cancelled, or a changed copy of one repeat (the repeat itself is imported)
    }
    let Some((_, params, value)) = get("DTSTART") else { return };
    let Some(start) = parse_start(value, params) else { return };
    let title = get("SUMMARY").map(|(_, _, v)| unescape(v)).filter(|t| !t.is_empty()).unwrap_or_else(|| "Event".into());
    let mut text = get("DESCRIPTION").map(|(_, _, v)| unescape(v)).unwrap_or_default();
    if let Some((_, _, place)) = get("LOCATION") {
        let place = unescape(place);
        if !place.is_empty() {
            text = if text.is_empty() { place } else { format!("{place}. {text}") };
        }
    }
    text = text.chars().take(300).collect();
    let (date, time) = match start {
        Start::Day(d) => (d, "All day".to_string()),
        Start::At(t) => (t.date(), t.format("%H:%M").to_string()),
    };
    let days = match get("RRULE") {
        Some((_, _, rule)) => {
            let until_past = rule.split(';').find_map(|p| p.strip_prefix("UNTIL=")).and_then(|u| NaiveDate::parse_from_str(&u[..u.len().min(8)], "%Y%m%d").ok()).is_some_and(|u| u < today);
            if until_past {
                out.past += 1;
                return;
            }
            match days_for(rule, date) {
                Some(d) => d,
                None => {
                    out.unsupported += 1;
                    return;
                }
            }
        }
        None => {
            if date < today {
                out.past += 1;
                return;
            }
            date.format("%Y-%m-%d").to_string()
        }
    };
    out.rows.push(RowInput { days, time, title, text, ..Default::default() });
}

#[cfg(test)]
mod tests {
    use super::*;

    const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nSUMMARY:Dentist\r\nDTSTART:20261118T150000\r\nLOCATION:Main St\r\n 12\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nSUMMARY:Old thing\r\nDTSTART:20200101T100000\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nSUMMARY:Gym\r\nDTSTART;TZID=Europe/Amsterdam:20260105T070000\r\nRRULE:FREQ=WEEKLY;BYDAY=MO,WE,FR\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nSUMMARY:Mum's birthday\r\nDTSTART;VALUE=DATE:19600314\r\nRRULE:FREQ=YEARLY\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nSUMMARY:Every other week\r\nDTSTART:20260105T070000\r\nRRULE:FREQ=WEEKLY;INTERVAL=2\r\nEND:VEVENT\r\nBEGIN:VEVENT\r\nSUMMARY:Rent\r\nDTSTART:20260101T090000\r\nRRULE:FREQ=MONTHLY\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    #[test]
    fn reads_phone_calendar_events() {
        let r = read(ICS, NaiveDate::from_ymd_opt(2026, 11, 14).unwrap());
        let row = |t: &str| r.rows.iter().find(|x| x.title == t).unwrap_or_else(|| panic!("{t} missing: {:?}", r.rows));
        assert_eq!(row("Dentist").days, "2026-11-18");
        assert_eq!(row("Dentist").time, "15:00");
        assert_eq!(row("Dentist").text, "Main St12");
        assert_eq!(row("Gym").days, "Mon,Wed,Fri");
        assert_eq!(row("Mum's birthday").days, "Yearly 03-14");
        assert_eq!(row("Mum's birthday").time, "All day");
        assert_eq!(row("Rent").days, "Monthly 1");
        assert_eq!((r.past, r.unsupported, r.rows.len()), (1, 1, 4));
        for x in &r.rows {
            assert!(crate::format::parse_days(&x.days).is_some(), "{}", x.days);
            assert!(crate::format::parse_time(&x.time).is_some(), "{}", x.time);
        }
    }
}

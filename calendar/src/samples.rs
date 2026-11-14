//! Samples for testing Calendar: reminders that pop up within minutes, a shared schedule file
//! with groups, and a Desk Reminders folder to bring in. All made up.

use crate::format::RowInput;
use crate::store::CalendarStore;
use anyhow::Result;
use chrono::{Duration, Local, NaiveDateTime};
use std::path::Path;

pub const SAMPLE_NAME: &str = "Sample reminders";
/// The sample team schedule, kept by a manager in New York, so time zones can be seen at work.
pub const TEAM_SAMPLE_NAME: &str = "Northwind team (sample)";

const SHARED: &str = "#! name = Northwind team (sample)
#! timezone = Europe/Amsterdam
#! groups = Design | Engineering | Support
Days,Time,Colour,Title,Text,Notice,For
Mon,10:00,,Weekly planning,Agenda in the team folder. Camera on.,15,
Wed,15:00,,Design review,Bring the latest mockups.,15,Design
\"Tue,Thu\",09:30,,Engineering stand-up,Ten minutes: done / next / blocked.,5,Engineering
Fri,16:00,,Support hand-over,Open tickets for the weekend shift.,15,Support
Monthly 1,11:00,,Expense reports due,Upload receipts before noon.,60,
";

/// A schedule you organise, with reminders a minute or two from `now` so pop-ups, snooze,
/// heads-ups and groups can be tried straight away. Remove the schedule to undo it all.
pub fn add_live_samples(store: &CalendarStore, now: NaiveDateTime) -> Result<i64> {
    // Start on the next whole minute so the first pop-up comes within about a minute
    let base = now + Duration::seconds(60 - now.and_utc().timestamp() % 60);
    let at = |m: i64| (base + Duration::minutes(m)).format("%H:%M").to_string();
    let today = now.format("%Y-%m-%d").to_string();
    let id = store.create_shared(SAMPLE_NAME, "", &["Team A".into(), "Team B".into()])?;
    store.add_rows(
        id,
        &[
            RowInput { days: today.clone(), time: at(1), title: "Your first pop-up".into(), text: "Try Done, or snooze it for 5 minutes.".into(), ..Default::default() },
            RowInput { days: today.clone(), time: at(4), title: "A heads-up before this one".into(), text: "You got a pop-up 2 minutes early.".into(), notice: 2, ..Default::default() },
            RowInput { days: today.clone(), time: at(3), title: "For Team A only".into(), text: "You get this because you're in Team A.".into(), for_groups: vec!["Team A".into()], ..Default::default() },
            RowInput { days: today.clone(), time: at(3), title: "For Team B only".into(), text: "You shouldn't see this pop up (you're not in Team B). It shows in the day list as \"No pop-up\".".into(), for_groups: vec!["Team B".into()], ..Default::default() },
            RowInput { days: "Every day".into(), time: format!("every 5 min {}-{}", at(6), at(26)), title: "Repeats every 5 minutes".into(), text: "Mute it for today from the day list, or skip one time.".into(), ..Default::default() },
            RowInput { days: "Monthly 1".into(), time: "19:00".into(), title: "Monthly reminder".into(), text: "Shows on the 1st of every month.".into(), ..Default::default() },
        ],
    )?;
    store.set_options(id, None, None, None, Some(&["Team A".to_string()]), None)?;

    // A team's shared schedule, kept in New York time: Sompio shows each meeting at your own local time.
    let team = SHARED.replace("Europe/Amsterdam", "America/New_York");
    let tid = store.import(&team, TEAM_SAMPLE_NAME, None, false)?;
    store.set_options(tid, None, None, None, Some(&["Design".to_string()]), None)?;
    Ok(id)
}

/// Files to try "Add a schedule file" and "Bring in Desk Reminders".
pub fn write_sample_files(dir: &Path) -> Result<Vec<(String, &'static str)>> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("Northwind team (sample).csv"), SHARED)?;
    let dr = dir.join("Sample Desk Reminders folder");
    std::fs::create_dir_all(dr.join("shared"))?;
    std::fs::write(
        dr.join("schedule.csv"),
        "Days,Time,Colour,Title,Text,Notice,For\nWeekdays,09:00,,Plan the day,Three things that matter today.,,\nSun,20:00,,Back up your files,Copy your folders to the external drive.,,\nMonthly 1,19:00,,Password check-up,Change one old password.,,\n",
    )?;
    std::fs::write(dr.join("shared").join("Northwind team.csv"), SHARED)?;
    std::fs::write(
        dr.join("settings.json"),
        r#"{"Sound":"Warm bell.wav","SoundOn":true,"Volume":60,"Quiet":{"On":true,"From":"23:00","To":"08:00"},"MyGroups":{"Northwind team.csv":["Design"]}}"#,
    )?;
    let files = vec![
        ("Northwind team (sample).csv".to_string(), "A shared team schedule with groups (Design, Engineering, Support), heads-ups and a time zone. Use \"Add a schedule file\"."),
        ("Sample Desk Reminders folder".to_string(), "A Desk Reminders folder: your schedule, a shared one, quiet hours and sound. Use \"Bring in Desk Reminders\" and choose this folder."),
    ];
    let mut readme = String::from("Sompio Calendar samples\r\n=======================\r\n\r\nMade up, to try every part of Calendar.\r\n\r\n");
    for (f, what) in &files {
        readme.push_str(&format!("{f}\r\n  {what}\r\n\r\n"));
    }
    readme.push_str("For pop-ups right away, use \"Add sample reminders\" in Sompio: the first one pops up within a minute.\r\n");
    std::fs::write(dir.join("READ ME.txt"), readme)?;
    Ok(files)
}

pub fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::due;
    use chrono::NaiveDate;

    #[test]
    fn live_samples_pop_up_soon_and_files_import() {
        let s = CalendarStore::open_in_memory().unwrap();
        let now = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap().and_hms_opt(10, 0, 30).unwrap();
        s.add_live_samples_check(now);
        let occ = s.occurrences_between(now.date(), now.date(), &chrono_tz::UTC).unwrap();
        let mut soon = Vec::new();
        for m in 0..6 {
            soon.extend(due(&occ, now + Duration::minutes(m), now + Duration::minutes(m + 1), &Default::default()));
        }
        let titles: Vec<&str> = soon.iter().map(|o| o.title.as_str()).collect();
        assert!(titles.contains(&"Your first pop-up"));
        assert!(titles.contains(&"For Team A only"));
        assert!(!titles.contains(&"For Team B only"));
        assert!(titles.iter().any(|t| t.contains("heads-up")), "the notice comes 2 minutes early");

        let dir = tempfile::tempdir().unwrap();
        write_sample_files(dir.path()).unwrap();
        let s2 = CalendarStore::open_in_memory().unwrap();
        assert_eq!(s2.import_desk_reminders(&dir.path().join("Sample Desk Reminders folder")).unwrap(), 2);
        let text = std::fs::read_to_string(dir.path().join("Northwind team (sample).csv")).unwrap();
        s2.import(&text, "x", None, false).unwrap();
        assert!(s2.load().unwrap().iter().all(|l| l.view.problems.is_empty()));
    }

    impl CalendarStore {
        fn add_live_samples_check(&self, now: NaiveDateTime) {
            add_live_samples(self, now).unwrap();
        }
    }
}

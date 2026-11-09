//! What's on screen right now: the pop-up queue, snoozes and pauses. Kept in memory only, so
//! reminders keep coming while Sompio is locked, from what was loaded when it was last open.

use crate::engine::{due, Occurrence, Quiet};
use chrono::{Duration, NaiveDateTime};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Popup {
    pub id: u64,
    pub occurrence: Occurrence,
    pub snoozed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Shown {
    pub popups: Vec<Popup>,
    pub paused_until: Option<NaiveDateTime>,
}

#[derive(Default)]
pub struct Reminders {
    /// The coming days, worked out while Sompio was unlocked
    upcoming: Vec<Occurrence>,
    /// Until when `upcoming` is complete
    covers_until: Option<NaiveDateTime>,
    last_tick: Option<NaiveDateTime>,
    queue: Vec<Popup>,
    snoozed: Vec<(NaiveDateTime, Popup)>,
    paused_until: Option<NaiveDateTime>,
    next_id: u64,
    pub quiet: Quiet,
}

impl Reminders {
    pub fn new() -> Self {
        Self::default()
    }

    /// New reminders from the store (after a change, or a fresh unlock).
    pub fn replace(&mut self, upcoming: Vec<Occurrence>, covers_until: NaiveDateTime, quiet: Quiet) {
        self.upcoming = upcoming;
        self.covers_until = Some(covers_until);
        self.quiet = quiet;
    }

    /// True when the loaded reminders run out soon and need refreshing from the store.
    pub fn needs_refresh(&self, now: NaiveDateTime) -> bool {
        self.covers_until.is_none_or(|c| c - now < Duration::hours(24))
    }

    pub fn has_data(&self) -> bool {
        self.covers_until.is_some()
    }

    /// Moves what's due into the queue. Returns true if anything new should show.
    pub fn tick(&mut self, now: NaiveDateTime) -> bool {
        let after = self.last_tick.unwrap_or(now - Duration::seconds(30));
        self.last_tick = Some(now);
        if now < after {
            return false; // clock went back: don't replay
        }
        let mut changed = false;
        let (ready, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut self.snoozed).into_iter().partition(|(at, _)| *at <= now);
        self.snoozed = waiting;
        for (_, p) in ready {
            self.queue.push(p);
            changed = true;
        }
        if self.paused_until.is_some_and(|p| p > now) {
            return changed;
        }
        self.paused_until = None;
        for o in due(&self.upcoming, after, now, &self.quiet) {
            if self.queue.iter().any(|p| p.occurrence.key == o.key && p.occurrence.at == o.at) {
                continue;
            }
            self.next_id += 1;
            self.queue.push(Popup { id: self.next_id, occurrence: o, snoozed: false });
            changed = true;
        }
        changed
    }

    pub fn shown(&self) -> Shown {
        Shown { popups: self.queue.clone(), paused_until: self.paused_until }
    }

    pub fn dismiss(&mut self, id: u64) {
        self.queue.retain(|p| p.id != id);
    }

    pub fn dismiss_all(&mut self) {
        self.queue.clear();
    }

    pub fn snooze(&mut self, id: u64, minutes: i64, now: NaiveDateTime) {
        if let Some(i) = self.queue.iter().position(|p| p.id == id) {
            let mut p = self.queue.remove(i);
            p.snoozed = true;
            self.snoozed.push((now + Duration::minutes(minutes.clamp(1, 24 * 60)), p));
        }
    }

    /// Pause all pop-ups (None resumes). Snoozed ones still come back.
    pub fn pause(&mut self, until: Option<NaiveDateTime>) {
        self.paused_until = until;
    }

    /// Forget everything (e.g. the calendar was wiped).
    pub fn clear(&mut self) {
        *self = Self { next_id: self.next_id, ..Self::default() };
    }

    /// The next few things coming up, for the tray tooltip and the "Next up" card while locked.
    pub fn next(&self, now: NaiveDateTime, n: usize) -> Vec<&Occurrence> {
        self.upcoming.iter().filter(|o| o.at > now && o.notify).take(n).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{occurrences, Source};
    use crate::format::parse;
    use chrono::NaiveDate;

    fn t(day: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 6, day).unwrap().and_hms_opt(h, m, 0).unwrap()
    }

    fn loaded() -> Reminders {
        let s = parse("Days,Time,Colour,Title,Text,Notice,For\nEvery day,10:00,,Water,,,\nEvery day,10:30,,Stretch,,,\n");
        let src = [Source { id: 1, name: "Mine", schedule: &s, mine: true, enabled: true, my_groups: &[], colour: "", quiet_allow: false }];
        let occ = occurrences(&src, t(1, 0, 0).date(), t(3, 0, 0).date(), &[], &chrono_tz::UTC);
        let mut r = Reminders::new();
        r.replace(occ, t(3, 23, 59), Quiet { on: true, from: 23 * 60, to: 7 * 60 });
        r
    }

    #[test]
    fn pops_snoozes_and_pauses() {
        let mut r = loaded();
        assert!(!r.tick(t(1, 9, 59)));
        assert!(r.tick(t(1, 10, 0)));
        assert!(!r.tick(t(1, 10, 0)), "not twice");
        let id = r.shown().popups[0].id;
        r.snooze(id, 5, t(1, 10, 0));
        assert!(r.shown().popups.is_empty());
        r.pause(Some(t(1, 11, 0)));
        assert!(r.tick(t(1, 10, 5)), "snoozed comes back even when paused");
        assert_eq!(r.shown().popups.len(), 1);
        assert!(r.shown().popups[0].snoozed);
        r.dismiss(id);
        assert!(!r.tick(t(1, 10, 30)), "paused: Stretch doesn't show");
        assert!(r.tick(t(2, 10, 0)), "pause over");
        assert_eq!(r.shown().popups[0].occurrence.title, "Water");
        assert!(!r.needs_refresh(t(2, 10, 0)));
        assert!(r.needs_refresh(t(3, 1, 0)));
        assert_eq!(r.next(t(2, 10, 0), 5).len(), 3);
    }
}

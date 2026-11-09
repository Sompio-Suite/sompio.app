// Calendar in the browser preview: a small copy of the Rust engine, enough to click through.
import type { CalSettings, Occurrence, RowInput, ScheduleView } from "./api";

interface Row extends Required<Omit<RowInput, "skip">> { skip: boolean; line: number }
interface Sched { view: Omit<ScheduleView, "reminders" | "problems">; rows: Row[] }

const PALETTE = ["#5B8DEF", "#4FB477", "#9B7BE0", "#E08A4F", "#3FB8B8", "#E06C9F", "#D4B24C", "#8A9BB0", "#E05C5C"];
const DAYS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
let nextLine = 100;
const row = (days: string, time: string, title: string, text = "", notice = 0, for_groups: string[] = []): Row =>
  ({ days, time, title, text, notice, for_groups, skip: false, line: nextLine++ });

const base = (id: number, name: string, mine: boolean, colour: string): Sched["view"] => ({
  id, name, mine, owned: mine, editable: mine, colour, enabled: true, sound: null, groups: [], my_groups: [], quiet_allow: !mine,
  timezone: "", updated: "", source: null,
});

const schedules: Sched[] = [
  {
    view: base(1, "My schedule", true, PALETTE[0]),
    rows: [
      row("Every day", "08:00", "Breakfast", "Eat something before the screen.", 15),
      row("Weekdays", "every 60 min 10:00-17:00", "Stand up", "Stretch for a minute."),
      row("Weekdays", "12:30", "Lunch", "Away from the screen."),
      row("Every day", "every 20 min 09:20-18:00", "Eye break", "Look 6 metres away for 20 seconds."),
      row("Monthly 1", "09:00", "Password check-up", "Change one old password and check for reused ones."),
      row("Every day", "23:00", "Bedtime", "Screens off.", 30),
    ],
  },
  {
    view: { ...base(2, "Night Owls raids", false, PALETTE[2]), groups: ["Tanks", "Healers", "DPS"], my_groups: ["Healers"], updated: "2026-11-02 21:14", timezone: "Europe/Amsterdam", source: "C:\\Users\\you\\Downloads\\Night Owls raids.csv" },
    rows: [
      row("Wed", "20:00", "Raid night", "Molten Core. Be on voice 10 minutes early.", 30),
      row("Sun", "19:30", "Healer briefing", "", 15, ["Healers"]),
      row("Sun", "19:30", "Tank briefing", "", 15, ["Tanks"]),
    ],
  },
];

let settings: CalSettings = {
  quiet: { on: true, from: 23 * 60, to: 8 * 60 }, sound: "Soft chime", sound_on: true, volume: 40,
  corner: "bottom-right", popup_seconds: 25, discreet: false, hide_from_capture: true,
};
const alerts = new Map<string, string>(); // key|date -> mode
let autostart = true;
let paused: string | null = null;

const pad = (n: number) => String(n).padStart(2, "0");
const iso = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const at = (day: string, m: number) => `${day}T${pad(Math.floor(m / 60))}:${pad(m % 60)}:00`;
const clock = (s: string) => {
  const m = /^(\d{1,2}):(\d{2})$/.exec(s.trim());
  return m ? Number(m[1]) * 60 + Number(m[2]) : null;
};

function dayMatches(spec: string, d: Date): boolean {
  const s = spec.trim().toLowerCase();
  const wd = (d.getDay() + 6) % 7;
  if (["every day", "daily", "*"].includes(s)) return true;
  if (s === "weekdays") return wd < 5;
  if (s === "weekends") return wd >= 5;
  if (/^\d{4}-\d{2}-\d{2}$/.test(s)) return s === iso(d);
  const mo = /^(monthly|quarterly)\s*(?:on\s*)?(\d+)/.exec(s);
  if (mo) return d.getDate() === Number(mo[2]) && (mo[1] === "monthly" || d.getMonth() % 3 === 0);
  return s.split(",").some((t) => DAYS.indexOf(t.trim().slice(0, 3)) === wd);
}

function times(spec: string): number[] | null {
  const s = spec.trim().toLowerCase();
  if (s === "all day") return [];
  const c = clock(s);
  if (c != null) return [c];
  const ev = /^every (\d+) ?min(?:utes)? (\d{1,2}:\d{2})-(\d{1,2}:\d{2})$/.exec(s);
  if (!ev) return null;
  const out: number[] = [];
  for (let m = clock(ev[2])!; m <= clock(ev[3])!; m += Number(ev[1])) out.push(m);
  return out;
}

export function check(r: RowInput): string | null {
  if (!r.days.trim() || !(DAYS.some((x) => r.days.toLowerCase().includes(x)) || /every day|daily|weekdays|weekends|monthly|quarterly|^\d{4}-\d{2}-\d{2}$/i.test(r.days.trim())))
    return `"${r.days}" isn't a day the calendar understands.`;
  if (times(r.time) == null) return `"${r.time}" isn't a time the calendar understands. Use 24-hour time like 09:30.`;
  if (!r.skip && !r.title.trim()) return "Give the reminder a title.";
  return null;
}

function occurrences(from: string, to: string): Occurrence[] {
  const out: Occurrence[] = [];
  for (let d = new Date(from + "T12:00:00"); iso(d) <= to; d.setDate(d.getDate() + 1)) {
    const day = iso(d);
    for (const s of schedules) {
      if (!s.view.enabled) continue;
      const skips = s.rows.filter((r) => r.skip && dayMatches(r.days, d));
      for (const r of s.rows) {
        if (r.skip || !dayMatches(r.days, d)) continue;
        for (const m of times(r.time) ?? []) {
          if (skips.some((k) => k.time.toLowerCase() === "all day" || clock(k.time) === m)) continue;
          const key = `${s.view.id}|${r.days}|${r.time}|${r.title}`;
          const mode = alerts.get(`${key}|${day}`) ?? alerts.get(`${key}|`);
          if (mode === "hide") continue;
          const forMe = r.for_groups.length === 0 || r.for_groups.some((g) => s.view.my_groups.includes(g));
          const notify = mode === "mute" ? false : mode === "shout" ? true : forMe;
          const o: Occurrence = {
            at: at(day, m), event_at: at(day, m), kind: "event", source_id: s.view.id, source_name: s.view.name, mine: s.view.mine,
            colour: s.view.colour, title: r.title, text: r.text, notice: r.notice, for_groups: r.for_groups, for_me: forMe, notify,
            quiet_allow: s.view.quiet_allow, key, line: r.line, days_text: r.days, time_text: r.time, one_off: /^\d{4}/.test(r.days),
            zone_date: day, zone_time: at(day, m).slice(11, 16),
          };
          out.push(o);
          if (r.notice > 0 && m - r.notice >= 0) out.push({ ...o, kind: "notice", at: at(day, m - r.notice) });
        }
      }
    }
  }
  return out.sort((a, b) => a.at.localeCompare(b.at));
}

const views = (): ScheduleView[] =>
  schedules.map((s) => ({ ...s.view, reminders: s.rows.filter((r) => !r.skip).length, problems: [] }));
const find = (id: number) => {
  const s = schedules.find((x) => x.view.id === id);
  if (!s) throw "That schedule no longer exists.";
  return s;
};
const editable = (id: number) => {
  const s = find(id);
  if (!s.view.editable) throw "This is someone else's shared schedule, so it can't be changed here. You can mute or hide its events.";
  return s;
};
const toRow = (r: RowInput): Row => {
  const p = check(r);
  if (p) throw p;
  return { days: r.days, time: r.time, title: r.title, text: r.text ?? "", notice: r.notice ?? 0, for_groups: r.for_groups ?? [], skip: !!r.skip, line: nextLine++ };
};

// The pop-up preview: one reminder showing, so the #popup view has something to draw.
const popups = [
  { id: 1, snoozed: false, occurrence: occurrences(iso(new Date()), iso(new Date())).find((o) => o.title === "Stand up") ?? occurrences(iso(new Date()), iso(new Date()))[0] },
];

export const calendarHandlers: Record<string, (a: any) => unknown> = {
  cal_overview: ({ from, to }) => ({ schedules: views(), occurrences: occurrences(from, to), settings }),
  cal_next: ({ count }) => {
    const now = new Date();
    const end = new Date(now.getTime() + 7 * 86_400_000);
    const nowS = `${iso(now)}T${pad(now.getHours())}:${pad(now.getMinutes())}:00`;
    return occurrences(iso(now), iso(end)).filter((o) => o.at > nowS && o.notify).slice(0, count);
  },
  cal_add_rows: ({ scheduleId, rows }) => { editable(scheduleId).rows.push(...(rows as RowInput[]).map(toRow)); },
  cal_update_row: ({ scheduleId, line, row: r }) => {
    const s = editable(scheduleId);
    const i = s.rows.findIndex((x) => x.line === line);
    if (i < 0) throw "That reminder moved. Refresh and try again.";
    s.rows[i] = toRow(r);
  },
  cal_remove_row: ({ scheduleId, line }) => { const s = editable(scheduleId); s.rows = s.rows.filter((x) => x.line !== line); },
  cal_skip_once: ({ scheduleId, day, time }) => { editable(scheduleId).rows.push({ ...row(day, time, ""), skip: true }); },
  cal_import_file: () => {
    const id = Math.max(...schedules.map((s) => s.view.id)) + 1;
    schedules.push({ view: { ...base(id, "Guild Schedule", false, PALETTE[schedules.length % 9]), source: "C:\\Users\\you\\Downloads\\Guild Schedule.csv" }, rows: [row("Fri", "21:00", "Mythic+ night", "Bring flasks.", 15)] });
    return id;
  },
  cal_import_desk_reminders: () => { find(1).rows.push(row("Every day", "every 60 min 09:00-21:00", "Drink water", "Have a glass of water (250 ml).")); return 1; },
  cal_export: () => undefined,
  cal_create_shared: ({ name, timezone, groups }) => {
    if (!name.trim()) throw "Give the schedule a name.";
    const id = Math.max(...schedules.map((s) => s.view.id)) + 1;
    schedules.push({ view: { ...base(id, name.trim(), false, PALETTE[schedules.length % 9]), owned: true, editable: true, timezone, groups, my_groups: groups }, rows: [] });
    return id;
  },
  cal_set_options: ({ scheduleId, colour, enabled, sound, myGroups, quietAllow }) => {
    const v = find(scheduleId).view;
    if (colour != null) v.colour = colour;
    if (enabled != null) v.enabled = enabled;
    if (sound != null) v.sound = sound || null;
    if (myGroups != null) v.my_groups = myGroups;
    if (quietAllow != null) v.quiet_allow = quietAllow;
  },
  cal_rename: ({ scheduleId, name }) => { find(scheduleId).view.name = name || (find(scheduleId).view.mine ? "My schedule" : "Shared schedule"); },
  cal_remove_schedule: ({ scheduleId }) => {
    if (find(scheduleId).view.mine) throw "Your own schedule can't be removed. You can remove its reminders instead.";
    schedules.splice(schedules.findIndex((s) => s.view.id === scheduleId), 1);
  },
  cal_set_alert: ({ key, day, mode }) => { const k = `${key}|${day ?? ""}`; if (mode) alerts.set(k, mode); else alerts.delete(k); },
  cal_set_settings: ({ settings: s }) => { settings = s; },
  cal_popups: () => ({ shown: { popups, paused_until: paused }, sound: settings.sound, sound_on: settings.sound_on, volume: settings.volume, seconds: settings.popup_seconds, discreet: settings.discreet, sounds: Object.fromEntries(schedules.filter((x) => x.view.sound).map((x) => [String(x.view.id), x.view.sound])) }),
  cal_popup_action: ({ id, action }) => {
    const i = popups.findIndex((p) => p.id === id);
    if (action === "close-all") popups.length = 0;
    else if (i >= 0) popups.splice(i, 1);
  },
  cal_pause: ({ minutes }) => {
    if (minutes == null) return (paused = null);
    const d = new Date(Date.now() + minutes * 60_000);
    return (paused = `${iso(d)}T${pad(d.getHours())}:${pad(d.getMinutes())}:00`);
  },
  cal_paused: () => paused,
  cal_autostart: () => autostart,
  cal_set_autostart: ({ on }) => { autostart = on; },
};

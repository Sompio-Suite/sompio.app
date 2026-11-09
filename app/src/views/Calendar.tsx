import { useCallback, useEffect, useMemo, useState } from "react";
import {
  api, listen, pickFolder, pickSavePath, pickScheduleFile, SOUNDS,
  type AlertModeKey, type CalOverview, type CalSettings, type Occurrence, type ScheduleView,
} from "../api";
import Icon from "../Icon";
import { plural } from "../format";
import { PACKS } from "../calendarTemplates";
import ReminderDialog from "./ReminderDialog";

const PALETTE = ["#5B8DEF", "#4FB477", "#9B7BE0", "#E08A4F", "#3FB8B8", "#E06C9F", "#D4B24C", "#8A9BB0", "#E05C5C"];
const pad = (n: number) => String(n).padStart(2, "0");
const iso = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const parse = (day: string) => new Date(`${day}T12:00:00`);
const addDays = (day: string, n: number) => { const d = parse(day); d.setDate(d.getDate() + n); return iso(d); };
const hm = (at: string) => at.slice(11, 16);
const minutes = (m: number) => `${pad(Math.floor(m / 60))}:${pad(m % 60)}`;
const clock = (s: string) => { const [h, m] = s.split(":").map(Number); return h * 60 + m; };
const longDay = new Intl.DateTimeFormat(undefined, { weekday: "long", day: "numeric", month: "long" });
const monthName = new Intl.DateTimeFormat(undefined, { month: "long", year: "numeric" });
const shortDay = new Intl.DateTimeFormat(undefined, { weekday: "short", day: "numeric", month: "short" });
const ROUTINE = /^(every ?day|daily|\*|weekdays|weekends)$/i;

/** "Every hour, 10:00–17:00" for the meta line */
function repeatText(o: Occurrence) {
  const ev = /^every (\d+) ?min/i.exec(o.time_text);
  const time = ev ? `every ${Number(ev[1]) === 60 ? "hour" : `${ev[1]} min`}` : "";
  const days = o.one_off ? "" : o.days_text;
  return [days, time].filter(Boolean).join(", ");
}

type Tab = "day" | "schedules" | "settings";

interface Props { readOnly: boolean; onAddKey: () => void }

export default function Calendar({ readOnly, onAddKey }: Props) {
  const today = iso(new Date());
  const [tab, setTab] = useState<Tab>("day");
  const [day, setDay] = useState(today);
  const [monthOf, setMonthOf] = useState(today.slice(0, 7));
  const [data, setData] = useState<CalOverview | null>(null);
  const [next, setNext] = useState<Occurrence[]>([]);
  const [paused, setPaused] = useState<string | null>(null);
  const [editing, setEditing] = useState<Occurrence | null | "new">(null);
  const [packs, setPacks] = useState(false);
  const [toast, setToast] = useState<{ ok: boolean; text: string } | null>(null);
  const [now, setNow] = useState(() => new Date());
  const [showEarlier, setShowEarlier] = useState(false);

  // The month grid shows whole weeks, so load from its first Monday to its last Sunday.
  const range = useMemo(() => {
    const first = parse(`${monthOf}-01`);
    const start = new Date(first);
    start.setDate(1 - ((first.getDay() + 6) % 7));
    const last = new Date(first.getFullYear(), first.getMonth() + 1, 0, 12);
    const end = new Date(last);
    end.setDate(last.getDate() + (6 - ((last.getDay() + 6) % 7)));
    let from = iso(start), to = iso(end);
    if (day < from) from = day;
    if (day > to) to = day;
    return { from, to, start };
  }, [monthOf, day]);

  const load = useCallback(async () => {
    try {
      const [o, n, p] = await Promise.all([api.calOverview(range.from, range.to), api.calNext(5), api.calPaused()]);
      setData(o);
      setNext(n);
      setPaused(p);
    } catch (e) {
      setToast({ ok: false, text: String(e) });
    }
  }, [range.from, range.to]);

  useEffect(() => { load(); }, [load]);
  useEffect(() => {
    const un = listen("calendar-changed", () => load());
    const t = window.setInterval(() => setNow(new Date()), 30_000);
    return () => { un.then((f) => f()); window.clearInterval(t); };
  }, [load]);

  const run = async (f: () => Promise<unknown>, ok?: string) => {
    try {
      await f();
      if (ok) setToast({ ok: true, text: ok });
      await load();
    } catch (e) {
      setToast({ ok: false, text: String(e) });
    }
  };
  useEffect(() => {
    if (!toast) return;
    const t = window.setTimeout(() => setToast(null), 5000);
    return () => window.clearTimeout(t);
  }, [toast]);

  if (!data) return <div className="page"><h1>Calendar</h1></div>;

  const events = data.occurrences.filter((o) => o.kind === "event");
  const ofDay = events.filter((o) => o.at.startsWith(day));
  const nowText = `${iso(now)}T${pad(now.getHours())}:${pad(now.getMinutes())}:00`;
  const done = ofDay.filter((o) => o.at <= nowText).length;
  const mine = data.schedules.find((s) => s.mine)!;
  const empty = data.schedules.every((s) => s.reminders === 0);

  const setAlert = (o: Occurrence, mode: AlertModeKey | null, once: boolean) =>
    run(() => api.calSetAlert(o.key, once ? o.zone_date : null, mode),
      mode === null ? "Back to normal." : mode === "mute" ? (once ? "Muted for this day." : "Muted. It still shows here, without a pop-up.") : mode === "hide" ? "Hidden." : "You'll get a pop-up for this too.");

  return (
    <div className="page cal-page">
      <div className="page-head">
        <div>
          <h1>Calendar</h1>
          <p className="muted">Your reminders, encrypted with your lock. They pop up on time, even while Sompio is locked.</p>
        </div>
        <div className="toolbar">
          <button onClick={() => setPacks(true)} disabled={readOnly}><Icon name="sparkle" size={16} />Routines</button>
          <button className="primary" onClick={() => setEditing("new")} disabled={readOnly}><Icon name="plus" size={16} />New reminder</button>
        </div>
      </div>

      {readOnly && (
        <div className="notice warn">
          <Icon name="lock" />
          <div><strong>Calendar is read-only.</strong><span>Your reminders still pop up. Add a key to change them.</span></div>
          <button onClick={onAddKey}>Add a key</button>
        </div>
      )}
      {paused && (
        <div className="notice">
          <Icon name="clock" />
          <div><strong>Pop-ups are paused until {hm(paused)}.</strong><span>Reminders you snoozed still come back.</span></div>
          <button onClick={() => run(() => api.calPause(null), "Pop-ups are back on.")}>Resume now</button>
        </div>
      )}

      <div className="segmented" role="tablist" aria-label="Calendar">
        {([["day", "Day"], ["schedules", "Schedules"], ["settings", "Pop-ups and sound"]] as [Tab, string][]).map(([k, label]) => (
          <button key={k} role="tab" aria-selected={tab === k} onClick={() => setTab(k)}>{label}</button>
        ))}
      </div>

      {tab === "day" && (
        <div className="cal-layout">
          <section className="panel day-panel" aria-label="Day">
            <header className="day-head">
              <div>
                <h2>{day === today ? "Today" : day === addDays(today, 1) ? "Tomorrow" : longDay.format(parse(day))}</h2>
                <span className="muted small">
                  {day === today ? `${longDay.format(parse(day))} · ${ofDay.length ? `${done} of ${ofDay.length} done` : "nothing planned"}` : plural(ofDay.length, "reminder", "reminders")}
                </span>
              </div>
              <div className="toolbar">
                <button className="quiet" aria-label="Previous day" onClick={() => setDay(addDays(day, -1))}>‹</button>
                {day !== today && <button className="quiet" onClick={() => { setDay(today); setMonthOf(today.slice(0, 7)); }}>Today</button>}
                <button className="quiet" aria-label="Next day" onClick={() => setDay(addDays(day, 1))}>›</button>
              </div>
            </header>
            {day === today && ofDay.length > 0 && (
              <div className="day-progress" role="progressbar" aria-valuemin={0} aria-valuemax={ofDay.length} aria-valuenow={done}>
                <span style={{ width: `${(done / ofDay.length) * 100}%` }} />
              </div>
            )}
            {empty ? (
              <div className="day-empty">
                <strong>No reminders yet.</strong>
                <span className="muted">Add one, start from a routine, or bring in your Desk Reminders schedule.</span>
                <div className="toolbar">
                  <button className="primary" disabled={readOnly} onClick={() => setEditing("new")}>New reminder</button>
                  <button disabled={readOnly} onClick={() => setPacks(true)}>Pick a routine</button>
                  <button disabled={readOnly} onClick={() => setTab("schedules")}>Bring in Desk Reminders</button>
                </div>
              </div>
            ) : ofDay.length === 0 ? (
              <div className="day-empty"><span className="muted">Nothing on this day.</span></div>
            ) : (
              <ol className="day-list">
                {day === today && done > 2 && (
                  <li><button className="show-earlier" onClick={() => setShowEarlier(!showEarlier)}>{showEarlier ? "Hide earlier" : `Show ${done} earlier`}</button></li>
                )}
                {ofDay.map((o, i) => {
                  if (day === today && done > 2 && !showEarlier && o.at <= nowText) return null;
                  const past = o.at <= nowText;
                  const isNext = !past && (i === 0 || ofDay[i - 1].at <= nowText) && day === today;
                  const sched = data.schedules.find((s) => s.id === o.source_id);
                  return (
                    <li key={`${o.key}-${o.at}`} className={`cal-item${past ? " past" : ""}${isNext ? " next" : ""}${o.notify ? "" : " silent"}`} style={{ ["--c" as string]: o.colour }}>
                      <span className="cal-time num">{hm(o.at)}</span>
                      <div className="cal-body">
                        <strong>{o.title}</strong>
                        {o.text && <span className="muted">{o.text}</span>}
                        <span className="cal-meta">
                          {!o.mine && <span>{o.source_name}</span>}
                          {repeatText(o) && <span>{repeatText(o)}</span>}
                          {o.notice > 0 && <span>Heads-up {o.notice} min before</span>}
                          {o.for_groups.length > 0 && <span>For {o.for_groups.join(", ")}</span>}
                          {!o.notify && <span className="meta-flag"><Icon name="eye" size={13} />No pop-up</span>}
                        </span>
                      </div>
                      <div className="cal-actions">
                        {sched?.editable && !readOnly && <button className="small-btn ghost" onClick={() => setEditing(o)}>Edit</button>}
                        {sched?.editable && !readOnly && !o.one_off && (
                          <button className="small-btn ghost" onClick={() => run(() => api.calSkipOnce(o.source_id, o.zone_date, o.zone_time), `Skipped ${o.title} at ${hm(o.at)}.`)}>Skip this time</button>
                        )}
                        <AlertMenu o={o} onPick={setAlert} />
                        {sched?.editable && !readOnly && (
                          <button className="small-btn ghost danger" onClick={() => run(() => api.calRemoveRow(o.source_id, o.line), o.one_off ? "Removed." : `Removed ${o.title} from every day it was on.`)}>Remove</button>
                        )}
                      </div>
                    </li>
                  );
                })}
              </ol>
            )}
          </section>

          <aside className="cal-side">
            <MonthGrid
              month={monthOf}
              start={range.start}
              selected={day}
              today={today}
              events={events}
              onPick={(d) => { setDay(d); }}
              onMonth={(m) => setMonthOf(m)}
            />
            <section className="panel next-up" aria-label="Next up">
              <h3>Next up</h3>
              {next.length === 0 ? (
                <span className="muted small">Nothing in the coming week.</span>
              ) : (
                <ul>
                  {next.map((o) => (
                    <li key={`${o.key}-${o.at}`} style={{ ["--c" as string]: o.colour }}>
                      <span className="num">{o.at.startsWith(today) ? hm(o.at) : `${shortDay.format(parse(o.at.slice(0, 10)))} ${hm(o.at)}`}</span>
                      <span>{o.title}</span>
                    </li>
                  ))}
                </ul>
              )}
              <div className="toolbar">
                {!paused ? (
                  <button className="small-btn" onClick={() => run(() => api.calPause(60), "Pop-ups paused for an hour.")}>Pause pop-ups for 1 hour</button>
                ) : null}
              </div>
            </section>
          </aside>
        </div>
      )}

      {tab === "schedules" && <Schedules data={data} readOnly={readOnly} run={run} mineId={mine.id} />}
      {tab === "settings" && <PopupSettings settings={data.settings} run={run} />}

      {editing && (
        <ReminderDialog
          schedules={data.schedules}
          editing={editing === "new" ? null : editing}
          day={day}
          onClose={() => setEditing(null)}
          onSaved={() => { setEditing(null); setToast({ ok: true, text: "Saved." }); load(); }}
        />
      )}
      {packs && <PacksDialog mineId={mine.id} onClose={() => setPacks(false)} run={run} />}
      {toast && <div className={`toast${toast.ok ? "" : " bad"}`} role="status">{toast.text}</div>}
    </div>
  );
}

function AlertMenu({ o, onPick }: { o: Occurrence; onPick: (o: Occurrence, m: AlertModeKey | null, once: boolean) => void }) {
  const [open, setOpen] = useState(false);
  const pick = (m: AlertModeKey | null, once: boolean) => { setOpen(false); onPick(o, m, once); };
  return (
    <div className="menu-wrap">
      <button className="small-btn ghost" aria-expanded={open} aria-haspopup="menu" onClick={() => setOpen(!open)}>{o.notify ? "Mute" : "Pop-up"} ▾</button>
      {open && (
        <div className="menu" role="menu" onMouseLeave={() => setOpen(false)}>
          {o.notify ? (
            <>
              <button role="menuitem" onClick={() => pick("mute", true)}>No pop-up on this day</button>
              <button role="menuitem" onClick={() => pick("mute", false)}>No pop-up, ever</button>
            </>
          ) : (
            <>
              {!o.for_me && <button role="menuitem" onClick={() => pick("shout", false)}>Pop up for me too</button>}
              <button role="menuitem" onClick={() => pick(null, true)}>Back to normal on this day</button>
              <button role="menuitem" onClick={() => pick(null, false)}>Back to normal</button>
            </>
          )}
          {!o.mine && <button role="menuitem" onClick={() => pick("hide", false)}>Hide it from my calendar</button>}
        </div>
      )}
    </div>
  );
}

interface MonthProps {
  month: string; start: Date; selected: string; today: string; events: Occurrence[];
  onPick: (day: string) => void; onMonth: (m: string) => void;
}

function MonthGrid({ month, start, selected, today, events, onPick, onMonth }: MonthProps) {
  const first = parse(`${month}-01`);
  const shift = (n: number) => { const d = new Date(first); d.setMonth(d.getMonth() + n); onMonth(iso(d).slice(0, 7)); };
  const cells = Array.from({ length: 42 }, (_, i) => { const d = new Date(start); d.setDate(start.getDate() + i); return iso(d); });
  const weeks = cells[35].slice(0, 7) === month ? 6 : 5;
  // Dots for what's special on a day: not the daily routine, but raids, check-ups, one-offs.
  const special = new Map<string, string[]>();
  for (const o of events) {
    if (ROUTINE.test(o.days_text.trim())) continue;
    const d = o.at.slice(0, 10);
    const c = special.get(d) ?? [];
    if (!c.includes(o.colour) && c.length < 3) c.push(o.colour);
    special.set(d, c);
  }
  return (
    <section className="panel month" aria-label="Month">
      <header>
        <strong>{monthName.format(first)}</strong>
        <div>
          <button className="quiet small-btn" aria-label="Previous month" onClick={() => shift(-1)}>‹</button>
          <button className="quiet small-btn" aria-label="Next month" onClick={() => shift(1)}>›</button>
        </div>
      </header>
      <div className="month-grid" role="grid">
        {["M", "T", "W", "T", "F", "S", "S"].map((d, i) => <span key={i} className="dow" aria-hidden>{d}</span>)}
        {cells.slice(0, weeks * 7).map((d) => (
          <button
            key={d}
            className={`${d.slice(0, 7) === month ? "" : "out"}${d === today ? " today" : ""}`}
            aria-current={d === selected ? "date" : undefined}
            aria-label={longDay.format(parse(d))}
            onClick={() => onPick(d)}
          >
            <span>{Number(d.slice(8))}</span>
            <i>{(special.get(d) ?? []).map((c) => <b key={c} style={{ background: c }} />)}</i>
          </button>
        ))}
      </div>
    </section>
  );
}

type Run = (f: () => Promise<unknown>, ok?: string) => Promise<void>;

function Schedules({ data, readOnly, run, mineId }: { data: CalOverview; readOnly: boolean; run: Run; mineId: number }) {
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [groups, setGroups] = useState("");
  const zone = Intl.DateTimeFormat().resolvedOptions().timeZone;

  const importFile = async () => {
    const p = await pickScheduleFile();
    if (p) run(() => api.calImportFile(p), "Schedule added. Pick your groups below if it has any.");
  };
  const importDr = async () => {
    const p = await pickFolder("Choose the folder Desk Reminders runs from");
    if (p) run(async () => {
      const n = await api.calImportDeskReminders(p);
      return n;
    }, "Desk Reminders brought in: your schedule, shared schedules, quiet hours and sound.");
  };
  const exportOne = async (s: ScheduleView) => {
    const p = await pickSavePath(`${s.name}.csv`);
    if (p) run(() => api.calExport(s.id, p), "Saved. Anyone with Sompio or Desk Reminders can open it. The file itself isn't encrypted.");
  };

  return (
    <div className="schedules">
      <div className="toolbar">
        <button onClick={importFile} disabled={readOnly}><Icon name="folder" size={16} />Add a schedule file</button>
        <button onClick={importDr} disabled={readOnly}><Icon name="arrow" size={16} />Bring in Desk Reminders</button>
        <button onClick={() => setCreating(!creating)} disabled={readOnly}><Icon name="plus" size={16} />New shared schedule</button>
      </div>
      {creating && (
        <form className="panel create-shared" onSubmit={(e) => {
          e.preventDefault();
          const g = groups.split(/[,|]/).map((x) => x.trim()).filter(Boolean);
          run(() => api.calCreateShared(name, zone, g), "Created. Add reminders to it, then save it as a file to share.").then(() => { setCreating(false); setName(""); setGroups(""); });
        }}>
          <p className="small muted">A schedule for a group, like a guild's raid nights. You edit it here, then share the file. Times are in your time zone ({zone}) and show up in each person's own time.</p>
          <div className="field-row">
            <label className="field grow"><span>Name</span><input type="text" value={name} onChange={(e) => setName(e.target.value)} placeholder="Night Owls raids" /></label>
            <label className="field grow"><span>Groups <span className="faint">(optional, comma separated)</span></span><input type="text" value={groups} onChange={(e) => setGroups(e.target.value)} placeholder="Tanks, Healers, DPS" /></label>
          </div>
          <div className="toolbar"><button className="primary" disabled={!name.trim()}>Create</button></div>
        </form>
      )}
      {data.schedules.map((s) => (
        <ScheduleCard key={s.id} s={s} run={run} onExport={() => exportOne(s)} isMine={s.id === mineId} />
      ))}
    </div>
  );
}

function ScheduleCard({ s, run, onExport, isMine }: { s: ScheduleView; run: Run; onExport: () => void; isMine: boolean }) {
  const [renaming, setRenaming] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [colours, setColours] = useState(false);
  return (
    <section className={`panel sched${s.enabled ? "" : " off"}`} style={{ ["--c" as string]: s.colour }}>
      <div className="sched-top">
        <button className="swatch-btn" aria-label={`Colour for ${s.name}`} aria-expanded={colours} onClick={() => setColours(!colours)} />
        <div className="sched-name">
          {renaming == null ? (
            <h3>{s.name}</h3>
          ) : (
            <form onSubmit={(e) => { e.preventDefault(); run(() => api.calRename(s.id, renaming)).then(() => setRenaming(null)); }}>
              <input type="text" autoFocus value={renaming} onChange={(e) => setRenaming(e.target.value)} onBlur={() => setRenaming(null)} />
            </form>
          )}
          <span className="small muted">
            {plural(s.reminders, "reminder", "reminders")}
            {s.mine ? " · only on this PC" : s.owned ? " · you organise this one" : " · shared with you"}
            {s.updated && ` · updated ${s.updated}`}
            {s.timezone && ` · times in ${s.timezone}`}
          </span>
        </div>
        <label className="switch">
          <input type="checkbox" checked={s.enabled} onChange={(e) => run(() => api.calSetOptions(s.id, { enabled: e.target.checked }))} />
          {s.enabled ? "On" : "Off"}
        </label>
      </div>
      {colours && (
        <div className="swatch-pop" role="radiogroup" aria-label={`Colour for ${s.name}`}>
          {PALETTE.map((c) => (
            <button key={c} role="radio" aria-checked={s.colour === c} aria-label={c} style={{ background: c }} onClick={() => { setColours(false); run(() => api.calSetOptions(s.id, { colour: c })); }} />
          ))}
        </div>
      )}
      {s.groups.length > 0 && (
        <div className="field">
          <span className="small"><strong>Your groups</strong> <span className="faint">You get pop-ups for events for everyone, and for these.</span></span>
          <div className="day-picks">
            {s.groups.map((g) => (
              <button key={g} aria-pressed={s.my_groups.includes(g)} onClick={() => run(() => api.calSetOptions(s.id, { myGroups: s.my_groups.includes(g) ? s.my_groups.filter((x) => x !== g) : [...s.my_groups, g] }))}>{g}</button>
            ))}
          </div>
        </div>
      )}
      <div className="sched-opts">
        <label className="field">
          <span className="small">Sound</span>
          <select value={s.sound ?? ""} onChange={(e) => run(() => api.calSetOptions(s.id, { sound: e.target.value }))}>
            <option value="">Same as the rest</option>
            {SOUNDS.map((x) => <option key={x}>{x}</option>)}
          </select>
        </label>
        <label className="check">
          <input type="checkbox" checked={s.quiet_allow} onChange={(e) => run(() => api.calSetOptions(s.id, { quietAllow: e.target.checked }))} />
          <span>Pop up during quiet hours too</span>
        </label>
      </div>
      {s.problems.length > 0 && (
        <details className="problems">
          <summary className="warn-text small">{plural(s.problems.length, "line", "lines")} in this schedule can't be read, so they're left out</summary>
          <ul className="small">{s.problems.map((p) => <li key={p.line}>Line {p.line}: {p.message}</li>)}</ul>
        </details>
      )}
      <div className="toolbar">
        {(s.mine || s.owned) && <button className="small-btn" onClick={() => setRenaming(s.name)}>Rename</button>}
        <button className="small-btn" onClick={onExport}>{s.owned && !s.mine ? "Save file to share" : "Save as file"}</button>
        {!isMine && !confirm && <button className="small-btn danger" onClick={() => setConfirm(true)}>Remove</button>}
        {confirm && (
          <>
            <span className="small">Remove {s.name} and its reminders from Sompio?</span>
            <button className="small-btn danger" onClick={() => run(() => api.calRemoveSchedule(s.id), "Removed.")}>Remove</button>
            <button className="small-btn ghost" onClick={() => setConfirm(false)}>Keep</button>
          </>
        )}
        {s.source && <span className="small faint grow-right" title={s.source}>From {s.source.split(/[\\/]/).pop()}</span>}
      </div>
    </section>
  );
}

function PopupSettings({ settings, run }: { settings: CalSettings; run: Run }) {
  const [s, setS] = useState(settings);
  const [autostart, setAutostart] = useState<boolean | null>(null);
  useEffect(() => { api.calAutostart().then(setAutostart).catch(() => setAutostart(null)); }, []);
  useEffect(() => setS(settings), [settings]);
  const save = (next: CalSettings) => { setS(next); run(() => api.calSetSettings(next)); };
  const test = () => {
    const a = new Audio(`/sounds/${encodeURIComponent(s.sound)}.wav`);
    a.volume = s.volume / 100;
    a.play().catch(() => {});
  };
  return (
    <div className="cal-settings">
      <section className="setting">
        <h3>Quiet hours</h3>
        <p className="muted">No pop-ups in these hours, except from schedules you allow.</p>
        <div className="inline-fields">
          <label className="check"><input type="checkbox" checked={s.quiet.on} onChange={(e) => save({ ...s, quiet: { ...s.quiet, on: e.target.checked } })} /><span>Quiet from</span></label>
          <input type="time" value={minutes(s.quiet.from)} onChange={(e) => e.target.value && save({ ...s, quiet: { ...s.quiet, from: clock(e.target.value) } })} />
          <span>to</span>
          <input type="time" value={minutes(s.quiet.to)} onChange={(e) => e.target.value && save({ ...s, quiet: { ...s.quiet, to: clock(e.target.value) } })} />
        </div>
      </section>
      <section className="setting">
        <h3>Sound</h3>
        <div className="inline-fields">
          <label className="check"><input type="checkbox" checked={s.sound_on} onChange={(e) => save({ ...s, sound_on: e.target.checked })} /><span>Play</span></label>
          <select value={s.sound} onChange={(e) => save({ ...s, sound: e.target.value })}>{SOUNDS.map((x) => <option key={x}>{x}</option>)}</select>
          <input type="range" min={0} max={100} step={5} value={s.volume} aria-label="Volume" onChange={(e) => setS({ ...s, volume: Number(e.target.value) })} onMouseUp={() => save(s)} onKeyUp={() => save(s)} />
          <span className="num small muted">{s.volume}%</span>
          <button className="small-btn" onClick={test}>Play it</button>
        </div>
      </section>
      <section className="setting">
        <h3>Pop-ups</h3>
        <div className="inline-fields">
          <span>Show them</span>
          <select value={s.corner} onChange={(e) => save({ ...s, corner: e.target.value })}>
            <option value="bottom-right">bottom right</option>
            <option value="bottom-left">bottom left</option>
            <option value="top-right">top right</option>
            <option value="top-left">top left</option>
          </select>
          <span>for</span>
          <select value={s.popup_seconds} onChange={(e) => save({ ...s, popup_seconds: Number(e.target.value) })}>
            {[10, 25, 60, 300, 0].map((x) => <option key={x} value={x}>{x === 0 ? "until I close them" : x < 60 ? `${x} seconds` : `${x / 60} min`}</option>)}
          </select>
        </div>
        <label className="check">
          <input type="checkbox" checked={s.hide_from_capture} onChange={(e) => save({ ...s, hide_from_capture: e.target.checked })} />
          <span>Keep pop-ups out of screen shares, recordings and streams<small className="faint">People watching see nothing where the pop-up is. Needs Windows 10 (2020) or newer.</small></span>
        </label>
        <label className="check">
          <input type="checkbox" checked={s.discreet} onChange={(e) => save({ ...s, discreet: e.target.checked })} />
          <span>Discreet pop-ups<small className="faint">Only the schedule name shows until you click. Also keeps titles out of the tray tooltip.</small></span>
        </label>
        <p className="small muted">Pop-ups never take the keyboard away from what you're typing.</p>
      </section>
      <section className="setting">
        <h3>Keep reminders running</h3>
        <p className="muted">Closing the Sompio window keeps it in the tray, next to the clock, so reminders still come. Use Quit in its menu to stop them. After a restart, unlock Sompio once to start the day's reminders.</p>
        {autostart !== null && (
          <label className="check">
            <input type="checkbox" checked={autostart} onChange={async (e) => { const on = e.target.checked; await run(() => api.calSetAutostart(on)); setAutostart(on); }} />
            <span>Start Sompio when I sign in to Windows</span>
          </label>
        )}
      </section>
    </div>
  );
}

function PacksDialog({ mineId, onClose, run }: { mineId: number; onClose: () => void; run: Run }) {
  const [added, setAdded] = useState<string[]>([]);
  const card = (p: (typeof PACKS)[number]) => (
    <article key={p.id} className={`pack${p.id === "privacy" ? " featured" : ""}`}>
      <div>
        <strong>{p.name}</strong>
        <span className="small muted">{p.line}</span>
        <ul className="small">
          {p.rows.map((r) => <li key={r.title}><span className="faint">{r.days}, {r.time.replace(/^every (\d+) min /, "every $1 min, ")}</span> {r.title}</li>)}
        </ul>
      </div>
      <button
        className={p.id === "privacy" ? "primary" : ""}
        disabled={added.includes(p.id)}
        onClick={() => run(() => api.calAddRows(mineId, p.rows), `${p.name} added.`).then(() => setAdded([...added, p.id]))}
      >
        {added.includes(p.id) ? "Added" : "Add"}
      </button>
    </article>
  );
  return (
    <dialog className="sheet wide" ref={(d) => { if (d && !d.open) d.showModal(); }} onClose={onClose} aria-labelledby="packs-title">
      <div className="sheet-body">
        <h2 id="packs-title">Routines</h2>
        <p className="muted">Small habits that keep your accounts, files and data in order. Each adds a few ordinary reminders you can change or remove afterwards.</p>
        <div className="packs">{PACKS.filter((p) => !p.everyday).map(card)}</div>
        <details className="everyday">
          <summary className="small muted">Everyday reminders</summary>
          <div className="packs">{PACKS.filter((p) => p.everyday).map(card)}</div>
        </details>
        <div className="toolbar end"><button onClick={(e) => (e.currentTarget.closest("dialog") as HTMLDialogElement).close()}>Done</button></div>
      </div>
    </dialog>
  );
}

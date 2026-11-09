import { useEffect, useRef, useState } from "react";
import { api, type Occurrence, type RowInput, type ScheduleView } from "../api";

type Repeat = "once" | "daily" | "weekdays" | "weekends" | "days" | "monthly" | "custom";
const WEEK = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const STEPS = [5, 10, 15, 20, 30, 45, 60, 90, 120];
const NOTICES = [0, 5, 10, 15, 30, 60];

interface Form {
  schedule: number;
  title: string;
  text: string;
  repeat: Repeat;
  date: string;
  days: boolean[];
  monthDay: number;
  quarterly: boolean;
  customDays: string;
  repeating: boolean;
  time: string;
  step: number;
  from: string;
  to: string;
  notice: number;
  groups: string[];
}

/** Turns the file's way of writing days back into the form, where it can. */
function readDays(text: string, f: Form): Form {
  const s = text.trim().toLowerCase();
  if (["every day", "everyday", "daily", "*"].includes(s)) return { ...f, repeat: "daily" };
  if (s === "weekdays") return { ...f, repeat: "weekdays" };
  if (s === "weekends") return { ...f, repeat: "days", days: WEEK.map((_, i) => i >= 5) };
  if (/^\d{4}-\d{2}-\d{2}$/.test(s)) return { ...f, repeat: "once", date: s };
  const m = /^(monthly|quarterly)\s*(?:on\s*)?(\d{1,2})$/.exec(s);
  if (m) return { ...f, repeat: "monthly", monthDay: Number(m[2]), quarterly: m[1] === "quarterly" };
  const parts = s.split(",").map((p) => p.trim().slice(0, 3));
  if (parts.every((p) => WEEK.some((w) => w.toLowerCase() === p)))
    return { ...f, repeat: "days", days: WEEK.map((w) => parts.includes(w.toLowerCase())) };
  return { ...f, repeat: "custom", customDays: text };
}

function readTime(text: string, f: Form): Form {
  const s = text.trim().toLowerCase();
  const ev = /^every (\d+) ?min(?:utes)? (\d{1,2}:\d{2})\s*-\s*(\d{1,2}:\d{2})$/.exec(s) ?? /^hourly ()(\d{1,2}:\d{2})\s*-\s*(\d{1,2}:\d{2})$/.exec(s);
  if (ev) return { ...f, repeating: true, step: Number(ev[1] || 60), from: ev[2].padStart(5, "0"), to: ev[3].padStart(5, "0") };
  return { ...f, repeating: false, time: s.padStart(5, "0") };
}

export function daysText(f: Form): string {
  switch (f.repeat) {
    case "once": return f.date;
    case "daily": return "Every day";
    case "weekdays": return "Weekdays";
    case "weekends": return "Weekends";
    case "days": return WEEK.filter((_, i) => f.days[i]).join(", ");
    case "monthly": return `${f.quarterly ? "Quarterly" : "Monthly"} ${f.monthDay}`;
    default: return f.customDays;
  }
}
const timeText = (f: Form) => (f.repeating ? `every ${f.step} min ${f.from}-${f.to}` : f.time);

interface Props {
  schedules: ScheduleView[];
  editing: Occurrence | null;
  day: string;
  onClose: () => void;
  onSaved: () => void;
}

export default function ReminderDialog({ schedules, editing, day, onClose, onSaved }: Props) {
  const ref = useRef<HTMLDialogElement>(null);
  const editable = schedules.filter((s) => s.editable);
  const [f, setF] = useState<Form>(() => {
    const base: Form = {
      schedule: editing?.source_id ?? editable[0]?.id ?? 0,
      title: editing?.title ?? "",
      text: editing?.text ?? "",
      repeat: "once",
      date: day,
      days: WEEK.map(() => false),
      monthDay: Number(day.slice(8, 10)) || 1,
      quarterly: false,
      customDays: "",
      repeating: false,
      time: "09:00",
      step: 60,
      from: "09:00",
      to: "17:00",
      notice: editing?.notice ?? 0,
      groups: editing?.for_groups ?? [],
    };
    return editing ? readTime(editing.time_text, readDays(editing.days_text, base)) : base;
  });
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const set = <K extends keyof Form>(k: K, v: Form[K]) => setF((x) => ({ ...x, [k]: v }));
  const schedule = schedules.find((s) => s.id === f.schedule);

  useEffect(() => {
    ref.current?.showModal();
  }, []);

  const save = async () => {
    const row: RowInput = { days: daysText(f), time: timeText(f), title: f.title, text: f.text, notice: f.notice, for_groups: f.groups };
    setBusy(true);
    setError(null);
    try {
      if (editing) await api.calUpdateRow(f.schedule, editing.line, row);
      else await api.calAddRows(f.schedule, [row]);
      onSaved();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const repeats: [Repeat, string][] = [["once", "Once"], ["daily", "Every day"], ["weekdays", "Weekdays"], ["days", "Some days"], ["monthly", "Monthly"], ["custom", "Other"]];

  return (
    <dialog ref={ref} className="sheet" onClose={onClose} aria-labelledby="reminder-title">
      <form method="dialog" className="sheet-body" onSubmit={(e) => { e.preventDefault(); save(); }}>
        <h2 id="reminder-title">{editing ? "Edit reminder" : "New reminder"}</h2>
        {editing && !editing.one_off && <p className="small muted">Changes apply every time this reminder comes up. To drop just one, use "Skip this time" instead.</p>}
        <label className="field">
          <span>What</span>
          <input type="text" autoFocus value={f.title} maxLength={120} placeholder="Call the dentist" onChange={(e) => set("title", e.target.value)} />
        </label>
        <label className="field">
          <span>Note <span className="faint">(optional)</span></span>
          <input type="text" value={f.text} maxLength={300} placeholder="Shows under the title on the pop-up" onChange={(e) => set("text", e.target.value)} />
        </label>

        <div className="field">
          <span>Which days</span>
          <div className="segmented wrap" role="radiogroup" aria-label="Which days">
            {repeats.map(([k, label]) => (
              <button type="button" key={k} role="radio" aria-checked={f.repeat === k} aria-selected={f.repeat === k} onClick={() => set("repeat", k)}>{label}</button>
            ))}
          </div>
          {f.repeat === "once" && <input type="date" value={f.date} onChange={(e) => set("date", e.target.value)} />}
          {f.repeat === "days" && (
            <div className="day-picks">
              {WEEK.map((w, i) => (
                <button type="button" key={w} aria-pressed={f.days[i]} onClick={() => set("days", f.days.map((d, j) => (j === i ? !d : d)))}>{w}</button>
              ))}
            </div>
          )}
          {f.repeat === "monthly" && (
            <div className="inline-fields">
              <span>On day</span>
              <select value={f.monthDay} onChange={(e) => set("monthDay", Number(e.target.value))}>
                {Array.from({ length: 31 }, (_, i) => i + 1).map((d) => <option key={d}>{d}</option>)}
              </select>
              <select value={f.quarterly ? "q" : "m"} onChange={(e) => set("quarterly", e.target.value === "q")}>
                <option value="m">of every month</option>
                <option value="q">every 3 months (Jan, Apr, Jul, Oct)</option>
              </select>
              {f.monthDay > 28 && <small className="faint">Months without day {f.monthDay} skip it.</small>}
            </div>
          )}
          {f.repeat === "custom" && (
            <>
              <input type="text" value={f.customDays} placeholder="Mon-Fri, 2026-12-24 to 2026-12-31, Quarterly 15…" onChange={(e) => set("customDays", e.target.value)} />
              <small className="faint">Written like in a Desk Reminders schedule: day names, Weekdays, a date, a range of dates, or Monthly / Quarterly and a day.</small>
            </>
          )}
        </div>

        <div className="field">
          <span>When</span>
          <div className="inline-fields">
            <select value={f.repeating ? "r" : "a"} onChange={(e) => set("repeating", e.target.value === "r")}>
              <option value="a">At</option>
              <option value="r">Every</option>
            </select>
            {!f.repeating ? (
              <input type="time" value={f.time} onChange={(e) => set("time", e.target.value)} />
            ) : (
              <>
                <select value={f.step} onChange={(e) => set("step", Number(e.target.value))}>
                  {STEPS.map((s) => <option key={s} value={s}>{s < 60 ? `${s} minutes` : s === 60 ? "hour" : `${s / 60} hours`}</option>)}
                </select>
                <span>from</span>
                <input type="time" value={f.from} onChange={(e) => set("from", e.target.value)} />
                <span>to</span>
                <input type="time" value={f.to} onChange={(e) => set("to", e.target.value)} />
              </>
            )}
          </div>
        </div>

        <div className="field-row">
          <label className="field">
            <span>Heads-up</span>
            <select value={f.notice} onChange={(e) => set("notice", Number(e.target.value))}>
              {NOTICES.map((m) => <option key={m} value={m}>{m === 0 ? "None" : `${m} min before`}</option>)}
            </select>
          </label>
          {!editing && editable.length > 1 && (
            <label className="field grow">
              <span>Schedule</span>
              <select value={f.schedule} onChange={(e) => setF((x) => ({ ...x, schedule: Number(e.target.value), groups: [] }))}>
                {editable.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
              </select>
            </label>
          )}
        </div>

        {schedule && schedule.groups.length > 0 && (
          <div className="field">
            <span>For</span>
            <div className="day-picks">
              {schedule.groups.map((g) => (
                <button type="button" key={g} aria-pressed={f.groups.includes(g)} onClick={() => set("groups", f.groups.includes(g) ? f.groups.filter((x) => x !== g) : [...f.groups, g])}>{g}</button>
              ))}
            </div>
            <small className="faint">{f.groups.length ? "Only people in these groups get a pop-up." : "Everyone on the schedule gets a pop-up."}</small>
          </div>
        )}

        <p className="small faint mono" aria-label="As written in the schedule file">{daysText(f) || "…"} · {timeText(f)}</p>
        {error && <div className="error" role="alert">{error}</div>}
        <div className="toolbar end">
          <button type="button" className="quiet" onClick={() => ref.current?.close()}>Cancel</button>
          <button className="primary" disabled={busy || !f.title.trim() || !daysText(f)}>{busy ? "Saving…" : editing ? "Save" : "Add reminder"}</button>
        </div>
      </form>
    </dialog>
  );
}

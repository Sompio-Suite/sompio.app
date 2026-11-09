import { useCallback, useEffect, useRef, useState } from "react";
import { api, listen, type PopupState } from "../api";

/** The reminder pop-up: its own small window, always on top, never takes focus. */
export default function Popup() {
  const [state, setState] = useState<PopupState | null>(null);
  const [revealed, setRevealed] = useState<number[]>([]);
  const heard = useRef(new Set<number>());

  const load = useCallback(async () => {
    const s = await api.calPopups().catch(() => null);
    if (!s) return;
    setState(s);
    // A sound for each pop-up the first time it shows (snoozed ones ring again).
    const fresh = s.shown.popups.filter((p) => !heard.current.has(p.id));
    fresh.forEach((p) => heard.current.add(p.id));
    if (fresh.length && s.sound_on) {
      const sound = s.sounds[String(fresh[0].occurrence.source_id)] ?? s.sound;
      const a = new Audio(`/sounds/${encodeURIComponent(sound)}.wav`);
      a.volume = s.volume / 100;
      a.play().catch(() => {});
    }
  }, []);

  useEffect(() => {
    document.body.classList.add("popup-body");
    load();
    const un = listen("calendar-popups", () => load());
    return () => { un.then((f) => f()); };
  }, [load]);

  const act = (id: number, action: "done" | "snooze", minutes?: number) => api.calPopupAction(id, action, minutes).then(load);

  if (!state) return null;
  const shown = state.shown.popups.slice(0, 3);
  const more = state.shown.popups.length - shown.length;
  return (
    <div className="popups">
      {shown.map((p) => (
        <Card
          key={`${p.id}-${p.snoozed}`}
          seconds={state.seconds}
          colour={p.occurrence.colour}
          onTimeout={() => act(p.id, "done")}
        >
          <div className="pop-head">
            <span className="pop-source">{p.occurrence.source_name}</span>
            <span className="num">
              {p.occurrence.kind === "notice" ? `at ${p.occurrence.event_at.slice(11, 16)}` : p.occurrence.at.slice(11, 16)}
              {p.snoozed && " · snoozed"}
            </span>
          </div>
          {state.discreet && !revealed.includes(p.id) ? (
            <button className="pop-reveal" onClick={() => setRevealed([...revealed, p.id])}>Reminder. Click to show</button>
          ) : (
            <>
              <strong className="pop-title">{p.occurrence.kind === "notice" ? `In ${p.occurrence.notice} min: ${p.occurrence.title}` : p.occurrence.title}</strong>
              {p.occurrence.text && <span className="pop-text">{p.occurrence.text}</span>}
            </>
          )}
          <div className="pop-actions">
            <button className="primary" onClick={() => act(p.id, "done")}>Done</button>
            <button onClick={() => act(p.id, "snooze", 5)}>5 min</button>
            <button onClick={() => act(p.id, "snooze", 15)}>15 min</button>
            {more > 0 && p === shown[shown.length - 1] && <span className="pop-more">+{more} more</span>}
          </div>
        </Card>
      ))}
    </div>
  );
}

/** One pop-up with a countdown bar. Pointing at it holds the countdown. */
function Card({ seconds, colour, onTimeout, children }: { seconds: number; colour: string; onTimeout: () => void; children: React.ReactNode }) {
  const [left, setLeft] = useState(seconds * 1000);
  const hover = useRef(false);
  const done = useRef(onTimeout);
  done.current = onTimeout;
  useEffect(() => {
    if (seconds <= 0) return;
    const step = 200;
    const t = window.setInterval(() => {
      if (hover.current) return;
      setLeft((l) => {
        if (l <= 0) return 0;
        const n = l - step;
        if (n <= 0) window.setTimeout(() => done.current(), 0);
        return Math.max(0, n);
      });
    }, step);
    return () => window.clearInterval(t);
  }, [seconds]);
  return (
    <article
      className="pop"
      style={{ ["--c" as string]: colour }}
      onMouseEnter={() => (hover.current = true)}
      onMouseLeave={() => (hover.current = false)}
      role="alert"
    >
      {children}
      {seconds > 0 && <span className="pop-timer" style={{ width: `${(left / (seconds * 1000)) * 100}%` }} />}
    </article>
  );
}

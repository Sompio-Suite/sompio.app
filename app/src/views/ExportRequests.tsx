import { useEffect, useState } from "react";
import { api, type ExportRequests as Asked } from "../api";
import { EXPORTS, daysSince, today } from "../exports";

/**
 * The slow part of Trace: data exports take hours to weeks to arrive. People ask for them here,
 * tick "I've asked", and Sompio reminds them when each one should be ready.
 */
export default function ExportRequests({ compact }: { compact?: boolean }) {
  const [asked, setAsked] = useState<Asked>({});
  const [open, setOpen] = useState<string | null>(null);

  useEffect(() => {
    api.exportRequests().then(setAsked).catch(() => {});
  }, []);

  const save = (next: Asked) => {
    setAsked(next);
    api.setExportRequests(next).catch(() => {});
  };
  const mark = (name: string) => save({ ...asked, [name]: today() });
  const unmark = (name: string) => {
    const next = { ...asked };
    delete next[name];
    save(next);
  };

  const slow = EXPORTS.filter((e) => !e.instant);
  const due = slow.filter((e) => asked[e.name] && daysSince(asked[e.name]) >= e.remindAfter);

  return (
    <div className="export-requests">
      {due.length > 0 && (
        <div className="notice" role="status">
          <strong>{due.length === 1 ? `Your ${due[0].name} export` : `${due.length} exports`} should be ready by now.</strong>{" "}
          Check your email for the download link, then add the file here.
        </div>
      )}
      <ul className="export-list">
        {(compact ? slow : EXPORTS).map((e) => {
          const day = asked[e.name];
          const ago = day ? daysSince(day) : 0;
          const isDue = day && ago >= e.remindAfter;
          return (
            <li key={e.name} className={isDue ? "due" : undefined}>
              <div className="export-row">
                <strong>{e.name}</strong>
                <span className="small muted">
                  {day
                    ? `Asked ${ago === 0 ? "today" : ago === 1 ? "yesterday" : `${ago} days ago`} · ${isDue ? "should be ready" : e.wait}`
                    : e.wait[0].toUpperCase() + e.wait.slice(1)}
                </span>
                <span className="export-actions">
                  <button className="link small" aria-expanded={open === e.name} onClick={() => setOpen(open === e.name ? null : e.name)}>
                    How to ask
                  </button>
                  {!e.instant &&
                    (day ? (
                      <button className="quiet small" onClick={() => unmark(e.name)}>Done</button>
                    ) : (
                      <button className="small" onClick={() => mark(e.name)}>I've asked</button>
                    ))}
                </span>
              </div>
              {open === e.name && <p className="small muted export-how">{e.how}</p>}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

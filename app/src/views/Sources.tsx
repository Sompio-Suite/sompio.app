import { useEffect, useState } from "react";
import {
  api, listen, onFileDrop, pickFolders, pickPackage, platformName,
  type ImportProgress, type ScanSummary, type SourceView,
} from "../api";
import { dateTime, n, plural } from "../format";

interface Props {
  demo: boolean;
  onScan: (s: ScanSummary) => void;
  /** Something was added or removed: other screens should reload. */
  onChanged: () => void;
}

interface Busy { label: string; done: number; total: number }

const HOW_TO: [string, string][] = [
  ["Discord", "User Settings, then Data & Privacy, then Request all of my data. Discord emails you a link."],
  ["Reddit", "Settings, then Privacy, then Request your data. Reddit emails you when it's ready."],
  ["Telegram", "In Telegram Desktop: Settings, Advanced, Export Telegram data. Tick Personal information and your chats, choose JSON. It saves straight to your PC."],
  ["X", "Settings, then Your account, then Download an archive of your data. X emails you when it's ready."],
];

export default function Sources({ demo, onScan, onChanged }: Props) {
  const [sources, setSources] = useState<SourceView[]>([]);
  const [busy, setBusy] = useState<Busy | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<string | null>(null);
  const [over, setOver] = useState(false);

  const refresh = () => api.sources().then(setSources);

  /** Adds a source, then rescans everything so the Review list includes it. */
  const add = async (label: string, load: () => Promise<{ messages: number; warnings: string[] }>) => {
    if (busy) return;
    setError(null);
    setNote(null);
    setBusy({ label, done: 0, total: 0 });
    try {
      const summary = await load();
      setBusy({ label: "Checking for personal information", done: 0, total: 0 });
      onScan(await api.runScan(await api.watchTerms()));
      setNote([`Added ${plural(summary.messages, "item", "items")}.`, ...summary.warnings].join(" "));
      await refresh();
      onChanged();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };
  const addExport = (path: string | null) => path && add("Reading export", () => api.importPackage(path));
  const addFolders = (folders: string[]) => folders.length > 0 && add("Reading files", () => api.scanFolders(folders));

  const remove = async (s: SourceView) => {
    await api.removeSource(s.platform);
    setConfirm(null);
    onScan(await api.runScan(await api.watchTerms()));
    await refresh();
    onChanged();
  };

  useEffect(() => {
    refresh();
    const a = listen<ImportProgress>("import-progress", (p) => setBusy((b) => (b ? { label: p.phase, done: p.done, total: p.total } : b)));
    const b = listen<[number, number]>("scan-progress", ([done, total]) => setBusy((x) => (x ? { label: "Checking for personal information", done, total } : x)));
    const drop = onFileDrop((p) => addExport(p), setOver);
    return () => {
      a.then((f) => f());
      b.then((f) => f());
      drop.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const pct = busy && busy.total > 0 ? (busy.done / busy.total) * 100 : 0;
  const pcSource = sources.find((s) => s.platform === "pc");

  return (
    <div className="page">
      <div className="lede">
        <h1>Sources</h1>
        <p className="muted">
          Add the export from each platform you use, and the folders on this PC you want checked. Everything is read here,
          offline, and lands in one Review list.
        </p>
      </div>

      {busy && (
        <div className="progress" role="status" aria-live="polite">
          <span>
            {busy.label}
            {busy.total > 0 && ` (${n(Math.round(busy.done))} of ${n(busy.total)})`}
          </span>
          <div className="bar">
            <span className={busy.total === 0 ? "indeterminate" : undefined} style={busy.total ? { width: `${pct}%` } : undefined} />
          </div>
        </div>
      )}
      {error && <div className="error">{error}</div>}
      {note && <p className="small muted">{note}</p>}

      <section className="setting">
        <h3>Added so far</h3>
        {sources.length === 0 && <p className="muted">Nothing yet.</p>}
        <div className="rows">
          {sources.map((s) => (
            <div key={s.platform} className="source-row">
              <div>
                <strong>{platformName[s.platform]}</strong>
                <span className="small muted">
                  {s.platform === "pc" ? plural(s.messages, "file", "files") : plural(s.messages, "message", "messages")}
                  {", added "}
                  {dateTime(s.imported_at)}
                </span>
                <span className="small muted origin" title={s.origin}>{demo ? "Sample data" : s.origin.split("\n").join(", ")}</span>
              </div>
              {confirm === s.platform ? (
                <div className="toolbar">
                  <button className="danger" onClick={() => remove(s)}>Remove</button>
                  <button onClick={() => setConfirm(null)}>Cancel</button>
                </div>
              ) : (
                <button onClick={() => setConfirm(s.platform)} disabled={!!busy}>Remove…</button>
              )}
            </div>
          ))}
        </div>
        {sources.length > 0 && (
          <p className="small muted">Removing a source deletes its copy from Veil. Your decisions are kept in case you add it again.</p>
        )}
      </section>

      <div className="split">
        <section className={`drop${over ? " over" : ""}`}>
          <h3>Add a data export</h3>
          <p className="muted small">
            Discord, Reddit, Telegram or X. Drop the .zip here or choose it. Adding a newer export replaces the older one.
          </p>
          <div className="drop-actions">
            <button className="primary" disabled={!!busy} onClick={async () => addExport(await pickPackage(false))}>Choose .zip file</button>
            <button disabled={!!busy} onClick={async () => addExport(await pickPackage(true))}>Choose folder</button>
          </div>
        </section>
        <section className="drop">
          <h3>Scan folders on this PC</h3>
          <p className="muted small">
            Finds passport scans, password lists, bank statements and files with personal details in them. Documents,
            Downloads and Desktop are good places to start.
            {pcSource && " Scanning again replaces the last folder scan."}
          </p>
          <div className="drop-actions">
            <button disabled={!!busy} onClick={async () => addFolders(await pickFolders())}>Choose folders</button>
          </div>
        </section>
      </div>

      <section className="setting">
        <h3>Getting your exports</h3>
        <dl className="howto">
          {HOW_TO.map(([name, how]) => (
            <div key={name}>
              <dt>{name}</dt>
              <dd className="muted">{how}</dd>
            </div>
          ))}
        </dl>
      </section>
    </div>
  );
}

import { useEffect, useState } from "react";
import {
  api, listen, onFileDrop, pickPackage, platformName,
  type ImportProgress, type PcScanOptions, type ScanSummary, type SourceView,
} from "../api";
import { dateTime, n, plural } from "../format";
import Icon from "../Icon";
import EmailConnect from "./EmailConnect";
import ExportRequests from "./ExportRequests";
import FolderScan from "./FolderScan";
import TelegramConnect from "./TelegramConnect";

interface Props {
  demo: boolean;
  onScan: (s: ScanSummary) => void;
  /** Something was added or removed: other screens should reload. */
  onChanged: () => void;
}

interface Busy { label: string; done: number; total: number }


/** The names you go by, for exports that don't say which messages are yours (WhatsApp, Facebook, Instagram). */
function YourNames() {
  const [value, setValue] = useState("");
  const [saved, setSaved] = useState("");
  useEffect(() => {
    api.myNames().then((n) => {
      setValue(n.join(", "));
      setSaved(n.join(", "));
    }).catch(() => {});
  }, []);
  const save = async () => {
    const names = value.split(",").map((x) => x.trim()).filter(Boolean);
    await api.setMyNames(names);
    setValue(names.join(", "));
    setSaved(names.join(", "));
  };
  return (
    <div className="panel connect">
      <div className="connect-head">
        <span className="step-icon"><Icon name="user" /></span>
        <div className="text">
          <strong>Your names</strong>
          <span className="small muted">
            WhatsApp, Facebook and Instagram exports include everyone's messages. Sompio only keeps yours, so it needs to
            know the names you appear under. Usually it works this out itself; add names here if an import says it couldn't.
          </span>
        </div>
      </div>
      <form className="connect-form" onSubmit={(e) => { e.preventDefault(); save(); }}>
        <label>Names, separated by commas
          <input type="text" value={value} onChange={(e) => setValue(e.target.value)} placeholder="Alex de Vries, alexdv" />
        </label>
        <div className="toolbar">
          <button disabled={value === saved}>Save</button>
          {value === saved && saved && <span className="small faint">Saved. Import again to apply.</span>}
        </div>
      </form>
    </div>
  );
}

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
  const addFolders = (folders: string[], opts: PcScanOptions) =>
    folders.length > 0 && add("Reading files", () => api.scanFolders(folders, opts));

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
      <div className="page-head">
        <div>
          <h1>Sources</h1>
          <p className="muted">
            Add the export from each platform you use, connect accounts where that's possible, and choose folders on this
            computer to check. It all lands in one Review list.
          </p>
        </div>
      </div>

      {busy && (
        <div className="panel" style={{ padding: "1.25rem 1.5rem" }}>
          <div className="progress" role="status" aria-live="polite">
            <span>
              {busy.label}
              {busy.total > 0 && ` (${n(Math.round(busy.done))} of ${n(busy.total)})`}
            </span>
            <div className="bar">
              <span className={busy.total === 0 ? "indeterminate" : undefined} style={busy.total ? { width: `${pct}%` } : undefined} />
            </div>
          </div>
        </div>
      )}
      {error && <div className="error">{error}</div>}
      {note && <p className="small muted">{note}</p>}

      <section className="section">
        <h2 className="section-title">Added so far</h2>
        <div className="panel" style={{ padding: "0 1.25rem" }}>
          {sources.length === 0 && <p className="muted" style={{ padding: "1rem 0" }}>Nothing yet.</p>}
          <div className="source-list">
            {sources.map((s) => (
              <div key={s.platform} className="source-row">
                <span className="step-icon"><Icon name={s.platform === "pc" ? "folder" : s.platform === "email" ? "mail" : "messages"} /></span>
                <div className="text">
                  <strong>{platformName[s.platform]}</strong>
                  <span className="small muted">
                    {s.platform === "pc" ? plural(s.messages, "file", "files") : s.platform === "email" ? plural(s.messages, "email", "emails") : plural(s.messages, "message", "messages")}
                    {", added "}
                    {dateTime(s.imported_at)}
                  </span>
                  <span className="small faint origin" title={s.origin}>{demo ? "Sample data" : s.origin.split("\n").join(", ")}</span>
                </div>
                {confirm === s.platform ? (
                  <div className="toolbar">
                    <button className="danger" onClick={() => remove(s)}>Remove</button>
                    <button className="quiet" onClick={() => setConfirm(null)}>Cancel</button>
                  </div>
                ) : (
                  <button className="quiet" onClick={() => setConfirm(s.platform)} disabled={!!busy}>Remove…</button>
                )}
              </div>
            ))}
          </div>
        </div>
        {sources.length > 0 && (
          <p className="small faint">Removing a source deletes Sompio's copy of it. Your decisions are kept in case you add it again.</p>
        )}
      </section>

      <section className="section">
        <h2 className="section-title">Connected accounts</h2>
        <TelegramConnect
          onStatus={() => onChanged()}
          onSynced={async (count) => {
            onScan(await api.runScan(await api.watchTerms()));
            setNote(`Synced ${plural(count, "message", "messages")} you sent on Telegram.`);
            await refresh();
            onChanged();
          }}
        />
        <EmailConnect
          onStatus={() => onChanged()}
          onSynced={async (count) => {
            onScan(await api.runScan(await api.watchTerms()));
            setNote(`Synced ${plural(count, "email", "emails")} you sent.`);
            await refresh();
            onChanged();
          }}
        />
        <p className="small faint">
          Connected accounts are the only time Sompio goes online, and then only to that platform or mail server, when you
          sync or delete.
        </p>
      </section>

      <section className="section">
        <h2 className="section-title">Add more</h2>
        <div className="add-grid">
          <div className={`panel add-option${over ? " over" : ""}`}>
            <span className="step-icon"><Icon name="sources" /></span>
            <h3>A data export</h3>
            <p className="muted small">
              Discord, Reddit, Telegram, X, WhatsApp, Instagram, Facebook, TikTok, Snapchat, LinkedIn or Gmail. Drop the
              file here or choose it. A newer export replaces the older one from the same platform.
            </p>
            <div className="drop-actions">
              <button className="primary" disabled={!!busy} onClick={async () => addExport(await pickPackage(false))}>Choose file</button>
              <button className="quiet" disabled={!!busy} onClick={async () => addExport(await pickPackage(true))}>Choose a folder</button>
            </div>
          </div>
          <div className="panel add-option">
            <span className="step-icon"><Icon name="folder" /></span>
            <h3>Folders on this computer</h3>
            <p className="muted small">
              Finds passport scans, password lists, bank statements and files with personal details. Documents, Downloads
              and Desktop are good places to start.
              {pcSource && " Scanning again replaces the last folder scan."}
            </p>
            <FolderScan disabled={!!busy} onStart={addFolders} />
          </div>
        </div>
      </section>

      <section className="section">
        <h2 className="section-title">Which messages are yours</h2>
        <YourNames />
      </section>

      <section className="section">
        <h2 className="section-title">Getting your exports</h2>
        <p className="small muted">Ask each platform for your data, tick "I've asked", and Sompio reminds you when it should be ready.</p>
        <div className="panel" style={{ padding: "0 1.25rem" }}>
          <ExportRequests />
        </div>
      </section>
    </div>
  );
}

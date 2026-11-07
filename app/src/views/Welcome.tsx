import { useEffect, useRef, useState } from "react";
import { api, listen, onFileDrop, pickFolders, pickPackage, type ImportProgress, type ScanSummary } from "../api";
import { n } from "../format";
import Icon from "../Icon";

interface Busy { label: string; done: number; total: number }

export default function Welcome({ onReady }: { onReady: (s: ScanSummary) => void }) {
  const [busy, setBusy] = useState<Busy | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [over, setOver] = useState(false);
  const busyRef = useRef(false);

  /** Import (or load the sample), then scan straight away so the first thing people see is the answer. */
  const start = async (load: () => Promise<unknown>) => {
    if (busyRef.current) return;
    busyRef.current = true;
    setError(null);
    setBusy({ label: "Opening", done: 0, total: 0 });
    try {
      await load();
      setBusy({ label: "Checking for personal information", done: 0, total: 0 });
      onReady(await api.runScan(await api.watchTerms()));
    } catch (e) {
      setError(String(e));
      setBusy(null);
      busyRef.current = false;
    }
  };
  const importPath = (path: string | null) => path && start(() => api.importPackage(path));

  useEffect(() => {
    const a = listen<ImportProgress>("import-progress", (p) =>
      setBusy({ label: p.phase === "Importing messages" ? "Reading conversations" : p.phase, done: p.done, total: p.total }),
    );
    const b = listen<[number, number]>("scan-progress", ([done, total]) =>
      setBusy({ label: "Checking for personal information", done, total }),
    );
    const drop = onFileDrop((p) => importPath(p), setOver);
    return () => {
      a.then((f) => f());
      b.then((f) => f());
      drop.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const pct = busy && busy.total > 0 ? (busy.done / busy.total) * 100 : 0;

  return (
    <div className="welcome">
      <div className="welcome-inner">
        <div className="brand" style={{ padding: 0 }}>
          <span className="brand-mark" aria-hidden /> Veil
        </div>
        <div className="lede">
          <h1>
            See what your chats and files say about <span className="redact">y••</span>.
          </h1>
          <p className="muted">
            Veil reads your data exports from Discord, Reddit, Telegram and X, and the folders on this PC you choose. It
            shows you where you've shared things like your phone number, address or a password, and helps you clean up.
          </p>
        </div>

        {busy ? (
          <div className="panel" style={{ padding: "1.5rem" }}>
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
        ) : (
          <div className={`panel choices${over ? " over" : ""}`}>
            <button className="choice main" onClick={async () => importPath(await pickPackage(false))}>
              <span className="step-icon"><Icon name="sources" /></span>
              <span className="step-text">
                <strong>Open a data export</strong>
                <span className="muted small">The .zip from Discord, Reddit, Telegram or X. You can also drop it here.</span>
              </span>
              <span className="step-go"><Icon name="arrow" /></span>
            </button>
            <button
              className="choice"
              onClick={async () => {
                const folders = await pickFolders();
                if (folders.length) start(() => api.scanFolders(folders));
              }}
            >
              <span className="step-icon"><Icon name="folder" /></span>
              <span className="step-text">
                <strong>Scan folders on this PC</strong>
                <span className="muted small">Finds passport scans, password lists and files with personal details.</span>
              </span>
              <span className="step-go"><Icon name="arrow" /></span>
            </button>
            <button className="choice" onClick={() => start(api.loadDemo)}>
              <span className="step-icon"><Icon name="eye" /></span>
              <span className="step-text">
                <strong>Try it with sample data</strong>
                <span className="muted small">A made-up chat history, while you wait for your own exports.</span>
              </span>
              <span className="step-go"><Icon name="arrow" /></span>
            </button>
          </div>
        )}
        {error && <div className="error">{error}</div>}
        {!busy && (
          <p className="small muted">
            Already extracted an export? <button className="link small" onClick={async () => importPath(await pickPackage(true))}>Choose its folder instead</button>.
            {" "}Don't have one yet? Each platform has a "request your data" option in its settings.
          </p>
        )}

        <div className="promises">
          <div>
            <h3><Icon name="lock" size={16} />Works offline</h3>
            <p>Veil never connects to the internet. No account, no analytics, no tracking.</p>
          </div>
          <div>
            <h3><Icon name="lock" size={16} />No logins</h3>
            <p>Veil doesn't ask for your passwords or tokens. It only reads the files you give it.</p>
          </div>
          <div>
            <h3><Icon name="lock" size={16} />Gone when you say so</h3>
            <p>Everything sits in one file on this PC. Settings has a button that deletes it for good.</p>
          </div>
        </div>
      </div>
    </div>
  );
}

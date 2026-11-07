import { useEffect, useRef, useState } from "react";
import { api, listen, onFileDrop, pickFolders, pickPackage, type ImportProgress, type ScanSummary } from "../api";
import { n } from "../format";

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
    setBusy({ label: "Opening package", done: 0, total: 0 });
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
        <div className="lede">
          <h1>
            See what your chats and files say about <span className="redact">y••</span>.
          </h1>
          <p className="muted">
            Veil reads the data exports from Discord, Reddit, Telegram and X, and the folders on this PC you choose. It
            points out where you shared things like your phone number, address or a password, then helps you clean up.
          </p>
        </div>

        <div className={`drop${over ? " over" : ""}`}>
          {busy ? (
            <div className="progress" role="status" aria-live="polite">
              <span>
                {busy.label}
                {busy.total > 0 && ` (${n(Math.round(busy.done))} of ${n(busy.total)})`}
              </span>
              <div className="bar">
                <span className={busy.total === 0 ? "indeterminate" : undefined} style={busy.total ? { width: `${pct}%` } : undefined} />
              </div>
            </div>
          ) : (
            <>
              <h3>Start with an export or your PC</h3>
              <p className="muted small">
                Drop a data export (.zip) from Discord, Reddit, Telegram or X here, or choose one. You can add more sources
                afterwards.
              </p>
              <div className="drop-actions">
                <button className="primary" onClick={async () => importPath(await pickPackage(false))}>
                  Choose an export
                </button>
                <button onClick={async () => importPath(await pickPackage(true))}>Choose an extracted folder</button>
                <button
                  onClick={async () => {
                    const folders = await pickFolders();
                    if (folders.length) start(() => api.scanFolders(folders));
                  }}
                >
                  Scan folders on this PC
                </button>
              </div>
            </>
          )}
          {error && <div className="error">{error}</div>}
        </div>

        <div className="split">
          <div>
            <h3>Don't have an export yet?</h3>
            <p className="muted">
              Each platform has a "request your data" option in its settings and emails you a download. Telegram Desktop
              exports straight to your PC. The Sources screen has the steps for each.
            </p>
          </div>
          <div>
            <h3>See how it works first</h3>
            <p className="muted" style={{ marginBottom: "0.75rem" }}>
              Load a made-up chat history and try the whole review. Opening your own package later replaces it.
            </p>
            <button onClick={() => start(api.loadDemo)} disabled={!!busy}>Try it with sample data</button>
          </div>
        </div>

        <div className="promises">
          <div>
            <h3>Works offline</h3>
            <p>Veil never connects to the internet. There is no account, no analytics and no update check.</p>
          </div>
          <div>
            <h3>No Discord login</h3>
            <p>Veil doesn't ask for your passwords or tokens. It only reads the files you give it.</p>
          </div>
          <div>
            <h3>Gone when you say so</h3>
            <p>Everything Veil stores sits in one file on this computer. Settings has a button that deletes it.</p>
          </div>
        </div>
      </div>
    </div>
  );
}

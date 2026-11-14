import { useEffect, useRef, useState } from "react";
import { api, listen, onFileDrop, pickPackage, type ImportProgress, type ScanSummary } from "../api";
import { n } from "../format";
import Icon from "../Icon";
import EmailConnect from "./EmailConnect";
import ExportRequests from "./ExportRequests";
import FolderScan from "./FolderScan";
import TelegramConnect from "./TelegramConnect";

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
        <div className="eyebrow">
          <Icon name="review" size={16} /> Sompio Trace
        </div>
        <div className="lede">
          <h1>
            See what your chats and files say about <span className="hl">you</span>.
          </h1>
          <p className="muted">
            Trace finds where you've shared things like your phone number, address, ID number or a password, and helps you
            clean up. Start with what's on this PC and your mailbox, which takes minutes. Ask for your exports from Discord,
            Instagram and the rest at the same time: they arrive in a few days.
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
          <>
            <h2 className="welcome-step"><span className="num">1</span> Start now <span className="small muted">· ready in minutes</span></h2>
            <div className="panel welcome-now">
              <div className="choice-static">
                <span className="step-icon"><Icon name="folder" /></span>
                <span className="step-text">
                  <strong>Folders on this PC</strong>
                  <span className="muted small">Passport scans, password lists, bank statements and files with personal details. Documents, Downloads and Desktop are good places to start.</span>
                </span>
              </div>
              <FolderScan onStart={(folders, opts) => start(() => api.scanFolders(folders, opts))} />
            </div>
            <EmailConnect onSynced={async () => start(async () => {})} />
            <TelegramConnect onSynced={async () => start(async () => {})} />
            <div className={`panel choices${over ? " over" : ""}`}>
              <button className="choice" onClick={async () => importPath(await pickPackage(false))}>
                <span className="step-icon"><Icon name="sources" /></span>
                <span className="step-text">
                  <strong>A WhatsApp chat, or an export you already have</strong>
                  <span className="muted small">WhatsApp exports a chat in seconds from your phone. Any platform's export works here. You can also drop the file here.</span>
                </span>
                <span className="step-go"><Icon name="arrow" /></span>
              </button>
              <button className="choice" onClick={() => start(api.loadDemo)}>
                <span className="step-icon"><Icon name="eye" /></span>
                <span className="step-text">
                  <strong>Try it with sample data</strong>
                  <span className="muted small">A made-up person's chats and files, to see what Trace finds before you use your own.</span>
                </span>
                <span className="step-go"><Icon name="arrow" /></span>
              </button>
            </div>
            {error && <div className="error">{error}</div>}
            <p className="small muted">
              Already extracted an export? <button className="link small" onClick={async () => importPath(await pickPackage(true))}>Choose its folder instead</button>.
            </p>

            <h2 className="welcome-step"><span className="num">2</span> Ask for the rest <span className="small muted">· arrives in a few days</span></h2>
            <p className="small muted">
              Every platform must give you a copy of your data. Ask for it now, tick "I've asked", and Sompio reminds you when
              each one should be ready.
            </p>
            <div className="panel" style={{ padding: "0 1.25rem" }}>
              <ExportRequests compact />
            </div>
          </>
        )}
        {busy && error && <div className="error">{error}</div>}

        <div className="promises">
          <div>
            <h3><Icon name="lock" size={16} />Works offline</h3>
            <p>Trace reads exports on this PC. It only goes online if you connect an account, and then only to that service. No analytics, no tracking.</p>
          </div>
          <div>
            <h3><Icon name="lock" size={16} />No logins needed</h3>
            <p>Exports work without signing in anywhere. Connecting an account is optional.</p>
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

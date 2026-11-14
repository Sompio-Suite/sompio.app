import { useEffect, useState } from "react";
import { api, listen, openLink, type EmailProgress, type EmailProvider, type EmailStatus } from "../api";
import { n } from "../format";
import Icon from "../Icon";

interface Props {
  /** Runs after a sync stored new emails: rescan and refresh. */
  onSynced: (messages: number) => Promise<void>;
  /** Connected or disconnected. */
  onStatus?: () => void;
}

const APP_PASSWORD_HELP: Record<string, string> = {
  gmail: "Gmail needs 2-Step Verification turned on. Then open App passwords in your Google Account, create one called Sompio, and paste the 16 letters here.",
  yahoo: "In Yahoo's Account security page, choose Generate app password, call it Sompio, and paste it here.",
  icloud: "Sign in at account.apple.com, open Sign-In and Security, then App-Specific Passwords. Create one called Sompio and paste it here.",
  gmx: "In GMX settings, turn on POP3 & IMAP access, and use an app password if you have two-factor sign-in on.",
  fastmail: "In Fastmail, open Settings, Privacy & Security, then Manage app passwords. Create one with IMAP access.",
  other: "Use your provider's IMAP server (it usually starts with imap.) and an app password if they offer them.",
};

export default function EmailConnect({ onSynced, onStatus }: Props) {
  const [status, setStatus] = useState<EmailStatus | null>(null);
  const [providers, setProviders] = useState<EmailProvider[]>([]);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<EmailProgress | null>(null);
  const [confirmDisconnect, setConfirmDisconnect] = useState(false);
  const [f, setF] = useState({ provider: "gmail", host: "", address: "", password: "" });
  const set = (k: keyof typeof f) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => setF({ ...f, [k]: e.target.value });

  useEffect(() => {
    api.emailStatus().then(setStatus).catch(() => setStatus({ connected: false, address: null, provider: null }));
    api.emailProviders().then(setProviders).catch(() => {});
    const un = listen<EmailProgress>("email-progress", setProgress);
    return () => void un.then((x) => x());
  }, []);

  const run = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      setProgress(null);
    }
  };

  const sync = () =>
    run(async () => {
      setProgress({ done: 0, total: 0 });
      const summary = await api.emailSync();
      await onSynced(summary.messages);
    });

  if (!status) return null;
  const provider = providers.find((p) => p.id === f.provider);
  const connectedName = providers.find((p) => p.id === status.provider)?.name;

  return (
    <div className="panel connect">
      <div className="connect-head">
        <span className={`step-icon${status.connected ? " good" : ""}`}><Icon name={status.connected ? "check" : "mail"} /></span>
        <div className="text">
          <strong>Email</strong>
          <span className="small muted">
            {status.connected
              ? `Connected as ${status.address}${connectedName ? ` (${connectedName})` : ""}. Sompio reads your Sent folder and can move emails to Trash.`
              : "Check everything you've sent for personal details. Gmail, Yahoo, iCloud and most other providers."}
          </span>
        </div>
        {status.connected && !progress && (
          confirmDisconnect ? (
            <div className="toolbar">
              <button className="danger" disabled={busy} onClick={() => run(async () => {
                await api.emailDisconnect();
                setConfirmDisconnect(false);
                setStatus(await api.emailStatus());
                onStatus?.();
              })}>Disconnect</button>
              <button className="quiet" onClick={() => setConfirmDisconnect(false)}>Cancel</button>
            </div>
          ) : (
            <div className="toolbar">
              <button className="primary" disabled={busy} onClick={sync}>Sync now</button>
              <button className="quiet" disabled={busy} onClick={() => setConfirmDisconnect(true)}>Disconnect…</button>
            </div>
          )
        )}
        {!status.connected && !open && <div className="toolbar"><button onClick={() => setOpen(true)}>Connect…</button></div>}
      </div>

      {progress && (
        <div className="progress" role="status" aria-live="polite">
          <span>
            Reading your sent mail
            {progress.total > 0 && ` (${n(progress.done)} of ${n(progress.total)})`}
          </span>
          <div className="bar">
            <span className={progress.total === 0 ? "indeterminate" : undefined}
              style={progress.total ? { width: `${(progress.done / progress.total) * 100}%` } : undefined} />
          </div>
        </div>
      )}

      {!status.connected && open && (
        <form
          className="connect-form"
          onSubmit={(e) => {
            e.preventDefault();
            run(async () => {
              const s = await api.emailConnect(f.provider, f.host, f.address, f.password);
              setF({ ...f, password: "" });
              setStatus(s);
              setOpen(false);
              onStatus?.();
              await sync();
            });
          }}
        >
          <div className="field-row">
            <label>Provider
              <select value={f.provider} onChange={set("provider")}>
                {providers.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
              </select>
            </label>
            {f.provider === "other" && (
              <label className="grow">IMAP server<input type="text" value={f.host} onChange={set("host")} placeholder="imap.example.com" /></label>
            )}
          </div>
          <label>Email address
            <input type="email" autoComplete="email" value={f.address} onChange={set("address")} placeholder="you@example.com" />
          </label>
          <label>App password
            <input type="password" autoComplete="off" value={f.password} onChange={set("password")} />
          </label>
          <p className="small muted">
            {APP_PASSWORD_HELP[f.provider]}{" "}
            {provider?.help_url && (
              <button type="button" className="link" onClick={() => openLink(provider.help_url)}>Open that page</button>
            )}
          </p>
          <div className="toolbar">
            <button className="primary" disabled={busy || !f.address.includes("@") || !f.password || (f.provider === "other" && !f.host)}>
              {busy ? "Checking…" : "Connect"}
            </button>
            <button type="button" className="quiet" onClick={() => setOpen(false)}>Cancel</button>
          </div>
          <p className="small faint">
            An app password only opens your mail, can't change your account, and you can revoke it any time. Sompio keeps it
            in your computer's own password store (Windows Credential Manager, the Mac's Keychain) and only talks to your mail server. Outlook.com and Hotmail need Microsoft
            sign-in, which Sompio doesn't support yet.
          </p>
        </form>
      )}

      {error && <div className="error">{error}</div>}
    </div>
  );
}

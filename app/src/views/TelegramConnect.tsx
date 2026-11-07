import { useEffect, useState } from "react";
import { api, listen, type TgProgress, type TgStatus } from "../api";
import { n } from "../format";
import Icon from "../Icon";

interface Props {
  /** Runs after a sync stored new messages: rescan and refresh. */
  onSynced: (messages: number) => Promise<void>;
  onStatus?: (s: TgStatus) => void;
}

type Step = "key" | "phone" | "code" | "password" | "connected";

export default function TelegramConnect({ onSynced, onStatus }: Props) {
  const [status, setStatus] = useState<TgStatus | null>(null);
  const [step, setStep] = useState<Step>("phone");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hint, setHint] = useState<string | null>(null);
  const [progress, setProgress] = useState<TgProgress | null>(null);
  const [confirmDisconnect, setConfirmDisconnect] = useState(false);
  const [f, setF] = useState({ apiId: "", apiHash: "", phone: "", code: "", password: "" });
  const set = (k: keyof typeof f) => (e: React.ChangeEvent<HTMLInputElement>) => setF({ ...f, [k]: e.target.value });

  const refresh = async () => {
    try {
      const s = await api.tgStatus();
      setStatus(s);
      onStatus?.(s);
      setStep(!s.has_app_key ? "key" : s.connected ? "connected" : s.waiting_for ?? "phone");
    } catch (e) {
      setError(String(e));
      setStatus({ has_app_key: true, connected: false, name: null, waiting_for: null });
    }
  };

  useEffect(() => {
    refresh();
    const un = listen<TgProgress>("tg-progress", setProgress);
    return () => void un.then((x) => x());
    // eslint-disable-next-line react-hooks/exhaustive-deps
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
    }
  };

  const sync = () =>
    run(async () => {
      setProgress({ chats_done: 0, chats_total: 0, messages: 0 });
      const summary = await api.tgSync();
      await onSynced(summary.messages);
      setProgress(null);
    }).finally(() => setProgress(null));

  const finishLogin = async (name: string) => {
    setF({ ...f, code: "", password: "" });
    await refresh();
    setHint(null);
    setStatus((s) => (s ? { ...s, name } : s));
    await sync();
  };

  if (!status) return null;

  return (
    <div className="panel connect">
      <div className="connect-head">
        <span className={`step-icon${step === "connected" ? " good" : ""}`}><Icon name={step === "connected" ? "check" : "messages"} /></span>
        <div className="text">
          <strong>Telegram</strong>
          <span className="small muted">
            {step === "connected"
              ? `Connected as ${status.name ?? "your account"}. Veil reads the messages you sent and can delete them for you.`
              : "Connect your account to check and delete your own messages without exporting first."}
          </span>
        </div>
        {step === "connected" && !progress && (
          confirmDisconnect ? (
            <div className="toolbar">
              <button className="danger" disabled={busy} onClick={() => run(async () => { await api.tgDisconnect(); setConfirmDisconnect(false); await refresh(); })}>Disconnect</button>
              <button className="quiet" onClick={() => setConfirmDisconnect(false)}>Cancel</button>
            </div>
          ) : (
            <div className="toolbar">
              <button className="primary" disabled={busy} onClick={sync}>Sync now</button>
              <button className="quiet" disabled={busy} onClick={() => setConfirmDisconnect(true)}>Disconnect…</button>
            </div>
          )
        )}
      </div>

      {progress && (
        <div className="progress" role="status" aria-live="polite">
          <span>
            Reading your messages
            {progress.chats_total > 0 && ` (chat ${n(progress.chats_done)} of ${n(progress.chats_total)}, ${n(progress.messages)} messages so far)`}
          </span>
          <div className="bar">
            <span className={progress.chats_total === 0 ? "indeterminate" : undefined}
              style={progress.chats_total ? { width: `${(progress.chats_done / progress.chats_total) * 100}%` } : undefined} />
          </div>
        </div>
      )}

      {step === "key" && (
        <form className="connect-form" onSubmit={(e) => { e.preventDefault(); run(async () => { await api.tgSetAppKey(f.apiId, f.apiHash); await refresh(); }); }}>
          <p className="small muted">
            Telegram asks every app for its own key. Get one for free: sign in at my.telegram.org, open API development
            tools, create an app with any name, and copy the two values here. You only do this once.
          </p>
          <div className="field-row">
            <label>api_id<input type="text" inputMode="numeric" value={f.apiId} onChange={set("apiId")} placeholder="1234567" /></label>
            <label className="grow">api_hash<input type="text" value={f.apiHash} onChange={set("apiHash")} placeholder="32 letters and numbers" /></label>
          </div>
          <div><button className="primary" disabled={busy || !f.apiId || !f.apiHash}>Save key</button></div>
        </form>
      )}

      {step === "phone" && (
        <form className="connect-form" onSubmit={(e) => { e.preventDefault(); run(async () => { await api.tgStartLogin(f.phone); setStep("code"); }); }}>
          <label>Your phone number, with country code
            <input type="text" inputMode="tel" autoComplete="tel" value={f.phone} onChange={set("phone")} placeholder="+31 6 12345678" />
          </label>
          <div><button className="primary" disabled={busy || f.phone.length < 8}>Send me a code</button></div>
          <p className="small faint">Telegram sends the code to your Telegram app. Your number goes to Telegram only; Veil doesn't keep it.</p>
        </form>
      )}

      {step === "code" && (
        <form className="connect-form" onSubmit={(e) => { e.preventDefault(); run(async () => {
          const r = await api.tgSubmitCode(f.code);
          if (r.step === "need_password") { setHint(r.hint); setStep("password"); } else await finishLogin(r.name);
        }); }}>
          <label>The code Telegram just sent you
            <input type="text" inputMode="numeric" autoComplete="one-time-code" value={f.code} onChange={set("code")} placeholder="12345" autoFocus />
          </label>
          <div className="toolbar">
            <button className="primary" disabled={busy || f.code.length < 4}>Continue</button>
            <button type="button" className="quiet" onClick={() => setStep("phone")}>Use another number</button>
          </div>
        </form>
      )}

      {step === "password" && (
        <form className="connect-form" onSubmit={(e) => { e.preventDefault(); run(async () => {
          const r = await api.tgSubmitPassword(f.password);
          if (r.step === "done") await finishLogin(r.name);
        }); }}>
          <label>Your Telegram cloud password{hint ? ` (hint: ${hint})` : ""}
            <input type="password" autoComplete="current-password" value={f.password} onChange={set("password")} autoFocus />
          </label>
          <div><button className="primary" disabled={busy || !f.password}>Connect</button></div>
          <p className="small faint">Sent to Telegram to finish logging in. Veil never stores it.</p>
        </form>
      )}

      {error && <div className="error">{error}</div>}
    </div>
  );
}

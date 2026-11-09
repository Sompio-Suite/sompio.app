import { useEffect, useState } from "react";
import { api, onFileDrop, passwordProblem, pickPicture, pickPictureSave, type LockStatus } from "../api";
import Icon from "../Icon";

interface Props {
  status: LockStatus;
  onUnlocked: () => void;
}

type Mode = "setup" | "unlock" | "recover";
type SetupStep = "intro" | "password" | "sheet" | "phone";

function Mark({ size = 56 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 120 120" aria-hidden className="lock-mark">
      <path className="sun-top" d="M14 56 A36 36 0 0 1 86 56 Z" />
      <path className="sun-bottom" d="M34 64 A36 36 0 0 0 106 64 Z" />
    </svg>
  );
}

function PasswordFields(p: { pw: string; setPw: (s: string) => void; again: string; setAgain: (s: string) => void; label?: string }) {
  const problem = p.pw ? passwordProblem(p.pw) : null;
  const mismatch = p.again.length > 0 && p.again !== p.pw;
  return (
    <>
      <label className="field">
        <span>{p.label ?? "Master password"}</span>
        <input type="password" autoComplete="new-password" value={p.pw} onChange={(e) => p.setPw(e.target.value)} autoFocus />
        <small className={problem ? "warn-text" : "muted"}>{problem ?? "At least 10 characters. A short sentence or a few unrelated words is easiest to remember."}</small>
      </label>
      <label className="field">
        <span>Type it again</span>
        <input type="password" autoComplete="new-password" value={p.again} onChange={(e) => p.setAgain(e.target.value)} />
        {mismatch && <small className="warn-text">These don't match yet.</small>}
      </label>
    </>
  );
}

function RecoverySheet({ code, onDone }: { code: string; onDone: () => void }) {
  const [kept, setKept] = useState(false);
  return (
    <div className="lock-step">
      <h1>Your recovery code</h1>
      <p className="muted">
        If you ever forget your password or lose the key picture, this code is the only way back in. Nobody can reset it for you, not even
        us. Print it or write it down, and keep it somewhere safe at home, away from this PC.
      </p>
      <div className="recovery-sheet" id="recovery-sheet">
        <div className="sheet-head"><Mark size={28} /><strong>Sompio recovery sheet</strong></div>
        <div className="recovery-code selectable">{code}</div>
        <p className="small">Use it in Sompio under "Use your recovery code". Letters and numbers only; dashes and capitals don't matter.</p>
        <p className="small sheet-date">Made {new Date().toLocaleDateString()}</p>
      </div>
      <div className="toolbar">
        <button onClick={() => window.print()}><Icon name="copy" size={16} />Print recovery sheet</button>
      </div>
      <label className="check">
        <input type="checkbox" checked={kept} onChange={(e) => setKept(e.target.checked)} />
        I've printed it or written it down, and I'll keep it safe.
      </label>
      <button className="primary big" disabled={!kept} onClick={onDone}>Continue</button>
    </div>
  );
}

function PhoneStep({ path, onDone }: { path: string; onDone: () => void }) {
  return (
    <div className="lock-step">
      <h1>Put the key picture on your phone</h1>
      <p className="muted">Your key picture is saved at:</p>
      <code className="path selectable">{path}</code>
      <ul className="plan-list">
        <li><Icon name="check" size={16} />Move it to your phone <strong>as a file</strong>: by cable, AirDrop, Nearby Share, or "Save to Files".</li>
        <li><Icon name="check" size={16} />Don't send it through WhatsApp, Messenger or similar. They shrink pictures, which erases the key inside.</li>
        <li><Icon name="check" size={16} />It looks like an ordinary night-sky wallpaper. Keep it with your photos; nobody needs to know what it is.</li>
        <li><Icon name="check" size={16} />Once it's on your phone, you can delete it from this PC.</li>
      </ul>
      <button className="primary big" onClick={onDone}>Open Sompio</button>
    </div>
  );
}

export default function LockScreen({ status, onUnlocked }: Props) {
  const [mode, setMode] = useState<Mode>(status.set_up ? "unlock" : "setup");
  const [step, setStep] = useState<SetupStep>("intro");
  const [pw, setPw] = useState("");
  const [again, setAgain] = useState("");
  const [code, setCode] = useState("");
  const [picture, setPicture] = useState<string | null>(null);
  const [remember, setRemember] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [newCode, setNewCode] = useState<string | null>(null);
  const [savedTo, setSavedTo] = useState<string | null>(null);
  const [over, setOver] = useState(false);

  useEffect(() => {
    const un = onFileDrop((p) => p.toLowerCase().endsWith(".png") && setPicture(p), setOver);
    return () => void un.then((f) => f());
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

  const ready = !passwordProblem(pw) && pw === again;

  const createLock = () =>
    run(async () => {
      const path = await pickPictureSave();
      if (!path) return;
      const c = await api.lockSetup(pw, path, remember);
      setPw("");
      setAgain("");
      setSavedTo(path);
      setNewCode(c);
      setStep("sheet");
    });

  const unlock = () =>
    run(async () => {
      await api.lockUnlock(pw, picture, remember);
      setPw("");
      onUnlocked();
    });

  const recover = () =>
    run(async () => {
      const path = await pickPictureSave();
      if (!path) return;
      const c = await api.lockRecover(code, pw, path, remember);
      setPw("");
      setAgain("");
      setCode("");
      setSavedTo(path);
      setNewCode(c);
      setStep("sheet");
    });

  // After setup or recovery: the sheet, then the phone step
  if (newCode && step === "sheet") return <div className="lock-screen"><RecoverySheet code={newCode} onDone={() => setStep("phone")} /></div>;
  if (newCode && step === "phone" && savedTo) {
    return <div className="lock-screen"><PhoneStep path={savedTo} onDone={() => { setNewCode(null); onUnlocked(); }} /></div>;
  }

  const rememberBox = (
    <label className="check">
      <input type="checkbox" checked={remember} onChange={(e) => setRemember(e.target.checked)} />
      <span>
        Remember the key picture on this PC
        <small className="muted"> Then you only type your password here. Leave it off on a shared or work PC.</small>
      </span>
    </label>
  );

  return (
    <div className={`lock-screen${over ? " over" : ""}`}>
      {mode === "setup" && step === "intro" && (
        <div className="lock-step">
          <Mark />
          <h1>Lock Sompio before you start</h1>
          <p className="muted">Everything Sompio keeps is encrypted on this PC. Two things open it, and both are needed:</p>
          <div className="two-keys">
            <div><Icon name="key" /><strong>A password you know</strong><span>Typed each time you open Sompio.</span></div>
            <div><Icon name="eye" /><strong>A key picture you carry</strong><span>A night-sky picture with a secret hidden in it. It lives on your phone like any other photo.</span></div>
          </div>
          <p className="muted small">You'll also get a recovery code to print. Without the password and the picture, or the code, nobody can open your data, including us.</p>
          <button className="primary big" onClick={() => setStep("password")}>Set up the lock</button>
        </div>
      )}

      {mode === "setup" && step === "password" && (
        <form className="lock-step" onSubmit={(e) => { e.preventDefault(); if (ready) createLock(); }}>
          <h1>Choose your master password</h1>
          <PasswordFields pw={pw} setPw={setPw} again={again} setAgain={setAgain} />
          {rememberBox}
          {error && <div className="error">{error}</div>}
          <div className="toolbar">
            <button className="primary big" disabled={!ready || busy}>{busy ? "Creating the lock…" : "Save my key picture…"}</button>
            <button type="button" className="quiet" onClick={() => setStep("intro")}>Back</button>
          </div>
          <p className="small muted">Next you choose where to save the key picture, then you get your recovery code.</p>
        </form>
      )}

      {mode === "unlock" && (
        <form className="lock-step narrow" onSubmit={(e) => { e.preventDefault(); if (pw) unlock(); }}>
          <Mark />
          <h1>Sompio is locked</h1>
          <label className="field">
            <span>Master password</span>
            <input type="password" autoComplete="current-password" value={pw} onChange={(e) => setPw(e.target.value)} autoFocus />
          </label>
          {!status.remembers_picture && (
            <div className={`picture-pick${picture ? " done" : ""}`}>
              <Icon name={picture ? "check" : "eye"} />
              <div>
                <strong>{picture ? "Key picture ready" : "Your key picture"}</strong>
                <span className="small muted">{picture ? picture.split(/[\\/]/).pop() : "Choose it, or drop it here. Use the original file from your phone."}</span>
              </div>
              <button type="button" onClick={async () => setPicture(await pickPicture())}>{picture ? "Change" : "Choose…"}</button>
            </div>
          )}
          {!status.remembers_picture && picture && rememberBox}
          {error && <div className="error">{error}</div>}
          <button className="primary big" disabled={busy || !pw || (!status.remembers_picture && !picture)}>{busy ? "Unlocking…" : "Unlock"}</button>
          <button type="button" className="link" onClick={() => { setMode("recover"); setError(null); setPw(""); }}>
            Forgot your password or lost the picture?
          </button>
        </form>
      )}

      {mode === "recover" && (
        <form className="lock-step" onSubmit={(e) => { e.preventDefault(); if (ready && code) recover(); }}>
          <h1>Use your recovery code</h1>
          <p className="muted">Your data stays as it is. You choose a new password and get a new key picture and recovery code; the old ones stop working.</p>
          <label className="field">
            <span>Recovery code</span>
            <input type="text" className="mono" autoComplete="off" spellCheck={false} placeholder="XXXX-XXXX-XXXX-XXXX-XXXX-XXXX" value={code} onChange={(e) => setCode(e.target.value)} autoFocus />
          </label>
          <PasswordFields pw={pw} setPw={setPw} again={again} setAgain={setAgain} label="New master password" />
          {rememberBox}
          {error && <div className="error">{error}</div>}
          <div className="toolbar">
            <button className="primary big" disabled={!ready || !code || busy}>{busy ? "Checking…" : "Save my new key picture…"}</button>
            <button type="button" className="quiet" onClick={() => { setMode("unlock"); setError(null); }}>Back</button>
          </div>
        </form>
      )}
    </div>
  );
}

import { useEffect, useState } from "react";
import { api, listen, onFileDrop, passwordProblem, pickPicture, pickPictureSave, type LockStatus } from "../api";
import Icon from "../Icon";

interface Props {
  status: LockStatus;
  onUnlocked: () => void;
}

type Mode = "setup" | "unlock" | "recover";
type SetupStep = "intro" | "rules" | "picture" | "password" | "sheet" | "phone";

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
      <p className="step-count">Last step</p>
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
  const [understood, setUnderstood] = useState(false);
  const [madePicture, setMadePicture] = useState<string | null>(null);
  const hasKeys = status.hardware_keys.length > 0;
  // With a YubiKey added and no remembered picture, the YubiKey is the usual way in
  const [useKey, setUseKey] = useState(hasKeys && !status.remembers_picture);
  const [touch, setTouch] = useState(false);
  useEffect(() => {
    const un = listen("yubikey-touch", () => setTouch(true));
    return () => void un.then((f) => f());
  }, []);

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

  const makePicture = () =>
    run(async () => {
      const path = await pickPictureSave();
      if (!path) return;
      await api.lockNewPicture(path);
      setMadePicture(path);
    });

  const createLock = () =>
    run(async () => {
      const c = await api.lockSetupWithPicture(pw, remember);
      setPw("");
      setAgain("");
      setSavedTo(madePicture);
      setNewCode(c);
      setStep("sheet");
    });

  const unlock = () =>
    run(async () => {
      try {
        if (useKey) await api.lockUnlockYubikey(pw);
        else await api.lockUnlock(pw, picture, remember);
      } finally {
        setTouch(false);
      }
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
  if (newCode && step === "sheet") {
    return <div className="lock-screen"><RecoverySheet code={newCode} onDone={() => (mode === "setup" ? (setNewCode(null), onUnlocked()) : setStep("phone"))} /></div>;
  }
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
          <h1>Welcome to Sompio</h1>
          <p className="muted">
            Sompio keeps everything on this PC, encrypted: your passwords, your calendar and what Clean finds. Nothing goes to us or
            to the cloud. Before you start, you set up the lock that keeps it safe. It takes about two minutes.
          </p>
          <ol className="setup-steps">
            <li><span>1</span><div><strong>Read how the lock works</strong><small className="muted">A few rules, so you never lock yourself out.</small></div></li>
            <li><span>2</span><div><strong>Make your key picture</strong><small className="muted">A .png file with a secret hidden in it. You save it on your PC.</small></div></li>
            <li><span>3</span><div><strong>Choose your password</strong><small className="muted">The password and the picture together open Sompio.</small></div></li>
            <li><span>4</span><div><strong>Keep your recovery code</strong><small className="muted">Your way back in if you lose the other two.</small></div></li>
          </ol>
          <button className="primary big" onClick={() => setStep("rules")}>Get started</button>
        </div>
      )}

      {mode === "setup" && step === "rules" && (
        <div className="lock-step wide">
          <p className="step-count">Step 1 of 4</p>
          <h1>How the lock works</h1>
          <p className="muted">
            Two things open Sompio, and both are needed: <strong>your password</strong> (in your head) and <strong>your key picture</strong> (a
            file). Someone who finds only one of them gets nothing.
          </p>
          <div className="rules">
            <div className="rules-col do">
              <h3><Icon name="check" size={16} />Do</h3>
              <ul>
                <li>Keep a copy of the key picture somewhere other than this PC: a USB stick, or your phone <em>as a file</em> (cable, AirDrop, "Save to Files").</li>
                <li>Print or write down the recovery code you get at the end, and keep it at home, away from the PC.</li>
                <li>Use a password you don't use anywhere else. A few unrelated words is easy to remember and hard to guess.</li>
                <li>On a shared or work PC, don't let Sompio remember the picture. Then the file is needed every time.</li>
              </ul>
            </div>
            <div className="rules-col dont">
              <h3><Icon name="plus" size={16} />Don't</h3>
              <ul>
                <li>Don't edit, crop, resize, rename to .jpg or screenshot the key picture. Changing a single pixel breaks the key.</li>
                <li>Don't send it through WhatsApp, Messenger, Discord or e-mail apps that shrink photos. They erase the key.</li>
                <li>Don't keep your password written next to the key picture or in the same folder.</li>
                <li>Don't share the picture, the password or the recovery code with anyone. Sompio support will never ask for them.</li>
              </ul>
            </div>
          </div>
          <div className="rules-warn">
            <strong>Nobody can reset it for you, not even us.</strong> If you lose the password or the picture, the recovery code is the only way
            back in. Lose all three and your data can't be opened by anyone.
          </div>
          <label className="check">
            <input type="checkbox" checked={understood} onChange={(e) => setUnderstood(e.target.checked)} />
            I understand how the lock works and that nobody can reset it for me.
          </label>
          <div className="toolbar">
            <button className="primary big" disabled={!understood} onClick={() => setStep("picture")}>Continue</button>
            <button type="button" className="quiet" onClick={() => setStep("intro")}>Back</button>
          </div>
        </div>
      )}

      {mode === "setup" && step === "picture" && (
        <div className="lock-step">
          <p className="step-count">Step 2 of 4</p>
          <h1>Make your key picture</h1>
          <p className="muted">
            Sompio makes a night-sky picture with a random secret hidden in its pixels, and you choose where to save it. It looks like any
            wallpaper, so nobody needs to know what it is.
          </p>
          <div className={`picture-pick big${madePicture ? " done" : ""}`}>
            <Icon name={madePicture ? "check" : "eye"} />
            <div>
              <strong>{madePicture ? "Key picture saved and checked" : "No key picture yet"}</strong>
              <span className="small muted selectable">{madePicture ?? "Pictures or Documents is a good place. Copy it to a USB stick or your phone afterwards."}</span>
            </div>
          </div>
          {error && <div className="error">{error}</div>}
          <div className="toolbar">
            {madePicture ? (
              <>
                <button className="primary big" onClick={() => { setError(null); setStep("password"); }}>Continue</button>
                <button type="button" className="quiet" disabled={busy} onClick={makePicture}>Make a new one instead…</button>
              </>
            ) : (
              <button className="primary big" disabled={busy} onClick={makePicture}>{busy ? "Making the picture…" : "Generate my key picture (.png)…"}</button>
            )}
            <button type="button" className="quiet" onClick={() => setStep("rules")}>Back</button>
          </div>
          {madePicture && <p className="small muted">Before you go on, copy the file to a USB stick or your phone, as a file. You can also do it later.</p>}
        </div>
      )}

      {mode === "setup" && step === "password" && (
        <form className="lock-step" onSubmit={(e) => { e.preventDefault(); if (ready) createLock(); }}>
          <p className="step-count">Step 3 of 4</p>
          <h1>Choose your master password</h1>
          <p className="muted">You type this each time you open Sompio. Together with the key picture, it opens everything.</p>
          <PasswordFields pw={pw} setPw={setPw} again={again} setAgain={setAgain} />
          {rememberBox}
          {error && <div className="error">{error}</div>}
          <div className="toolbar">
            <button className="primary big" disabled={!ready || busy}>{busy ? "Creating the lock…" : "Create the lock"}</button>
            <button type="button" className="quiet" onClick={() => setStep("picture")}>Back</button>
          </div>
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
          {hasKeys && (
            <div className="segmented" role="radiogroup" aria-label="Second key">
              <button type="button" role="radio" aria-checked={useKey} aria-selected={useKey} onClick={() => setUseKey(true)}>YubiKey</button>
              <button type="button" role="radio" aria-checked={!useKey} aria-selected={!useKey} onClick={() => setUseKey(false)}>Key picture</button>
            </div>
          )}
          {useKey && (
            <div className={`picture-pick${touch ? " done" : ""}`}>
              <Icon name="key" />
              <div>
                <strong>{touch ? "Touch your YubiKey now" : "Plug in your YubiKey"}</strong>
                <span className="small muted">{touch ? "It's blinking: touch the gold disc." : status.hardware_keys.map((k) => k.label).join(", ")}</span>
              </div>
            </div>
          )}
          {!useKey && !status.remembers_picture && (
            <div className={`picture-pick${picture ? " done" : ""}`}>
              <Icon name={picture ? "check" : "eye"} />
              <div>
                <strong>{picture ? "Key picture ready" : "Your key picture"}</strong>
                <span className="small muted">{picture ? picture.split(/[\\/]/).pop() : "Choose it, or drop it here. Use the original file from your phone."}</span>
              </div>
              <button type="button" onClick={async () => setPicture(await pickPicture())}>{picture ? "Change" : "Choose…"}</button>
            </div>
          )}
          {!useKey && !status.remembers_picture && picture && rememberBox}
          {error && <div className="error">{error}</div>}
          <button className="primary big" disabled={busy || !pw || (!useKey && !status.remembers_picture && !picture)}>{busy ? (useKey ? "Asking the YubiKey…" : "Unlocking…") : "Unlock"}</button>
          <button type="button" className="link" onClick={() => { setMode("recover"); setError(null); setPw(""); }}>
            {hasKeys ? "Forgot your password, or lost the picture and YubiKey?" : "Forgot your password or lost the picture?"}
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

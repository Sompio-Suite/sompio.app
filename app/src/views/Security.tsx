import { useState } from "react";
import { api, passwordProblem, pickPicture, type LockStatus } from "../api";
import Icon from "../Icon";

interface Props {
  status: LockStatus;
  idleMinutes: number;
  onIdleMinutes: (m: number) => void;
  onLockNow: () => void;
  onChanged: () => void;
}

export default function Security({ status, idleMinutes, onIdleMinutes, onLockNow, onChanged }: Props) {
  const [old, setOld] = useState("");
  const [pw, setPw] = useState("");
  const [again, setAgain] = useState("");
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<{ ok: boolean; text: string } | null>(null);
  const problem = pw ? passwordProblem(pw) : null;

  const change = async () => {
    setBusy(true);
    setNote(null);
    try {
      const picture = status.remembers_picture ? null : await pickPicture();
      if (!status.remembers_picture && !picture) return;
      await api.lockChangePassword(old, pw, picture);
      setOld("");
      setPw("");
      setAgain("");
      setNote({ ok: true, text: "Password changed. Your key picture and recovery code stay the same." });
    } catch (e) {
      setNote({ ok: false, text: String(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>Security</h1>
          <p className="muted">Everything Sompio keeps is encrypted with your lock. It opens with your password and key picture.</p>
        </div>
        <button onClick={onLockNow}><Icon name="lock" size={16} />Lock now</button>
      </div>

      <section className="setting">
        <h3>Lock when I'm away</h3>
        <p className="muted">Sompio locks itself after this long without you using it.</p>
        <select value={idleMinutes} onChange={(e) => onIdleMinutes(Number(e.target.value))}>
          {[5, 15, 30, 60].map((m) => <option key={m} value={m}>After {m} minutes</option>)}
          <option value={0}>Only when I close it</option>
        </select>
      </section>

      <section className="setting">
        <h3>Key picture on this PC</h3>
        {status.remembers_picture ? (
          <>
            <p className="muted">This PC remembers your key picture, so you only type your password here. Forget it on a PC you share or will sell.</p>
            <div className="toolbar">
              <button onClick={async () => { await api.lockForgetPicture(); onChanged(); setNote({ ok: true, text: "This PC no longer remembers the key picture. You'll need it next time you unlock." }); }}>
                Forget the picture on this PC
              </button>
            </div>
          </>
        ) : (
          <p className="muted">This PC doesn't remember your key picture. You'll choose it each time you unlock, and can tick "Remember" then.</p>
        )}
      </section>

      <section className="setting">
        <h3>Change your master password</h3>
        <form className="stack-form" onSubmit={(e) => { e.preventDefault(); change(); }}>
          <label className="field"><span>Current password</span><input type="password" autoComplete="current-password" value={old} onChange={(e) => setOld(e.target.value)} /></label>
          <label className="field">
            <span>New password</span>
            <input type="password" autoComplete="new-password" value={pw} onChange={(e) => setPw(e.target.value)} />
            {problem && <small className="warn-text">{problem}</small>}
          </label>
          <label className="field">
            <span>New password again</span>
            <input type="password" autoComplete="new-password" value={again} onChange={(e) => setAgain(e.target.value)} />
            {again && again !== pw && <small className="warn-text">These don't match yet.</small>}
          </label>
          <div className="toolbar">
            <button className="primary" disabled={busy || !old || !!problem || !pw || pw !== again}>
              {busy ? "Changing…" : status.remembers_picture ? "Change password" : "Choose key picture and change"}
            </button>
          </div>
        </form>
      </section>

      {note && <div className={note.ok ? "success" : "error"} role="status">{note.text}</div>}

      <section className="setting">
        <h3>Coming to the lock</h3>
        <p className="muted">YubiKey as the second key instead of the picture, and Windows Hello for quick unlocking on this PC.</p>
      </section>
    </div>
  );
}

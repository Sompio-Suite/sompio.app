import { useEffect, useState } from "react";
import { api, listen, passwordProblem, pickPicture, type LockStatus } from "../api";
import Icon from "../Icon";
import { autoCheckEnabled, checkForUpdate, currentVersion, setAutoCheck, updatesOn, type UpdateInfo } from "../updates";

interface Props {
  status: LockStatus;
  idleMinutes: number;
  onIdleMinutes: (m: number) => void;
  onLockNow: () => void;
  onChanged: () => void;
  onUpdateFound: (u: UpdateInfo | null) => void;
}

export default function Security({ status, idleMinutes, onIdleMinutes, onLockNow, onChanged, onUpdateFound }: Props) {
  const [version, setVersion] = useState("");
  const [auto, setAuto] = useState(autoCheckEnabled());
  const [updateNote, setUpdateNote] = useState<string | null>(null);
  const [canUpdate, setCanUpdate] = useState(true);
  useEffect(() => {
    currentVersion().then(setVersion).catch(() => {});
    updatesOn().then(setCanUpdate);
  }, []);
  const checkNow = async () => {
    setUpdateNote("Checking…");
    try {
      const u = await checkForUpdate();
      onUpdateFound(u);
      setUpdateNote(u ? `Sompio ${u.version} is available. Use "Update now" in the sidebar.` : "You have the latest version.");
    } catch {
      setUpdateNote("Couldn't reach the update server. Check your internet connection and try again.");
    }
  };
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
      if (!status.remembers_picture && status.hardware_keys.length > 0) {
        await api.lockChangePasswordYubikey(old, pw);
      } else {
        const picture = status.remembers_picture ? null : await pickPicture();
        if (!status.remembers_picture && !picture) return;
        await api.lockChangePassword(old, pw, picture);
      }
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
              {busy ? "Changing…" : status.remembers_picture ? "Change password" : status.hardware_keys.length > 0 ? "Change with my YubiKey" : "Choose key picture and change"}
            </button>
          </div>
        </form>
      </section>

      {note && <div className={note.ok ? "success" : "error"} role="status">{note.text}</div>}

      <section className="setting">
        <h3>Updates</h3>
        {canUpdate ? (
          <>
            <p className="muted">
              You have Sompio {version}. To find out about new versions, Sompio downloads one small version file every few
              hours. Nothing about you or your data is sent. Updates are signed, and Sompio only installs ones signed by us.
            </p>
            <label className="check">
              <input type="checkbox" checked={auto} onChange={(e) => { setAuto(e.target.checked); setAutoCheck(e.target.checked); }} />
              Check for updates automatically
            </label>
            <div className="toolbar">
              <button onClick={checkNow}>Check now</button>
              {updateNote && <span className="small muted">{updateNote}</span>}
            </div>
          </>
        ) : (
          <p className="muted">
            You have Sompio {version}, a test version. It doesn't look for updates or go online for them. Newer test
            versions come from whoever gave you this one: install them over this one and your data stays.
          </p>
        )}
      </section>

      <YubiKeys status={status} onChanged={onChanged} />

      <ReportProblem />

      <section className="setting">
        <h3>Coming to the lock</h3>
        <p className="muted">Windows Hello for quick unlocking on this PC.</p>
      </section>
    </div>
  );
}

function YubiKeys({ status, onChanged }: { status: LockStatus; onChanged: () => void }) {
  const [present, setPresent] = useState<{ name: string; serial: number | null }[] | null>(null);
  const [adding, setAdding] = useState(false);
  const [slot, setSlot] = useState(2);
  const [label, setLabel] = useState("");
  const [busy, setBusy] = useState(false);
  const [touch, setTouch] = useState(false);
  const [note, setNote] = useState<{ ok: boolean; text: string } | null>(null);
  const [help, setHelp] = useState(false);
  useEffect(() => {
    const un = listen("yubikey-touch", () => setTouch(true));
    return () => void un.then((f) => f());
  }, []);
  const look = () => api.lockYubikeysPresent().then(setPresent).catch((e) => setNote({ ok: false, text: String(e) }));
  const add = async () => {
    setBusy(true);
    setNote(null);
    try {
      const picture = status.remembers_picture ? null : await pickPicture();
      if (!status.remembers_picture && !picture) return;
      await api.lockAddYubikey(picture, slot, label);
      setAdding(false);
      setLabel("");
      onChanged();
      setNote({ ok: true, text: "YubiKey added. Next time you unlock, choose YubiKey and touch it. Add a spare key too, in case you lose this one." });
    } catch (e) {
      setNote({ ok: false, text: String(e) });
    } finally {
      setBusy(false);
      setTouch(false);
    }
  };
  return (
    <section className="setting">
      <h3>YubiKey</h3>
      <p className="muted">
        A YubiKey can stand in for your key picture: password plus a touch of the key. Your picture and recovery code keep working
        too. Add two keys if you can, so a lost one doesn't lock you out.
      </p>
      {status.hardware_keys.length > 0 && (
        <ul className="key-list">
          {status.hardware_keys.map((k) => (
            <li key={k.index}>
              <Icon name="key" size={16} />
              <span className="grow"><strong>{k.label}</strong><span className="small muted">Slot {k.slot}{k.serial ? ` · serial ${k.serial}` : ""} · added {new Date(k.added_ms).toLocaleDateString()}</span></span>
              <button className="small-btn ghost danger" onClick={async () => {
                try { await api.lockRemoveYubikey(k.index); onChanged(); } catch (e) { setNote({ ok: false, text: String(e) }); }
              }}>Remove</button>
            </li>
          ))}
        </ul>
      )}
      {!adding ? (
        <div className="toolbar"><button onClick={() => { setAdding(true); look(); }}><Icon name="plus" size={16} />Add a YubiKey</button></div>
      ) : (
        <div className="panel inset-form">
          <span className="small">
            {present === null ? "Looking for YubiKeys…" : present.length === 0 ? "No YubiKey found. Plug it in, then look again." : `Found: ${present.map((p) => p.name + (p.serial ? ` (${p.serial})` : "")).join(", ")}`}
            {" "}<button type="button" className="link" onClick={look}>Look again</button>
          </span>
          <div className="field-row">
            <label className="field grow"><span>Name it</span><input type="text" value={label} onChange={(e) => setLabel(e.target.value)} placeholder="Keyring key, Spare in the drawer…" /></label>
            <label className="field"><span>Slot</span>
              <select value={slot} onChange={(e) => setSlot(Number(e.target.value))}><option value={2}>2 (long touch)</option><option value={1}>1 (short touch)</option></select>
            </label>
          </div>
          {touch && <div className="notice"><Icon name="key" /><div><strong>Touch your YubiKey now</strong><span>It's blinking. You may be asked twice.</span></div></div>}
          <div className="toolbar">
            <button className="primary" disabled={busy || !present?.length} onClick={add}>{busy ? "Asking the YubiKey…" : status.remembers_picture ? "Add this YubiKey" : "Choose key picture and add"}</button>
            <button className="quiet" onClick={() => setAdding(false)}>Cancel</button>
            <button type="button" className="link" onClick={() => setHelp(!help)}>{help ? "Hide" : "How do I set up the slot?"}</button>
          </div>
          {help && (
            <ol className="small muted help-steps">
              <li>The slot needs HMAC-SHA1 challenge-response, like KeePassXC uses. Keys from Sompio come set up already.</li>
              <li>To set one up yourself, install Yubico's free YubiKey Manager and open Applications, then OTP.</li>
              <li>Pick slot 2 (Long Touch), choose Challenge-response, Generate a secret, tick Require touch, and Finish.</li>
              <li>This replaces whatever was in that slot. If you already use slot 2 for KeePassXC, you can keep it: Sompio works with the same setup.</li>
            </ol>
          )}
        </div>
      )}
      {note && <div className={note.ok ? "success" : "error"} role="status">{note.text}</div>}
    </section>
  );
}

/** Problems are written to a local file only. Nothing is sent anywhere: the person decides what to share. */
function ReportProblem() {
  const [note, setNote] = useState<{ ok: boolean; text: string } | null>(null);
  const [manual, setManual] = useState<string | null>(null);
  const copy = async () => {
    setManual(null);
    let text = "";
    try {
      const d = await api.appDetails();
      text = [
        `Sompio ${d.version}`,
        `System: ${d.os}`,
        "",
        "What I did:",
        "",
        "What happened:",
        "",
        "Last lines of the problems log:",
        ...(d.recent.length ? d.recent : ["(empty)"]),
      ].join("\n");
    } catch (e) {
      setNote({ ok: false, text: String(e) });
      return;
    }
    try {
      await navigator.clipboard.writeText(text);
      setNote({ ok: true, text: "Copied. Paste it into your message and fill in what you did and what happened." });
    } catch {
      // Clipboard not available: show it to copy by hand
      setNote(null);
      setManual(text);
    }
  };
  return (
    <section className="setting">
      <h3>Something wrong?</h3>
      <p className="muted">
        When something goes wrong, Sompio writes a short line about it to a problems log on this PC. It holds error
        messages only, never your passwords, messages or files, and nothing is sent anywhere. To report a problem, copy
        the details below and send them along with what you were doing.
      </p>
      <div className="toolbar">
        <button onClick={copy}>Copy details for a bug report</button>
        <button className="quiet" onClick={() => api.openProblemsFolder().catch((e) => setNote({ ok: false, text: String(e) }))}>Open the problems log</button>
      </div>
      {note && <div className={note.ok ? "success" : "error"} role="status">{note.text}</div>}
      {manual && (
        <label className="field">
          <span>Select all of this (Ctrl+A, Ctrl+C) and paste it into your message</span>
          <textarea readOnly rows={8} className="mono small" value={manual} onFocus={(e) => e.currentTarget.select()} />
        </label>
      )}
    </section>
  );
}

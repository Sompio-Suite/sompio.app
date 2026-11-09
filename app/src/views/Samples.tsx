import { useState } from "react";
import { api } from "../api";
import Icon from "../Icon";
import type { View } from "../App";

interface Props {
  cleanHasData: boolean;
  onCleanDemo: () => Promise<void>;
  onGo: (v: View) => void;
}

type Written = { folder: string; files: [string, string][] };

/** Everything a tester needs: sample data inside each app, and sample files for every import. */
export default function Samples({ cleanHasData, onCleanDemo, onGo }: Props) {
  const [written, setWritten] = useState<Record<string, Written>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [note, setNote] = useState<{ ok: boolean; text: string } | null>(null);

  const run = async (key: string, f: () => Promise<string | void>) => {
    setBusy(key);
    setNote(null);
    try {
      const text = await f();
      if (text) setNote({ ok: true, text });
    } catch (e) {
      setNote({ ok: false, text: String(e) });
    } finally {
      setBusy(null);
    }
  };
  const files = (which: "clean" | "calendar" | "vault") =>
    run(`${which}-files`, async () => {
      const w = await api.samplesWrite(which);
      setWritten({ ...written, [which]: w });
    });

  const fileList = (which: string) =>
    written[which] && (
      <div className="sample-files">
        <span className="small">Saved to <span className="mono">{written[which].folder}</span> (it opened for you).</span>
        <ul className="small">
          {written[which].files.map(([f, what]) => <li key={f}><strong>{f}</strong> <span className="muted">{what}</span></li>)}
        </ul>
      </div>
    );

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>Samples</h1>
          <p className="muted">
            Made-up data to try every part of Sompio without using your own. The people, numbers and passwords in them are
            fictional. Sample data inside the apps is clearly marked and you can remove it in one go.
          </p>
        </div>
      </div>
      {note && <div className={note.ok ? "success" : "error"} role="status">{note.text}</div>}

      <section className="panel sample-card">
        <header><Icon name="review" /><h2>Clean</h2></header>
        <div className="sample-row">
          <div>
            <strong>Look around with sample data</strong>
            <span className="small muted">A ready-scanned set of Discord messages: review, group, mark and delete without touching anything real. "Use my own data" in the bar at the top removes it.</span>
          </div>
          {cleanHasData ? (
            <span className="small faint">You already have your own data in Clean, so this is off. Use the sample files instead.</span>
          ) : (
            <button className="primary" disabled={!!busy} onClick={() => run("clean-demo", onCleanDemo)}>Open Clean with sample data</button>
          )}
        </div>
        <div className="sample-row">
          <div>
            <strong>Sample exports for every import</strong>
            <span className="small muted">Discord, Reddit, Telegram, X, Facebook, Instagram, TikTok, Snapchat, LinkedIn, WhatsApp, Gmail, and a folder for "Scan a folder on this PC". Import them under Clean, Sources.</span>
          </div>
          <button disabled={!!busy} onClick={() => files("clean")}>{busy === "clean-files" ? "Saving…" : "Save sample exports"}</button>
        </div>
        {fileList("clean")}
      </section>

      <section className="panel sample-card">
        <header><Icon name="calendar" /><h2>Calendar</h2></header>
        <div className="sample-row">
          <div>
            <strong>Sample reminders</strong>
            <span className="small muted">A schedule called "Sample reminders": the first pop-up comes within a minute, then a heads-up, a reminder only for your group, and one every 5 minutes. Remove the schedule to undo it all.</span>
          </div>
          <button className="primary" disabled={!!busy} onClick={() => run("cal", async () => { await api.calAddSamples(); onGo("calendar"); })}>Add sample reminders</button>
        </div>
        <div className="sample-row">
          <div>
            <strong>Sample files</strong>
            <span className="small muted">A shared schedule with groups and a time zone, and a Desk Reminders folder to bring in.</span>
          </div>
          <button disabled={!!busy} onClick={() => files("calendar")}>{busy === "calendar-files" ? "Saving…" : "Save sample files"}</button>
        </div>
        {fileList("calendar")}
      </section>

      <section className="panel sample-card">
        <header><Icon name="key" /><h2>Vault</h2></header>
        <div className="sample-row">
          <div>
            <strong>Sample entries</strong>
            <span className="small muted">A "Samples" folder with 2FA codes, hidden extra fields, an expired password, weak and reused ones for the health check, earlier versions, and two auto-type tests (Notepad and a test login page). Delete the folder to remove them.</span>
          </div>
          <button className="primary" disabled={!!busy} onClick={() => run("vault", async () => { await api.vaultAddSamples(); onGo("vault"); })}>Add sample entries</button>
        </div>
        <div className="sample-row">
          <div>
            <strong>Sample files</strong>
            <span className="small muted">Two KeePass files (password: <span className="mono">sample-password</span>, one also needs the key file next to it), CSV exports from Bitwarden, Chrome, Firefox, 1Password and KeePassXC, and a test login page for auto-type.</span>
          </div>
          <button disabled={!!busy} onClick={() => files("vault")}>{busy === "vault-files" ? "Saving…" : "Save sample files"}</button>
        </div>
        {fileList("vault")}
      </section>

      <section className="panel sample-card">
        <header><Icon name="mic" /><h2>Voice</h2></header>
        <div className="sample-row">
          <div>
            <strong>A test server on this PC</strong>
            <span className="small muted">On the Voice page, "Start a server", then "Join it", and use the admin key it shows to try channels, passwords, roles, kick and ban. To hear each other, a second PC on your network joins the address it shows.</span>
          </div>
          <button onClick={() => onGo("voice")}>Open Voice</button>
        </div>
      </section>
    </div>
  );
}

import { useCallback, useEffect, useRef, useState } from "react";
import {
  api, listen, pickOpen, pickSave,
  type HealthReport, type VaultLeak, type PassphraseRules, type PasswordRules, type Strength, type VaultSettings,
} from "../api";
import Icon from "../Icon";
import { plural } from "../format";
import { reloadIcons, useIcon } from "../vaultIcons";

export function StrengthMeter({ s }: { s: Strength | null }) {
  if (!s) return null;
  return (
    <div className={`meter s${s.score}`} aria-label={`Strength: ${s.label}`}>
      <div className="meter-bar">{[0, 1, 2, 3, 4].map((i) => <span key={i} className={i <= s.score ? "on" : ""} />)}</div>
      <span className="small"><strong>{s.label}</strong> · {Math.round(s.bits)} bits · cracked in {s.crack_time}</span>
      {s.warning && <span className="small warn-text">{s.warning}</span>}
    </div>
  );
}

const PW_DEFAULT: PasswordRules = { length: 20, lower: true, upper: true, digits: true, symbols: true, no_lookalikes: true, every_kind: true, exclude: "" };
const PP_DEFAULT: PassphraseRules = { words: 6, separator: "-", capitalize: false, number: false };

function load<T>(key: string, fallback: T): T {
  try {
    const v = localStorage.getItem(key);
    return v ? { ...fallback, ...JSON.parse(v) } : fallback;
  } catch {
    return fallback;
  }
}

/** Passwords and passphrases. `onUse` puts the result into the entry being edited. */
export function Generator({ onUse, onCopied }: { onUse?: (v: string) => void; onCopied?: (s: number) => void }) {
  const [kind, setKind] = useState<"password" | "passphrase">(() => load("sompio.gen", { kind: "password" as const }).kind);
  const [pw, setPw] = useState<PasswordRules>(() => load("sompio.gen.pw", PW_DEFAULT));
  const [pp, setPp] = useState<PassphraseRules>(() => load("sompio.gen.pp", PP_DEFAULT));
  const [out, setOut] = useState<{ value: string; strength: Strength } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const make = useCallback(async () => {
    try {
      setOut(await api.vaultGenerate(kind, pw, pp));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
    try {
      localStorage.setItem("sompio.gen", JSON.stringify({ kind }));
      localStorage.setItem("sompio.gen.pw", JSON.stringify(pw));
      localStorage.setItem("sompio.gen.pp", JSON.stringify(pp));
    } catch { /* fine */ }
  }, [kind, pw, pp]);
  useEffect(() => { make(); }, [make]);
  return (
    <div className="generator">
      <div className="segmented" role="tablist" aria-label="Kind">
        <button role="tab" aria-selected={kind === "password"} onClick={() => setKind("password")}>Password</button>
        <button role="tab" aria-selected={kind === "passphrase"} onClick={() => setKind("passphrase")}>Passphrase</button>
      </div>
      <div className="gen-out">
        <code className="mono selectable">{out?.value ?? "…"}</code>
        <button className="quiet" aria-label="Make another" onClick={make}>↻</button>
      </div>
      <StrengthMeter s={out?.strength ?? null} />
      {kind === "password" ? (
        <div className="gen-opts">
          <label className="inline-fields"><span>Length</span>
            <input type="range" min={8} max={64} value={pw.length} onChange={(e) => setPw({ ...pw, length: Number(e.target.value) })} />
            <span className="num">{pw.length}</span>
          </label>
          <div className="day-picks">
            {([["lower", "a-z"], ["upper", "A-Z"], ["digits", "0-9"], ["symbols", "!#$"]] as [keyof PasswordRules, string][]).map(([k, label]) => (
              <button key={k} aria-pressed={!!pw[k]} onClick={() => setPw({ ...pw, [k]: !pw[k] })}>{label}</button>
            ))}
          </div>
          <label className="check"><input type="checkbox" checked={pw.no_lookalikes} onChange={(e) => setPw({ ...pw, no_lookalikes: e.target.checked })} /><span>Leave out look-alikes (0 O, 1 l I)</span></label>
          <label className="check"><input type="checkbox" checked={pw.every_kind} onChange={(e) => setPw({ ...pw, every_kind: e.target.checked })} /><span>Use every chosen kind at least once</span></label>
          <label className="field"><span className="small">Never use these characters</span><input type="text" value={pw.exclude} onChange={(e) => setPw({ ...pw, exclude: e.target.value })} placeholder={"Like \" ' ` \\"} /></label>
        </div>
      ) : (
        <div className="gen-opts">
          <label className="inline-fields"><span>Words</span>
            <input type="range" min={4} max={12} value={pp.words} onChange={(e) => setPp({ ...pp, words: Number(e.target.value) })} />
            <span className="num">{pp.words}</span>
          </label>
          <label className="inline-fields"><span>Between words</span>
            <select value={pp.separator} onChange={(e) => setPp({ ...pp, separator: e.target.value })}>
              <option value="-">dash -</option><option value=" ">space</option><option value=".">dot .</option><option value="_">underscore _</option><option value="">nothing</option>
            </select>
          </label>
          <label className="check"><input type="checkbox" checked={pp.capitalize} onChange={(e) => setPp({ ...pp, capitalize: e.target.checked })} /><span>Capital first letters</span></label>
          <label className="check"><input type="checkbox" checked={pp.number} onChange={(e) => setPp({ ...pp, number: e.target.checked })} /><span>Add a digit</span></label>
          <p className="small faint">Words from the EFF's list of 7,776. Six words is about 77 bits: easy to type, very hard to guess.</p>
        </div>
      )}
      {error && <div className="error">{error}</div>}
      <div className="toolbar">
        {onUse && <button className="primary" disabled={!out} onClick={() => out && onUse(out.value)}>Use this</button>}
        <button disabled={!out} onClick={async () => out && onCopied?.(await api.vaultCopyText(out.value))}><Icon name="copy" size={16} />Copy</button>
      </div>
    </div>
  );
}

const ISSUE_TEXT = {
  weak: "Weak",
  reused: "Reused",
  expired: "Expired",
  old: "Old",
  no_password: "No password",
} as const;

export function Health({ onOpen }: { onOpen: (id: string) => void }) {
  const [r, setR] = useState<HealthReport | null>(null);
  const [leaks, setLeaks] = useState<VaultLeak[] | null>(null);
  const [checking, setChecking] = useState(false);
  const [leakError, setLeakError] = useState<string | null>(null);
  useEffect(() => { api.vaultHealth().then(setR).catch(() => {}); }, []);
  const checkLeaks = async () => {
    setChecking(true);
    setLeakError(null);
    try {
      setLeaks(await api.vaultCheckLeaks());
    } catch (e) {
      setLeakError(String(e));
    } finally {
      setChecking(false);
    }
  };
  if (!r) return <p className="muted">Checking your passwords…</p>;
  const tone = r.score >= 85 ? "ok" : r.score >= 60 ? "mid" : "bad";
  return (
    <div className="health">
      <section className="panel health-top">
        <div className={`health-score ${tone}`}><strong className="num">{r.score}</strong><span>out of 100</span></div>
        <div className="health-counts">
          <div><strong className="num">{r.weak}</strong><span>weak</span></div>
          <div><strong className="num">{r.reused}</strong><span>reused</span></div>
          <div><strong className="num">{r.old}</strong><span>older than a year</span></div>
          <div><strong className="num">{r.expired}</strong><span>expired</span></div>
        </div>
        <p className="small muted">Checked {plural(r.checked, "password", "passwords")} on this PC. Nothing was sent anywhere.</p>
      </section>
      <section className="panel leak-check">
        <div>
          <strong>Have any of your passwords been in a data breach?</strong>
          <span className="small muted">
            Sompio asks Have I Been Pwned, the breach list security teams use. Only the first 5 characters of a scrambled
            fingerprint (SHA-1 hash) of each password are sent; the answer covers hundreds of passwords, and the match happens
            on this PC. Your passwords never leave it. This is the only part of Vault that goes online, and only when you press the button.
          </span>
        </div>
        <button className={leaks ? "" : "primary"} disabled={checking || r.checked === 0} onClick={checkLeaks}>{checking ? "Checking…" : leaks ? "Check again" : "Check for breaches"}</button>
        {leakError && <div className="error">{leakError}</div>}
        {leaks && leaks.length === 0 && <div className="success">None of your passwords show up in known breaches.</div>}
        {leaks && leaks.length > 0 && (
          <ul className="health-list">
            {leaks.map((l) => (
              <li key={l.entry.id}>
                <button className="health-row" onClick={() => onOpen(l.entry.id)}>
                  <Avatar title={l.entry.title} icon={l.entry.icon} />
                  <span className="health-name"><strong>{l.entry.title}</strong><span className="small muted">{l.entry.username || l.entry.group_path}</span></span>
                  <span className="health-issues"><span className="issue issue-weak">Seen {l.seen.toLocaleString()} times in breaches</span></span>
                  <Icon name="arrow" size={16} />
                </button>
              </li>
            ))}
            <li className="small muted">Change these first: attackers try breached passwords on every site.</li>
          </ul>
        )}
      </section>
      {r.items.length === 0 ? (
        <div className="notice"><Icon name="check" /><div><strong>Nothing to fix.</strong><span>Every password is strong, unique and up to date.</span></div></div>
      ) : (
        <ul className="health-list">
          {r.items.map((it) => (
            <li key={it.entry.id}>
              <button className="health-row" onClick={() => onOpen(it.entry.id)}>
                <Avatar title={it.entry.title} icon={it.entry.icon} />
                <span className="health-name"><strong>{it.entry.title}</strong><span className="small muted">{it.entry.username || it.entry.group_path}</span></span>
                <span className="health-issues">
                  {it.issues.map((x, i) => (
                    <span key={i} className={`issue issue-${x.kind}`} title={x.kind === "reused" ? `Also used for ${x.others.join(", ")}` : x.kind === "weak" ? x.hint ?? "" : ""}>
                      {ISSUE_TEXT[x.kind]}
                      {x.kind === "reused" && ` ×${x.count}`}
                      {x.kind === "old" && `, ${Math.floor(x.days / 365)}y`}
                    </span>
                  ))}
                </span>
                <Icon name="arrow" size={16} />
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** The entry's website icon, or a key when it has none. */
export function Avatar({ title, icon = null, big = false }: { title: string; icon?: string | null; big?: boolean }) {
  const src = useIcon(icon);
  return src ? (
    <img className={`avatar img${big ? " big" : ""}`} src={src} alt="" aria-hidden title={title} />
  ) : (
    <span className={`avatar none${big ? " big" : ""}`} aria-hidden title={title}>
      <Icon name="key" size={big ? 20 : 13} />
    </span>
  );
}

/** Turns a key press into "Ctrl+Alt+A". */
function comboFrom(e: React.KeyboardEvent): string | null {
  const k = e.key;
  if (["Control", "Alt", "Shift", "Meta"].includes(k)) return null;
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(Boolean) as string[];
  if (mods.length === 0) return null;
  const key = k.length === 1 ? k.toUpperCase() : /^F\d+$/.test(k) ? k : e.code.replace(/^Key|^Digit/, "");
  return [...mods, key].join("+");
}

export function VaultSettingsPanel({ readOnly, onDone }: { readOnly: boolean; onDone: (ok: boolean, text: string) => void }) {
  const [s, setS] = useState<VaultSettings | null>(null);
  const [capturing, setCapturing] = useState(false);
  const [importing, setImporting] = useState<{ path: string; password: string; keyfile: string | null; merge: boolean } | null>(null);
  const [exporting, setExporting] = useState<{ password: string; again: string } | null>(null);
  const [csvWarn, setCsvWarn] = useState(false);
  const [busy, setBusy] = useState(false);
  const [iconProgress, setIconProgress] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => { api.vaultSettings().then(setS).catch(() => {}); }, []);
  if (!s) return null;
  const save = async (n: VaultSettings) => {
    try {
      await api.vaultSetSettings(n);
      setS(n);
    } catch (e) {
      onDone(false, String(e));
    }
  };
  const run = async (f: () => Promise<string>) => {
    setBusy(true);
    try {
      onDone(true, await f());
    } catch (e) {
      onDone(false, String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="cal-settings">
      <section className="setting">
        <h3>Auto-type</h3>
        <p className="muted">
          Click into a login form, press the shortcut, and Sompio types the username and password for the window in front.
          It picks the entry by the window's title, the site's name, or a window rule you set on the entry.
        </p>
        <div className="inline-fields">
          <span>Shortcut</span>
          <input
            ref={input}
            type="text"
            className="shortcut-input mono"
            readOnly
            value={capturing ? "Press the keys…" : s.shortcut || "Off"}
            onFocus={() => setCapturing(true)}
            onBlur={() => setCapturing(false)}
            onKeyDown={(e) => {
              e.preventDefault();
              if (e.key === "Escape") return input.current?.blur();
              const c = comboFrom(e);
              if (c) { save({ ...s, shortcut: c }); input.current?.blur(); }
            }}
          />
          <button className="small-btn" onClick={() => input.current?.focus()}>Change</button>
          {s.shortcut && <button className="small-btn ghost" onClick={() => save({ ...s, shortcut: "" })}>Turn off</button>}
        </div>
        <label className="check"><input type="checkbox" checked={s.lock_after_type} onChange={(e) => save({ ...s, lock_after_type: e.target.checked })} /><span>Lock Sompio after auto-typing</span></label>
      </section>
      <section className="setting">
        <h3>Website icons</h3>
        <label className="check">
          <input type="checkbox" checked={s.auto_icons} onChange={(e) => save({ ...s, auto_icons: e.target.checked })} />
          <span>
            Get an entry's icon from its website when you save it
            <small className="muted"> Sompio asks only the entry's own website, like opening it in a browser. No icon service in between, and nothing about you or your passwords is sent.</small>
          </span>
        </label>
        <div className="toolbar">
          <button disabled={readOnly || busy} onClick={() => run(async () => {
            const un = await listen<[number, number]>("vault-icons-progress", ([d, t]) => setIconProgress(t ? `${d} of ${t}` : null));
            try {
              const [found, total] = await api.vaultFetchAllIcons();
              await reloadIcons();
              return total === 0 ? "Every entry with a website already has an icon." : found === total ? `Found icons for all ${total} entries.` : `Found icons for ${found} of ${total} entries. For the others, open the entry, click Icon, and give another address or a picture.`;
            } finally {
              un();
              setIconProgress(null);
            }
          })}>Get icons for all entries{iconProgress ? ` (${iconProgress})` : ""}</button>
        </div>
      </section>
      <section className="setting">
        <h3>Copying</h3>
        <div className="inline-fields">
          <span>Clear copied passwords from the clipboard after</span>
          <select value={s.clear_seconds} onChange={(e) => save({ ...s, clear_seconds: Number(e.target.value) })}>
            {[10, 20, 30, 60, 0].map((x) => <option key={x} value={x}>{x === 0 ? "never" : `${x} seconds`}</option>)}
          </select>
        </div>
        <p className="small muted">Copied passwords stay out of Windows' clipboard history (Win+V) and cloud clipboard.</p>
      </section>
      <section className="setting">
        <h3>Bring passwords in</h3>
        <p className="muted">Each import lands in its own folder, so nothing you have is overwritten. To work on your passwords on another PC with KeePassXC, save a KeePass file below, use it there, then "Merge changes from a copy" to bring the changes back.</p>
        <div className="toolbar">
          <button disabled={readOnly || busy} onClick={async () => {
            const p = await pickOpen("Choose a KeePass file", [{ name: "KeePass", extensions: ["kdbx", "kdb"] }], "C:\\Users\\you\\Documents\\Passwords.kdbx");
            if (p) setImporting({ path: p, password: "", keyfile: null, merge: false });
          }}><Icon name="key" size={16} />KeePass or KeePassXC file</button>
          <button disabled={readOnly || busy} title="For a copy of this vault you exported and changed elsewhere" onClick={async () => {
            const p = await pickOpen("Choose the copy to merge", [{ name: "KeePass", extensions: ["kdbx"] }], "C:\\Users\\you\\Documents\\Sompio passwords.kdbx");
            if (p) setImporting({ path: p, password: "", keyfile: null, merge: true });
          }}>Merge changes from a copy</button>
          <button disabled={readOnly || busy} onClick={async () => {
            const p = await pickOpen("Choose a password export", [{ name: "CSV", extensions: ["csv"] }], "C:\\Users\\you\\Downloads\\bitwarden_export.csv");
            if (p) run(async () => { const r = await api.vaultImportCsv(p); return `Brought in ${plural(r.entries, "entry", "entries")}.${r.warnings.length ? ` ${r.warnings.length} rows couldn't be read.` : ""} Delete the CSV file now: it holds your passwords in plain text.`; });
          }}><Icon name="folder" size={16} />CSV export</button>
        </div>
        <p className="small faint">CSV exports from Bitwarden, Chrome, Edge, Brave, Firefox, 1Password, LastPass, Proton Pass, Dashlane and KeePassXC work.</p>
        {importing && (
          <form className="panel inset-form" onSubmit={(e) => {
            e.preventDefault();
            const im = importing;
            run(async () => {
              if (im.merge) {
                const m = await api.vaultMergeKdbx(im.path, im.password, im.keyfile);
                setImporting(null);
                const parts = [m.added && `${plural(m.added, "new entry", "new entries")}`, m.updated && `${m.updated} updated`, m.moved && `${m.moved} moved`, m.deleted && `${m.deleted} deleted`].filter(Boolean);
                return parts.length ? `Merged: ${parts.join(", ")}. Older versions are kept in each entry's history.${m.warnings.length ? " " + m.warnings.join(" ") : ""}` : "Nothing new in that copy.";
              }
              const r = await api.vaultImportKdbx(im.path, im.password, im.keyfile);
              setImporting(null);
              return `Brought in ${plural(r.entries, "entry", "entries")} in ${plural(r.groups + 1, "folder", "folders")}.${r.warnings.length ? " " + r.warnings.join(" ") : ""}`;
            });
          }}>
            <span className="small">{importing.merge ? "Merging" : "Opening"} <strong>{importing.path.split(/[\\/]/).pop()}</strong>{importing.merge && ". Entries match by their ID; the newer version of each wins."}</span>
            <label className="field"><span>Its password</span><input type="password" autoFocus value={importing.password} onChange={(e) => setImporting({ ...importing, password: e.target.value })} /></label>
            <div className="inline-fields">
              <button type="button" className="small-btn" onClick={async () => { const k = await pickOpen("Choose the key file", [{ name: "Key file", extensions: ["keyx", "key", "*"] }]); if (k) setImporting({ ...importing, keyfile: k }); }}>{importing.keyfile ? "Key file chosen" : "It has a key file too"}</button>
            </div>
            <div className="toolbar"><button className="primary" disabled={busy}>{busy ? "Opening…" : importing.merge ? "Merge" : "Bring in"}</button><button type="button" className="quiet" onClick={() => setImporting(null)}>Cancel</button></div>
          </form>
        )}
      </section>
      <section className="setting">
        <h3>Take your passwords with you</h3>
        <p className="muted">A KeePass file opens in KeePassXC, KeePass, KeeWeb, Strongbox and others, with the password you set here. This works even when your access has ended.</p>
        <div className="toolbar">
          <button disabled={busy} onClick={() => setExporting({ password: "", again: "" })}><Icon name="key" size={16} />Save as KeePass file</button>
          <button disabled={busy} onClick={() => setCsvWarn(true)}>Save as CSV</button>
        </div>
        {exporting && (
          <form className="panel inset-form" onSubmit={async (e) => {
            e.preventDefault();
            const p = await pickSave("Save the KeePass file", "Sompio passwords.kdbx", [{ name: "KeePass", extensions: ["kdbx"] }]);
            if (p) run(async () => { await api.vaultExportKdbx(p, exporting.password); setExporting(null); return "Saved. Open it with the password you just chose."; });
          }}>
            <label className="field"><span>Password for the file</span><input type="password" autoFocus value={exporting.password} onChange={(e) => setExporting({ ...exporting, password: e.target.value })} /></label>
            <label className="field"><span>Again</span><input type="password" value={exporting.again} onChange={(e) => setExporting({ ...exporting, again: e.target.value })} /></label>
            <div className="toolbar">
              <button className="primary" disabled={busy || exporting.password.length < 8 || exporting.password !== exporting.again}>Choose where to save</button>
              <button type="button" className="quiet" onClick={() => setExporting(null)}>Cancel</button>
              {exporting.password && exporting.password.length < 8 && <span className="small warn-text">At least 8 characters.</span>}
            </div>
          </form>
        )}
        {csvWarn && (
          <div className="notice warn">
            <Icon name="eye" />
            <div><strong>A CSV file isn't encrypted.</strong><span>Anyone who opens it sees every password. Use it only to move to another app, then delete it.</span></div>
            <button onClick={async () => {
              setCsvWarn(false);
              const p = await pickSave("Save as CSV", "Sompio passwords.csv", [{ name: "CSV", extensions: ["csv"] }]);
              if (p) run(async () => { await api.vaultExportCsv(p); return "Saved. Delete it once you've imported it elsewhere."; });
            }}>Save anyway</button>
            <button className="quiet" onClick={() => setCsvWarn(false)}>Cancel</button>
          </div>
        )}
      </section>
    </div>
  );
}

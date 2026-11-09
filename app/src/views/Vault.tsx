import { useCallback, useEffect, useState } from "react";
import { api, listen, type Totp, type VaultEntry, type VaultEntryDetail, type VaultGroup } from "../api";
import Icon from "../Icon";
import { dateTime, plural } from "../format";
import VaultEntryDialog from "./VaultEntryDialog";
import { Avatar, Generator, Health, VaultSettingsPanel } from "./VaultTools";

type Tab = "passwords" | "health" | "generator" | "settings";
const when = (iso: string | null) => (iso ? dateTime(Date.parse(iso)) : "");

interface Props { readOnly: boolean; onAddKey: () => void }

export default function Vault({ readOnly, onAddKey }: Props) {
  const [tab, setTab] = useState<Tab>("passwords");
  const [groups, setGroups] = useState<VaultGroup[]>([]);
  const [group, setGroup] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [entries, setEntries] = useState<VaultEntry[] | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<VaultEntryDetail | null>(null);
  const [editing, setEditing] = useState<VaultEntryDetail | "new" | null>(null);
  const [toast, setToast] = useState<{ ok: boolean; text: string } | null>(null);
  const [newFolder, setNewFolder] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [g, e] = await Promise.all([api.vaultGroups(), api.vaultEntries(group, search)]);
      setGroups(g);
      setEntries(e);
    } catch (e) {
      setToast({ ok: false, text: String(e) });
    }
  }, [group, search]);
  useEffect(() => { load(); }, [load]);
  useEffect(() => {
    if (!selected) return setDetail(null);
    api.vaultEntry(selected).then(setDetail).catch(() => setDetail(null));
  }, [selected, entries]);
  useEffect(() => {
    if (!toast) return;
    const t = window.setTimeout(() => setToast(null), toast.ok ? 4000 : 7000);
    return () => window.clearTimeout(t);
  }, [toast]);
  useEffect(() => {
    const un = listen("vault-clipboard-cleared", () => setToast({ ok: true, text: "Clipboard cleared." }));
    return () => { un.then((f) => f()); };
  }, []);

  const run = async (f: () => Promise<unknown>, ok?: string) => {
    try {
      await f();
      if (ok) setToast({ ok: true, text: ok });
      await load();
    } catch (e) {
      setToast({ ok: false, text: String(e) });
    }
  };
  const copied = (seconds: number, what = "Copied") =>
    setToast({ ok: true, text: seconds > 0 ? `${what}. It clears from the clipboard in ${seconds} seconds.` : `${what}.` });

  const bin = groups.find((g) => g.recycle_bin);
  const inBin = !!bin && group === bin.id;
  const current = groups.find((g) => g.id === group);

  return (
    <div className="page full vault-page">
      <div className="page-head">
        <div>
          <h1>Vault</h1>
          <p className="muted">Your passwords, in the KeePass format, encrypted with your lock. Nothing leaves this PC.</p>
        </div>
        <div className="toolbar">
          <button className="primary" disabled={readOnly} onClick={() => setEditing("new")}><Icon name="plus" size={16} />New entry</button>
        </div>
      </div>
      {readOnly && (
        <div className="notice warn">
          <Icon name="lock" />
          <div><strong>Vault is read-only.</strong><span>You can still see, copy, auto-type and export every password. Add a key to add or change them.</span></div>
          <button onClick={onAddKey}>Add a key</button>
        </div>
      )}
      <div className="segmented" role="tablist" aria-label="Vault">
        {([["passwords", "Passwords"], ["health", "Health check"], ["generator", "Generator"], ["settings", "Auto-type, import and export"]] as [Tab, string][]).map(([k, label]) => (
          <button key={k} role="tab" aria-selected={tab === k} onClick={() => setTab(k)}>{label}</button>
        ))}
      </div>

      {tab === "passwords" && (
        <div className="vault-layout">
          <nav className="panel folders" aria-label="Folders">
            <button className="folder" aria-current={group === null ? "true" : undefined} onClick={() => setGroup(null)}>
              <Icon name="key" size={16} /><span>All passwords</span>
            </button>
            {groups.filter((g) => g.depth > 0 && !g.recycle_bin && !isInBin(groups, g)).map((g) => (
              <div key={g.id} className="folder-row" style={{ paddingLeft: `${(g.depth - 1) * 0.9}rem` }}>
                <button className="folder" aria-current={group === g.id ? "true" : undefined} onClick={() => setGroup(g.id)}>
                  <Icon name="folder" size={16} /><span>{g.name}</span>{g.entries > 0 && <span className="faint small num">{g.entries}</span>}
                </button>
              </div>
            ))}
            {newFolder !== null ? (
              <form className="folder-new" onSubmit={(e) => { e.preventDefault(); const name = newFolder; setNewFolder(null); run(() => api.vaultAddGroup(current && !inBin ? current.id : null, name)); }}>
                <input type="text" autoFocus value={newFolder} placeholder={current && current.depth > 0 && !inBin ? `In ${current.name}` : "Folder name"} onChange={(e) => setNewFolder(e.target.value)} onBlur={() => !newFolder && setNewFolder(null)} />
              </form>
            ) : (
              <button className="folder add" disabled={readOnly} onClick={() => setNewFolder("")}><Icon name="plus" size={16} /><span>New folder</span></button>
            )}
            {bin && (
              <button className="folder bin" aria-current={inBin ? "true" : undefined} onClick={() => setGroup(bin.id)}>
                <Icon name="trash" size={16} /><span>Recycle bin</span>{bin.entries > 0 && <span className="faint small num">{bin.entries}</span>}
              </button>
            )}
          </nav>

          <section className="panel entry-list" aria-label="Entries">
            <div className="list-head">
              <div className="search-box">
                <Icon name="search" size={16} />
                <input type="search" placeholder={current && current.depth > 0 ? `Search ${current.name}` : "Search titles, usernames, websites, tags"} value={search} onChange={(e) => setSearch(e.target.value)} />
              </div>
              {current && current.depth > 0 && !current.recycle_bin && (
                <FolderMenu g={current} readOnly={readOnly} onRename={(n) => run(() => api.vaultRenameGroup(current.id, n))} onDelete={() => run(() => api.vaultDeleteGroup(current.id), `${current.name} moved to the recycle bin.`).then(() => setGroup(null))} />
              )}
              {inBin && entries && entries.length > 0 && (
                <button className="small-btn danger" onClick={() => run(async () => { const n = await api.vaultEmptyBin(); setSelected(null); return n; }, "Recycle bin emptied.")}>Empty</button>
              )}
            </div>
            {entries === null ? null : entries.length === 0 ? (
              <div className="day-empty">
                {search ? <span className="muted">Nothing matches "{search}".</span> : inBin ? <span className="muted">The recycle bin is empty.</span> : (
                  <>
                    <strong>No passwords here yet.</strong>
                    <span className="muted">Add one, or bring in your KeePass file or an export from your browser or another password manager.</span>
                    <div className="toolbar">
                      <button className="primary" disabled={readOnly} onClick={() => setEditing("new")}>New entry</button>
                      <button onClick={() => setTab("settings")}>Import</button>
                      <button className="quiet" onClick={() => run(() => api.vaultAddSamples(), "Sample entries added in the Samples folder.")}>Add sample entries</button>
                    </div>
                  </>
                )}
              </div>
            ) : (
              <ul className="entries" role="listbox" aria-label="Entries">
                {entries.map((e) => (
                  <li key={e.id} role="option" aria-selected={selected === e.id}>
                    <button className="entry-row" onClick={() => setSelected(e.id)} onDoubleClick={() => run(async () => copied(await api.vaultCopy(e.id, "Password"), "Password copied"))}>
                      <Avatar title={e.title} />
                      <span className="entry-name">
                        <strong>{e.title}</strong>
                        <span className="small muted">{e.username || e.url.replace(/^https?:\/\//, "") || " "}</span>
                      </span>
                      <span className="entry-flags">
                        {e.has_totp && <span className="flag" title="Has 2FA codes">2FA</span>}
                        {e.expired && <span className="flag bad" title="Expired">Expired</span>}
                        {group === null && e.group_path && <span className="faint small">{e.group_path}</span>}
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
            {entries && entries.length > 0 && <div className="list-foot small faint">{plural(entries.length, "entry", "entries")} · double-click to copy a password</div>}
          </section>

          <section className="panel entry-detail" aria-label="Entry">
            {detail ? (
              <EntryView
                key={detail.id}
                d={detail}
                groups={groups}
                readOnly={readOnly}
                onCopied={copied}
                onError={(t) => setToast({ ok: false, text: t })}
                onEdit={() => setEditing(detail)}
                run={run}
                onGone={() => setSelected(null)}
              />
            ) : (
              <div className="detail-empty muted">
                <Icon name="key" size={28} />
                <span>Choose an entry to see it here.</span>
              </div>
            )}
          </section>
        </div>
      )}

      {tab === "health" && <Health onOpen={(id) => { setSelected(id); setGroup(null); setSearch(""); setTab("passwords"); }} />}
      {tab === "generator" && <section className="panel gen-page"><Generator onCopied={(s) => copied(s)} /></section>}
      {tab === "settings" && <VaultSettingsPanel readOnly={readOnly} onDone={(ok, text) => { setToast({ ok, text }); load(); }} />}

      {editing && (
        <VaultEntryDialog
          groups={groups}
          editing={editing === "new" ? null : editing}
          group={group}
          onClose={() => setEditing(null)}
          onSaved={(id) => { setEditing(null); setSelected(id); setToast({ ok: true, text: "Saved." }); load(); api.vaultEntry(id).then(setDetail).catch(() => {}); }}
        />
      )}
      {toast && <div className={`toast${toast.ok ? "" : " bad"}`} role="status">{toast.text}</div>}
    </div>
  );
}

function isInBin(groups: VaultGroup[], g: VaultGroup): boolean {
  let cur: VaultGroup | undefined = g;
  while (cur) {
    if (cur.recycle_bin) return true;
    cur = groups.find((x) => x.id === cur!.parent);
  }
  return false;
}

function FolderMenu({ g, readOnly, onRename, onDelete }: { g: VaultGroup; readOnly: boolean; onRename: (n: string) => void; onDelete: () => void }) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState<string | null>(null);
  if (name !== null)
    return (
      <form onSubmit={(e) => { e.preventDefault(); onRename(name); setName(null); }}>
        <input type="text" autoFocus value={name} onChange={(e) => setName(e.target.value)} onBlur={() => setName(null)} />
      </form>
    );
  return (
    <div className="menu-wrap">
      <button className="small-btn ghost" disabled={readOnly} aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(!open)}>Folder ▾</button>
      {open && (
        <div className="menu" role="menu" onMouseLeave={() => setOpen(false)}>
          <button role="menuitem" onClick={() => { setOpen(false); setName(g.name); }}>Rename</button>
          <button role="menuitem" onClick={() => { setOpen(false); onDelete(); }}>Move to the recycle bin</button>
        </div>
      )}
    </div>
  );
}

interface EntryProps {
  d: VaultEntryDetail;
  groups: VaultGroup[];
  readOnly: boolean;
  onCopied: (s: number, what?: string) => void;
  onError: (t: string) => void;
  onEdit: () => void;
  run: (f: () => Promise<unknown>, ok?: string) => Promise<void>;
  onGone: () => void;
}

function EntryView({ d, groups, readOnly, onCopied, onError, onEdit, run, onGone }: EntryProps) {
  const [shown, setShown] = useState<Record<string, string>>({});
  const [totp, setTotp] = useState<Totp | null>(null);
  const [history, setHistory] = useState(false);
  const [moving, setMoving] = useState(false);

  useEffect(() => {
    if (!d.has_totp) return;
    let alive = true;
    const tick = () => api.vaultTotp(d.id).then((t) => alive && setTotp(t)).catch(() => {});
    tick();
    const t = window.setInterval(tick, 1000);
    return () => { alive = false; window.clearInterval(t); };
  }, [d.id, d.has_totp]);

  const copy = (field: string, what: string) => api.vaultCopy(d.id, field).then((s) => onCopied(s, what)).catch((e) => onError(String(e)));
  const toggle = async (field: string, history: number | null = null) => {
    const k = history == null ? field : `${field}#${history}`;
    if (k in shown) {
      const { [k]: _, ...rest } = shown;
      return setShown(rest);
    }
    try {
      setShown({ ...shown, [k]: await api.vaultReveal(d.id, field, history) });
    } catch (e) {
      onError(String(e));
    }
  };

  const row = (label: string, field: string, value: string | null, secret: boolean, extra?: React.ReactNode) => (
    <div className="kv">
      <span className="kv-label">{label}</span>
      <span className={`kv-value${secret ? " mono" : ""} selectable`}>{secret ? (field in shown ? shown[field] : "••••••••••••") : value}</span>
      <span className="kv-actions">
        {extra}
        {secret && <button className="quiet" aria-label={field in shown ? `Hide ${label}` : `Show ${label}`} onClick={() => toggle(field)}><Icon name="eye" size={16} /></button>}
        <button className="quiet" aria-label={`Copy ${label}`} onClick={() => copy(field, `${label} copied`)}><Icon name="copy" size={16} /></button>
      </span>
    </div>
  );

  return (
    <div className="entry-view">
      <header className="entry-head">
        <Avatar title={d.title} big />
        <div>
          <h2>{d.title}</h2>
          <span className="small muted">{d.in_recycle_bin ? "In the recycle bin" : d.group_path || "Top folder"}</span>
        </div>
      </header>
      {d.expired && <div className="notice warn"><Icon name="clock" /><div><strong>This password has expired.</strong><span>Change it on the site, then save the new one here.</span></div></div>}

      <div className="kvs">
        {d.username && row("Username", "UserName", d.username, false)}
        {d.has_password && row("Password", "Password", null, true)}
        {d.has_totp && (
          <div className="kv">
            <span className="kv-label">2FA code</span>
            <span className="kv-value totp num">
              {totp ? `${totp.code.slice(0, 3)} ${totp.code.slice(3)}` : "…"}
              {totp && <span className="totp-ring" style={{ ["--p" as string]: `${(totp.remaining / totp.period) * 100}%` }} title={`${totp.remaining} seconds left`} />}
            </span>
            <span className="kv-actions"><button className="quiet" aria-label="Copy 2FA code" onClick={() => copy("TOTP", "Code copied")}><Icon name="copy" size={16} /></button></span>
          </div>
        )}
        {d.url && row("Website", "URL", d.url, false)}
        {d.custom.map((c) => <div key={c.key}>{row(c.key, c.key, c.value, c.protected)}</div>)}
      </div>

      {d.tags.length > 0 && <div className="day-picks">{d.tags.map((t) => <span key={t} className="chip plain">{t}</span>)}</div>}
      {d.notes && <div className="entry-notes selectable">{d.notes}</div>}

      <div className="toolbar">
        {d.in_recycle_bin ? (
          <>
            <button className="primary" onClick={() => run(() => api.vaultRestoreEntry(d.id), "Restored.")}>Restore</button>
            <button className="danger" onClick={() => run(async () => { await api.vaultDeleteEntry(d.id); onGone(); }, "Deleted for good.")}>Delete for good</button>
          </>
        ) : (
          <>
            {d.has_password && <button className="primary" onClick={() => api.vaultAutotypeNow(d.id).catch((e) => onError(String(e)))} title="Sompio steps aside and types into the window behind it"><Icon name="arrow" size={16} />Type it</button>}
            <button disabled={readOnly} onClick={onEdit}>Edit</button>
            {moving ? (
              <select autoFocus defaultValue="" onChange={(e) => { setMoving(false); if (e.target.value) run(() => api.vaultMoveEntry(d.id, e.target.value), "Moved."); }} onBlur={() => setMoving(false)}>
                <option value="" disabled>Move to…</option>
                {groups.filter((g) => !g.recycle_bin && !isInBin(groups, g) && g.id !== d.group_id).map((g) => <option key={g.id} value={g.id}>{" ".repeat(g.depth)}{g.depth === 0 ? "Top folder" : g.name}</option>)}
              </select>
            ) : (
              <button disabled={readOnly} onClick={() => setMoving(true)}>Move</button>
            )}
            <button className="quiet danger" onClick={() => run(async () => { await api.vaultDeleteEntry(d.id); onGone(); }, "Moved to the recycle bin.")}>Delete</button>
          </>
        )}
      </div>

      <dl className="entry-meta small">
        {d.password_modified && <><dt>Password set</dt><dd>{when(d.password_modified)}</dd></>}
        {d.modified && <><dt>Changed</dt><dd>{when(d.modified)}</dd></>}
        {d.created && <><dt>Created</dt><dd>{when(d.created)}</dd></>}
        {d.expires && <><dt>Expires</dt><dd>{when(d.expires)}</dd></>}
        {d.autotype.windows.length > 0 && <><dt>Auto-types into</dt><dd className="mono">{d.autotype.windows.join(", ")}</dd></>}
      </dl>

      {d.history.length > 0 && (
        <details className="history" open={history} onToggle={(e) => setHistory((e.target as HTMLDetailsElement).open)}>
          <summary className="small muted">{plural(d.history.length, "earlier version", "earlier versions")}</summary>
          <ul>
            {d.history.map((h) => (
              <li key={h.index}>
                <span className="small">{when(h.modified)}{h.password_changed ? " · different password" : ""}</span>
                <span className="mono small selectable">{`Password#${h.index}` in shown ? shown[`Password#${h.index}`] : ""}</span>
                <span className="kv-actions">
                  <button className="small-btn ghost" onClick={() => toggle("Password", h.index)}>{`Password#${h.index}` in shown ? "Hide" : "Show password"}</button>
                  {!readOnly && <button className="small-btn ghost" onClick={() => run(() => api.vaultRestoreVersion(d.id, h.index), "That version is back. The one you had is in the history now.")}>Use this version</button>}
                </span>
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}

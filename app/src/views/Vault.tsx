import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, listen, openExternal, pickOpen, pickSave, type Totp, type VaultEntry, type VaultEntryDetail, type VaultGroup } from "../api";
import Icon from "../Icon";
import { dateTime, plural } from "../format";
import VaultEntryDialog from "./VaultEntryDialog";
import { reloadIcons } from "../vaultIcons";
import { Avatar, Generator, Health, VaultSettingsPanel } from "./VaultTools";

type Tab = "passwords" | "health" | "generator" | "settings";
type SortKey = "title" | "username" | "url" | "group" | "modified";
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
  const [sort, setSort] = useState<{ key: SortKey; up: boolean }>({ key: "title", up: true });
  const [dropOn, setDropOn] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const rowsRef = useRef<HTMLDivElement>(null);

  const load = useCallback(async () => {
    try {
      const [g, e] = await Promise.all([api.vaultGroups(), api.vaultEntries(group, search), reloadIcons()]);
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
  const pick = entries?.find((e) => e.id === selected) ?? null;

  const sorted = useMemo(() => {
    if (!entries) return null;
    const val = (e: VaultEntry) =>
      sort.key === "group" ? e.group_path : sort.key === "modified" ? e.modified ?? "" : sort.key === "url" ? e.url.replace(/^https?:\/\/(www\.)?/, "") : e[sort.key];
    const out = [...entries].sort((a, b) => val(a).toLowerCase().localeCompare(val(b).toLowerCase()) || a.title.localeCompare(b.title));
    return sort.up ? out : out.reverse();
  }, [entries, sort]);

  // What the toolbar and the KeePassXC shortcuts do
  const act = {
    copy: (field: string, what: string) => pick && api.vaultCopy(pick.id, field).then((s) => copied(s, what)).catch((e) => setToast({ ok: false, text: String(e) })),
    open: () => pick?.url && openExternal(/^[a-z]+:\/\//i.test(pick.url) ? pick.url : `https://${pick.url}`).catch((e) => setToast({ ok: false, text: String(e) })),
    type: () => pick?.has_password && api.vaultAutotypeNow(pick.id).catch((e) => setToast({ ok: false, text: String(e) })),
    edit: () => !readOnly && detail && setEditing(detail),
    clone: () => !readOnly && pick && !pick.in_recycle_bin && run(async () => { const id = await api.vaultCloneEntry(pick.id); setSelected(id); }, "Cloned."),
    remove: () => pick && run(async () => { const gone = pick.in_recycle_bin; await api.vaultDeleteEntry(pick.id); setSelected(null); return gone; }, pick.in_recycle_bin ? "Deleted for good." : "Moved to the recycle bin."),
  };
  const move = (d: number) => {
    if (!sorted || sorted.length === 0) return;
    const i = sorted.findIndex((e) => e.id === selected);
    const next = sorted[Math.max(0, Math.min(sorted.length - 1, i < 0 ? 0 : i + d))];
    setSelected(next.id);
    rowsRef.current?.querySelector(`[data-id="${next.id}"]`)?.scrollIntoView({ block: "nearest" });
  };
  useEffect(() => {
    if (tab !== "passwords" || editing) return;
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement;
      const typing = el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT";
      const ctrl = e.ctrlKey || e.metaKey;
      const k = e.key.toLowerCase();
      if (ctrl && k === "f") { e.preventDefault(); searchRef.current?.focus(); searchRef.current?.select(); return; }
      if (ctrl && k === "n" && !readOnly) { e.preventDefault(); setEditing("new"); return; }
      if (typing && !(el === searchRef.current && (e.key === "ArrowDown" || e.key === "Enter"))) return;
      if (el === searchRef.current) { e.preventDefault(); rowsRef.current?.focus(); move(e.key === "Enter" ? 0 : 1); return; }
      if (e.key === "ArrowDown") { e.preventDefault(); return move(1); }
      if (e.key === "ArrowUp") { e.preventDefault(); return move(-1); }
      if (!pick) return;
      const go = (f: () => unknown) => { e.preventDefault(); f(); };
      if (ctrl && e.shiftKey && k === "v") return go(act.type);
      if (ctrl && k === "b") return go(() => act.copy("UserName", "Username copied"));
      if (ctrl && k === "c" && !String(window.getSelection() ?? "")) return go(() => act.copy("Password", "Password copied"));
      if (ctrl && k === "t" && pick.has_totp) return go(() => act.copy("TOTP", "Code copied"));
      if (ctrl && k === "u") return go(act.open);
      if (ctrl && k === "e") return go(act.edit);
      if (ctrl && k === "k") return go(act.clone);
      if (e.key === "Enter") return go(act.edit);
      if (e.key === "Delete") return go(act.remove);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const head = (key: SortKey, label: string) => (
    <button className="th" aria-sort={sort.key === key ? (sort.up ? "ascending" : "descending") : "none"} onClick={() => setSort({ key, up: sort.key === key ? !sort.up : true })}>
      {label}{sort.key === key && <span className="arrow">{sort.up ? "▲" : "▼"}</span>}
    </button>
  );
  const dropProps = (gid: string) => ({
    onDragOver: (e: React.DragEvent) => { if (!readOnly && e.dataTransfer.types.includes("sompio/entry")) { e.preventDefault(); setDropOn(gid); } },
    onDragLeave: () => setDropOn((d) => (d === gid ? null : d)),
    onDrop: (e: React.DragEvent) => {
      e.preventDefault();
      setDropOn(null);
      const id = e.dataTransfer.getData("sompio/entry");
      if (id) run(() => (bin && gid === bin.id ? api.vaultDeleteEntry(id) : api.vaultMoveEntry(id, gid)), bin && gid === bin.id ? "Moved to the recycle bin." : "Moved.");
    },
  });
  const root = groups.find((g) => g.depth === 0);
  const tabs = ([["passwords", "Passwords"], ["health", "Health check"], ["generator", "Generator"], ["settings", "Auto-type, import and export"]] as [Tab, string][]);

  return (
    <div className="xc-page vault-page">
      <div className="app-head">
        <div className="app-title">
          <h1>Vault</h1>
          <span className="muted">Your passwords in the KeePass format, encrypted with your lock. Nothing leaves this PC.</span>
        </div>
        <div className="segmented" role="tablist" aria-label="Vault">
          {tabs.map(([k, label]) => <button key={k} role="tab" aria-selected={tab === k} onClick={() => setTab(k)}>{label}</button>)}
        </div>
      </div>
      {readOnly && (
        <div className="notice warn vault-notice">
          <Icon name="lock" />
          <div><strong>Vault is read-only.</strong><span>You can still see, copy, auto-type and export every password. Add a key to add or change them.</span></div>
          <button onClick={onAddKey}>Add a key</button>
        </div>
      )}

      {tab === "passwords" && (
        <>
          <div className="xc-toolbar" role="toolbar" aria-label="Entries">
            <button disabled={readOnly} onClick={() => setEditing("new")} title="New entry (Ctrl+N)"><Icon name="plus" size={16} />New</button>
            <button disabled={readOnly || !detail} onClick={act.edit} title="Edit entry (Ctrl+E)"><Icon name="edit" size={16} />Edit</button>
            <button disabled={readOnly || !pick || pick.in_recycle_bin} onClick={act.clone} title="Clone entry (Ctrl+K)"><Icon name="copy" size={16} />Clone</button>
            <button disabled={!pick} onClick={act.remove} title="Delete entry (Delete)"><Icon name="trash" size={16} />Delete</button>
            <span className="sep" />
            <button disabled={!pick?.username} onClick={() => act.copy("UserName", "Username copied")} title="Copy username (Ctrl+B)"><Icon name="user" size={16} />Copy username</button>
            <button disabled={!pick?.has_password} onClick={() => act.copy("Password", "Password copied")} title="Copy password (Ctrl+C)"><Icon name="key" size={16} />Copy password</button>
            {pick?.has_totp && <button onClick={() => act.copy("TOTP", "Code copied")} title="Copy 2FA code (Ctrl+T)"><Icon name="clock" size={16} />Copy 2FA</button>}
            <button disabled={!pick?.url} onClick={act.open} title="Open the website (Ctrl+U)"><Icon name="external" size={16} />Open URL</button>
            <button disabled={!pick?.has_password || pick.in_recycle_bin} onClick={act.type} title="Type it into the window behind Sompio (Ctrl+Shift+V)"><Icon name="arrow" size={16} />Type it</button>
            <span className="grow" />
            <div className="search-box xc-search">
              <Icon name="search" size={16} />
              <input ref={searchRef} type="search" placeholder={current && current.depth > 0 ? `Search ${current.name} (Ctrl+F)` : "Search (Ctrl+F)"} value={search} onChange={(e) => setSearch(e.target.value)} />
            </div>
          </div>

          <div className="xc-body">
            <nav className="xc-tree folders" aria-label="Folders">
              <button className="xc-node root" aria-current={group === null} onClick={() => setGroup(null)} {...(root ? dropProps(root.id) : {})} data-drop={root && dropOn === root.id}>
                <Icon name="key" size={16} /><span>All passwords</span>
              </button>
              {groups.filter((g) => g.depth > 0 && !g.recycle_bin && !isInBin(groups, g)).map((g) => (
                <button
                  key={g.id}
                  className="xc-node"
                  style={{ paddingLeft: `${0.5 + g.depth * 0.9}rem` }}
                  aria-current={group === g.id}
                  data-drop={dropOn === g.id}
                  onClick={() => setGroup(g.id)}
                  {...dropProps(g.id)}
                >
                  <Icon name="folder" size={16} /><span>{g.name}</span>{g.entries > 0 && <span className="num">{g.entries}</span>}
                </button>
              ))}
              {newFolder !== null ? (
                <form className="folder-new" onSubmit={(e) => { e.preventDefault(); const name = newFolder; setNewFolder(null); run(() => api.vaultAddGroup(current && !inBin ? current.id : null, name)); }}>
                  <input type="text" autoFocus value={newFolder} placeholder={current && current.depth > 0 && !inBin ? `In ${current.name}` : "Folder name"} onChange={(e) => setNewFolder(e.target.value)} onBlur={() => !newFolder && setNewFolder(null)} />
                </form>
              ) : (
                <button className="xc-node add" disabled={readOnly} onClick={() => setNewFolder("")}><Icon name="plus" size={16} /><span>New folder</span></button>
              )}
              {bin && (
                <button className="xc-node bin" aria-current={inBin} data-drop={dropOn === bin.id} onClick={() => setGroup(bin.id)} {...dropProps(bin.id)}>
                  <Icon name="trash" size={16} /><span>Recycle bin</span>{bin.entries > 0 && <span className="num">{bin.entries}</span>}
                </button>
              )}
              {current && current.depth > 0 && !current.recycle_bin && (
                <div className="xc-tree-foot">
                  <FolderMenu g={current} readOnly={readOnly} onRename={(n) => run(() => api.vaultRenameGroup(current.id, n))} onDelete={() => run(() => api.vaultDeleteGroup(current.id), `${current.name} moved to the recycle bin.`).then(() => setGroup(null))} />
                </div>
              )}
              {inBin && entries && entries.length > 0 && (
                <div className="xc-tree-foot">
                  <button className="small-btn danger" onClick={() => run(async () => { const n = await api.vaultEmptyBin(); setSelected(null); return n; }, "Recycle bin emptied.")}>Empty the recycle bin</button>
                </div>
              )}
            </nav>

            <div className="xc-main">
              <div className="xc-table">
                <div className="xc-thead vault-cols" role="row">
                  {head("title", "Title")}
                  {head("username", "Username")}
                  <span className="th">Password</span>
                  {head("url", "URL")}
                  {group === null ? head("group", "Folder") : <span className="th">Notes</span>}
                  {head("modified", "Modified")}
                </div>
                <div className="xc-rows" ref={rowsRef} tabIndex={0} role="listbox" aria-label="Entries">
                  {sorted === null ? null : sorted.length === 0 ? (
                    <div className="empty">
                      {search ? <span>Nothing matches "{search}".</span> : inBin ? <span>The recycle bin is empty.</span> : (
                        <>
                          <strong>No passwords here yet.</strong>
                          <span>Add one, or bring in your KeePass file or an export from your browser or another password manager.</span>
                          <div className="toolbar">
                            <button className="primary" disabled={readOnly} onClick={() => setEditing("new")}>New entry</button>
                            <button onClick={() => setTab("settings")}>Import</button>
                            <button className="quiet" onClick={() => run(() => api.vaultAddSamples(), "Sample entries added in the Samples folder.")}>Add sample entries</button>
                          </div>
                        </>
                      )}
                    </div>
                  ) : (
                    sorted.map((e) => (
                      <div
                        key={e.id}
                        data-id={e.id}
                        role="option"
                        aria-selected={selected === e.id}
                        className={`xc-row vault-cols${selected === e.id ? " active" : ""}${e.expired ? " expired" : ""}`}
                        draggable={!readOnly}
                        onDragStart={(ev) => { ev.dataTransfer.setData("sompio/entry", e.id); ev.dataTransfer.effectAllowed = "move"; }}
                        onClick={() => setSelected(e.id)}
                        onDoubleClick={() => run(async () => copied(await api.vaultCopy(e.id, "Password"), "Password copied"))}
                      >
                        <span className="c-title"><Avatar title={e.title} icon={e.icon} />{e.title}{e.has_totp && <span className="flag" title="Has 2FA codes">2FA</span>}{e.expired && <span className="flag bad">Expired</span>}</span>
                        <span>{e.username}</span>
                        <span className="faint">{e.has_password ? "••••••••" : ""}</span>
                        <span className="faint">{e.url.replace(/^https?:\/\/(www\.)?/, "")}</span>
                        <span className="faint">{group === null ? e.group_path : ""}</span>
                        <span className="faint c-date">{when(e.modified)}</span>
                      </div>
                    ))
                  )}
                </div>
                {entries && entries.length > 0 && (
                  <div className="xc-status small faint">
                    {plural(entries.length, "entry", "entries")} · Double-click copies the password · Drag an entry onto a folder to move it
                  </div>
                )}
              </div>

              <section className="xc-preview entry-detail" aria-label="Entry">
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
                  <div className="detail-empty small muted">Choose an entry to see it here. Shortcuts: Ctrl+B username, Ctrl+C password, Ctrl+U website, Ctrl+Shift+V type it.</div>
                )}
              </section>
            </div>
          </div>
        </>
      )}

      {tab === "health" && <div className="page"><Health onOpen={(id) => { setSelected(id); setGroup(null); setSearch(""); setTab("passwords"); }} /></div>}
      {tab === "generator" && <div className="page"><section className="panel gen-page"><Generator onCopied={(s) => copied(s)} /></section></div>}
      {tab === "settings" && <div className="page"><VaultSettingsPanel readOnly={readOnly} onDone={(ok, text) => { setToast({ ok, text }); load(); }} /></div>}

      {editing && (
        <VaultEntryDialog
          groups={groups}
          editing={editing === "new" ? null : editing}
          group={group}
          onClose={() => setEditing(null)}
          onSaved={async (id) => {
            setEditing(null);
            setSelected(id);
            setToast({ ok: true, text: "Saved." });
            await load();
            const d = await api.vaultEntry(id).catch(() => null);
            setDetail(d);
            // A new website, or no icon yet: get it from the site, quietly, if that's switched on
            const urlChanged = editing !== "new" && editing !== null && d && editing.url !== d.url;
            if (d && d.url && (!d.icon || urlChanged)) {
              const s = await api.vaultSettings().catch(() => null);
              if (s?.auto_icons) {
                api.vaultFetchIcon(id, null).then(() => load()).catch(() => {});
              }
            }
          }}
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

/** Get the entry's icon from its website, another address (or a picture's own address), or a file. */
function IconMenu({ d, run }: { d: VaultEntryDetail; run: (f: () => Promise<unknown>, ok?: string) => Promise<void> }) {
  const [open, setOpen] = useState(false);
  const [address, setAddress] = useState("");
  const [busy, setBusy] = useState(false);
  const go = async (f: () => Promise<unknown>, ok: string) => {
    setBusy(true);
    await run(f, ok);
    setBusy(false);
    setOpen(false);
    setAddress("");
  };
  const site = d.url.replace(/^https?:\/\//, "").split("/")[0];
  return (
    <div className="menu-wrap icon-menu">
      <button className="small-btn" aria-haspopup="dialog" aria-expanded={open} onClick={() => setOpen(!open)}>{busy ? "Getting the icon…" : "Icon ▾"}</button>
      {open && (
        <div className="menu icon-panel" role="dialog" aria-label="Entry icon">
          <button disabled={busy || !d.url} onClick={() => go(() => api.vaultFetchIcon(d.id, null), "Icon added from the website.")}>
            <Icon name="refresh" size={16} />{d.url ? `Get it from ${site}` : "No website on this entry"}
          </button>
          <form onSubmit={(e) => { e.preventDefault(); if (address.trim()) go(() => api.vaultFetchIcon(d.id, address.trim()), "Icon added."); }}>
            <span className="small muted">From another website, or a picture's own address</span>
            <div className="row">
              <input type="text" placeholder="blizzard.com or https://…/icon.png" value={address} onChange={(e) => setAddress(e.target.value)} />
              <button disabled={busy || !address.trim()}>Get</button>
            </div>
          </form>
          <button disabled={busy} onClick={async () => {
            const p = await pickOpen("Choose a picture for this entry", [{ name: "Picture", extensions: ["png", "ico", "jpg", "jpeg", "gif", "webp", "bmp"] }], "C:\\Users\\you\\Pictures\\icon.png");
            if (p) go(() => api.vaultIconFromFile(d.id, p), "Icon added.");
          }}><Icon name="folder" size={16} />Choose a picture file…</button>
          {d.icon && <button className="danger" disabled={busy} onClick={() => go(() => api.vaultRemoveIcon(d.id), "Icon removed.")}><Icon name="trash" size={16} />Remove the icon</button>}
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
        <Avatar title={d.title} icon={d.icon} big />
        <div>
          <h2>{d.title}</h2>
          <span className="small muted">{d.in_recycle_bin ? "In the recycle bin" : d.group_path || "Top folder"}</span>
        </div>
        {!readOnly && !d.in_recycle_bin && <IconMenu d={d} run={run} />}
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

      {(d.attachments.length > 0 || !readOnly) && !d.in_recycle_bin && (
        <div className="attachments">
          <span className="kv-label">Files</span>
          {d.attachments.map((a) => (
            <div key={a.name} className="attachment">
              <Icon name="folder" size={16} />
              <span className="grow">{a.name}</span>
              <span className="small faint num">{a.size < 1024 ? `${a.size} B` : a.size < 1048576 ? `${Math.round(a.size / 1024)} KB` : `${(a.size / 1048576).toFixed(1)} MB`}</span>
              <button className="small-btn ghost" onClick={async () => {
                const p = await pickSave("Save the file", a.name, []);
                if (p) run(() => api.vaultSaveAttachment(d.id, a.name, p), "Saved. That copy isn't encrypted, so delete it when you're done.");
              }}>Save</button>
              {!readOnly && <button className="small-btn ghost danger" onClick={() => run(() => api.vaultRemoveAttachment(d.id, a.name), "File removed. The earlier version of the entry still has it.")}>Remove</button>}
            </div>
          ))}
          {!readOnly && (
            <button className="small-btn" onClick={async () => {
              const p = await pickOpen("Choose a file to keep in this entry", [], "C:\\Users\\you\\Documents\\recovery-codes.txt");
              if (p) run(async () => { const n = await api.vaultAddAttachment(d.id, p); return n; }, "File added. It's encrypted with the vault; you can delete the original.");
            }}><Icon name="plus" size={14} />Attach a file</button>
          )}
        </div>
      )}

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

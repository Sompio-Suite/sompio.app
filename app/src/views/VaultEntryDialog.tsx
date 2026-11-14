import { useEffect, useRef, useState } from "react";
import { api, type Strength, type VaultEntryDetail, type VaultEntryInput, type VaultGroup } from "../api";
import Icon from "../Icon";
import { Generator, StrengthMeter } from "./VaultTools";

interface Props {
  groups: VaultGroup[];
  editing: VaultEntryDetail | null;
  group: string | null;
  onClose: () => void;
  onSaved: (id: string) => void;
}

interface Custom { key: string; value: string | null; protected: boolean; revealed: boolean }

export default function VaultEntryDialog({ groups, editing, group, onClose, onSaved }: Props) {
  const ref = useRef<HTMLDialogElement>(null);
  const places = groups.filter((g) => !g.recycle_bin);
  const [title, setTitle] = useState(editing?.title ?? "");
  const [username, setUsername] = useState(editing?.username ?? "");
  // null = unchanged (the current password isn't loaded unless you reveal it)
  const [password, setPassword] = useState<string | null>(editing ? null : "");
  const [show, setShow] = useState(false);
  const [url, setUrl] = useState(editing?.url ?? "");
  const [groupId, setGroupId] = useState(editing?.group_id ?? (group && places.some((g) => g.id === group) ? group : places[0]?.id ?? null));
  const [tags, setTags] = useState(editing?.tags.join(", ") ?? "");
  const [expires, setExpires] = useState(editing?.expires?.slice(0, 10) ?? "");
  const [notes, setNotes] = useState(editing?.notes ?? "");
  const [otp, setOtp] = useState<string | null>(editing ? null : "");
  const [custom, setCustom] = useState<Custom[]>(editing?.custom.map((c) => ({ ...c, revealed: !c.protected })) ?? []);
  const [at, setAt] = useState(editing?.autotype ?? { enabled: true, sequence: "", windows: [] });
  const [more, setMore] = useState(!!editing && (editing.custom.length > 0 || !!editing.autotype.sequence || editing.autotype.windows.length > 0));
  const [gen, setGen] = useState(false);
  const [strength, setStrength] = useState<Strength | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => { ref.current?.showModal(); }, []);
  useEffect(() => {
    if (!password) return setStrength(null);
    const t = window.setTimeout(() => api.vaultStrength(password).then(setStrength).catch(() => {}), 150);
    return () => window.clearTimeout(t);
  }, [password]);

  const revealCurrent = async () => {
    if (editing && password === null) setPassword(await api.vaultReveal(editing.id, "Password"));
    setShow(true);
  };

  const save = async () => {
    const input: VaultEntryInput = {
      group_id: groupId,
      title,
      username,
      password,
      url,
      notes,
      tags: tags.split(",").map((t) => t.trim()).filter(Boolean),
      expires: expires || null,
      custom: custom.filter((c) => c.key.trim()).map((c) => ({ key: c.key.trim(), value: c.value, protected: c.protected })),
      otp,
      autotype: at,
    };
    setBusy(true);
    setError(null);
    try {
      if (editing) {
        await api.vaultUpdateEntry(editing.id, input);
        onSaved(editing.id);
      } else {
        onSaved(await api.vaultAddEntry(input));
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <dialog ref={ref} className="sheet wide" onClose={onClose} aria-labelledby="entry-title">
      <form className="sheet-body" onSubmit={(e) => { e.preventDefault(); save(); }}>
        <h2 id="entry-title">{editing ? `Edit ${editing.title}` : "New entry"}</h2>
        <div className="field-row">
          <label className="field grow"><span>Title</span><input type="text" autoFocus value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Proton Mail" /></label>
          <label className="field"><span>Folder</span>
            <select value={groupId ?? ""} onChange={(e) => setGroupId(e.target.value)}>
              {places.map((g) => <option key={g.id} value={g.id}>{" ".repeat(g.depth)}{g.depth === 0 ? "Top folder" : g.name}</option>)}
            </select>
          </label>
        </div>
        <label className="field"><span>Username or email</span><input type="text" autoComplete="off" spellCheck={false} value={username} onChange={(e) => setUsername(e.target.value)} /></label>
        <div className="field">
          <span>Password</span>
          <div className="input-with-buttons">
            <input
              type={show ? "text" : "password"}
              className="mono"
              autoComplete="new-password"
              spellCheck={false}
              value={password ?? ""}
              placeholder={editing && password === null ? "Unchanged. Type to replace it." : ""}
              onChange={(e) => setPassword(e.target.value)}
            />
            <button type="button" className="quiet" aria-label={show ? "Hide" : "Show"} onClick={() => (show ? setShow(false) : revealCurrent())}><Icon name="eye" size={16} /></button>
            <button type="button" className={gen ? "" : "quiet"} onClick={() => setGen(!gen)}><Icon name="sparkle" size={16} />Generate</button>
          </div>
          <StrengthMeter s={strength} />
          {gen && (
            <div className="panel inset-form">
              <Generator onUse={(v) => { setPassword(v); setShow(true); setGen(false); }} />
            </div>
          )}
        </div>
        <label className="field"><span>Website</span><input type="text" spellCheck={false} value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://account.proton.me" /></label>
        <div className="field-row">
          <label className="field grow"><span>Tags <span className="faint">(comma separated)</span></span><input type="text" value={tags} onChange={(e) => setTags(e.target.value)} /></label>
          <label className="field"><span>Expires</span><input type="date" value={expires} onChange={(e) => setExpires(e.target.value)} /></label>
        </div>
        <div className="field">
          <span>2FA (one-time codes) <span className="faint">(optional)</span></span>
          {editing?.has_totp && otp === null ? (
            <div className="inline-fields"><span className="ok-text">Set up</span><button type="button" className="small-btn" onClick={() => setOtp("")}>Remove or replace</button></div>
          ) : (
            <input type="text" className="mono" spellCheck={false} value={otp ?? ""} onChange={(e) => setOtp(e.target.value)} placeholder="Setup key (JBSW Y3DP…) or otpauth:// link" />
          )}
        </div>
        <label className="field"><span>Notes</span><textarea rows={3} value={notes} onChange={(e) => setNotes(e.target.value)} /></label>

        <details open={more} onToggle={(e) => setMore((e.target as HTMLDetailsElement).open)}>
          <summary className="small muted">Extra fields and auto-type</summary>
          <div className="more-fields">
            <div className="field">
              <span>Extra fields</span>
              {custom.map((c, i) => (
                <div key={i} className="custom-row">
                  <input type="text" placeholder="Name (PIN, Security answer…)" value={c.key} onChange={(e) => setCustom(custom.map((x, j) => (j === i ? { ...x, key: e.target.value } : x)))} />
                  <input
                    type={c.protected && !c.revealed ? "password" : "text"}
                    placeholder={c.value === null ? "Hidden. Type to replace it." : "Value"}
                    value={c.value ?? ""}
                    onChange={(e) => setCustom(custom.map((x, j) => (j === i ? { ...x, value: e.target.value } : x)))}
                  />
                  <label className="check" title="Hidden and kept in protected memory"><input type="checkbox" checked={c.protected} onChange={(e) => setCustom(custom.map((x, j) => (j === i ? { ...x, protected: e.target.checked } : x)))} /><span>Hide</span></label>
                  <button type="button" className="quiet" aria-label="Remove field" onClick={() => setCustom(custom.filter((_, j) => j !== i))}>×</button>
                </div>
              ))}
              <div><button type="button" className="small-btn" onClick={() => setCustom([...custom, { key: "", value: "", protected: true, revealed: true }])}>Add a field</button></div>
            </div>
            <div className="field">
              <span>Auto-type</span>
              <label className="check"><input type="checkbox" checked={at.enabled} onChange={(e) => setAt({ ...at, enabled: e.target.checked })} /><span>Allow auto-type for this entry</span></label>
              <label className="field"><span className="small">What to type <span className="faint">(empty: username, Tab, password, Enter)</span></span>
                <input type="text" className="mono" value={at.sequence} onChange={(e) => setAt({ ...at, sequence: e.target.value })} placeholder="{USERNAME}{TAB}{PASSWORD}{ENTER}" />
              </label>
              <label className="field"><span className="small">Windows it's for <span className="faint">(one per line, * matches anything)</span></span>
                <textarea rows={2} className="mono" value={at.windows.join("\n")} onChange={(e) => setAt({ ...at, windows: e.target.value.split("\n") })} placeholder={"Zoom*\n*Microsoft Teams*"} />
              </label>
              <p className="small faint">Also: {"{TOTP}"} for the 2FA code, {"{S:PIN}"} for an extra field, {"{DELAY 500}"} to wait, {"{TAB 2}"}, ^a for Ctrl+A.</p>
            </div>
          </div>
        </details>

        {error && <div className="error" role="alert">{error}</div>}
        <div className="toolbar end">
          <button type="button" className="quiet" onClick={() => ref.current?.close()}>Cancel</button>
          <button className="primary" disabled={busy || !title.trim()}>{busy ? "Saving…" : editing ? "Save" : "Add entry"}</button>
        </div>
      </form>
    </dialog>
  );
}

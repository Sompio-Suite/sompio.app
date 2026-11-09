import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  api, listen,
  type ChatTarget, type VoiceChannel, type VoiceClient, type VoiceClientMsg, type VoiceConnectResult,
  type VoiceHosted, type VoiceRole, type VoiceSaved, type VoiceServerInfo, type VoiceServerMsg,
} from "../api";
import Icon from "../Icon";
import { microphones, VoiceEngine, voiceSupported } from "../voiceAudio";

interface Conn {
  address: string;
  label: string;
  fingerprint: string;
  you: number;
  server: VoiceServerInfo;
  channels: VoiceChannel[];
  clients: VoiceClient[];
}

interface Line {
  id: number;
  at: number;
  kind: "chat" | "system" | "poke" | "error";
  tab: string;
  from?: string;
  text: string;
}

type Trust = { result: Extract<VoiceConnectResult, { kind: "new_server" | "changed" }>; address: string; name: string; password: string | null };

let lineId = 0;
const time = (ms: number) => new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
const ROLE: Record<VoiceRole, string> = { admin: "Admin", moderator: "Mod", member: "" };

interface Props { canHost: boolean; onAddKey: () => void }

export default function Voice({ canHost, onAddKey }: Props) {
  const [saved, setSaved] = useState<VoiceSaved | null>(null);
  const [conn, setConn] = useState<Conn | null>(null);
  const [lines, setLines] = useState<Line[]>([]);
  const [tab, setTab] = useState("channel");
  const [privates, setPrivates] = useState<{ id: number; name: string }[]>([]);
  const [speaking, setSpeaking] = useState<Set<number>>(new Set());
  const [sending, setSending] = useState(false);
  const [level, setLevel] = useState(0);
  const [muted, setMuted] = useState(false);
  const [deafened, setDeafened] = useState(false);
  const [ping, setPing] = useState(0);
  const [trust, setTrust] = useState<Trust | null>(null);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<{ ok: boolean; text: string } | null>(null);
  const [micError, setMicError] = useState<string | null>(null);
  const engine = useRef<VoiceEngine | null>(null);
  const connRef = useRef<Conn | null>(null);
  connRef.current = conn;

  useEffect(() => { api.voiceSettings().then(setSaved).catch((e) => setNote({ ok: false, text: String(e) })); }, []);
  const saveSettings = (s: VoiceSaved) => { setSaved(s); api.voiceSaveSettings(s).catch((e) => setNote({ ok: false, text: String(e) })); };

  const add = useCallback((l: Omit<Line, "id" | "at"> & { at?: number }) => setLines((x) => [...x.slice(-400), { id: ++lineId, at: l.at ?? Date.now(), ...l }]), []);
  const nameOf = (id: number) => connRef.current?.clients.find((c) => c.id === id)?.name ?? "Someone";

  // Server events
  useEffect(() => {
    const unEvent = listen<VoiceServerMsg>("voice-event", (m) => {
      setConn((c) => {
        if (!c) return c;
        switch (m.type) {
          case "client_joined": return { ...c, clients: [...c.clients.filter((x) => x.id !== m.client.id), m.client] };
          case "client_left": return { ...c, clients: c.clients.filter((x) => x.id !== m.id) };
          case "client_updated": return { ...c, clients: c.clients.map((x) => (x.id === m.client.id ? m.client : x)) };
          case "channel_saved": return { ...c, channels: [...c.channels.filter((x) => x.id !== m.channel.id), m.channel] };
          case "channel_deleted": return { ...c, channels: c.channels.filter((x) => x.id !== m.id) };
          case "server_updated": return { ...c, server: m.server };
          default: return c;
        }
      });
      const me = connRef.current?.you;
      switch (m.type) {
        case "chat": {
          const t = m.target.to === "client" ? `p${m.from === me ? m.target.id : m.from}` : m.target.to;
          if (m.target.to === "client" && m.from !== me) setPrivates((p) => (p.some((x) => x.id === m.from) ? p : [...p, { id: m.from, name: m.from_name }]));
          add({ kind: "chat", tab: t, from: m.from_name, text: m.text, at: m.at_ms });
          break;
        }
        case "poked": add({ kind: "poke", tab: tabRef.current, from: m.from_name, text: m.text || "poked you" }); break;
        case "error": add({ kind: "error", tab: tabRef.current, text: m.message }); break;
        case "client_joined": add({ kind: "system", tab: "server", text: `${m.client.name} connected` }); break;
        case "client_left": add({ kind: "system", tab: "server", text: `${nameOf(m.id)} ${m.reason === "left" ? "left" : `left (${m.reason})`}` }); break;
        case "privilege_key": add({ kind: "system", tab: "server", text: `New ${m.role} key: ${m.key} (works once)` }); break;
      }
    });
    const unAudio = listen<{ from: number; seq: number; data: number[] }>("voice-audio", (p) => {
      engine.current?.receive(p.from, new Uint8Array(p.data));
    });
    const unClosed = listen<string>("voice-closed", (reason) => {
      setConn(null);
      engine.current?.stop();
      engine.current = null;
      setNote({ ok: false, text: reason });
    });
    const unPtt = listen<boolean>("voice-ptt", (down) => { if (engine.current && engine.current.ptt !== null) engine.current.ptt = down; });
    return () => { [unEvent, unAudio, unClosed, unPtt].forEach((u) => u.then((f) => f())); };
  }, [add]);
  const tabRef = useRef(tab);
  tabRef.current = tab;

  useEffect(() => {
    if (!conn) return;
    const t = window.setInterval(() => api.voicePing().then(setPing).catch(() => {}), 3000);
    return () => window.clearInterval(t);
  }, [!!conn]);

  useEffect(() => () => { engine.current?.stop(); }, []);

  const startAudio = async () => {
    setMicError(null);
    const e = new VoiceEngine({
      threshold: saved?.threshold ?? 0.02,
      onLevel: (l, s) => { setLevel(l); setSending(s); },
      onSpeaking: (who, on) => setSpeaking((x) => { const n = new Set(x); if (on) n.add(who); else n.delete(who); return n; }),
      onError: (m) => setMicError(m),
    });
    e.ptt = saved?.push_to_talk ? false : null;
    try {
      await e.start();
      engine.current = e;
    } catch (err) {
      setMicError(err instanceof Error ? err.message : `Microphone: ${String(err)}. You can still listen and chat.`);
      engine.current = e;
    }
  };

  const connect = async (address: string, name: string, password: string | null, trusted: string | null, label?: string) => {
    setBusy(true);
    setNote(null);
    try {
      const r = await api.voiceConnect(address, name, password, trusted);
      if (r.kind !== "connected") return setTrust({ result: r, address, name, password });
      const w = r.welcome;
      if (w.type !== "welcome") throw new Error("unexpected answer");
      setConn({ address, label: label ?? w.server.name, fingerprint: r.fingerprint, you: w.you, server: w.server, channels: w.channels, clients: w.clients });
      setLines([{ id: ++lineId, at: Date.now(), kind: "system", tab: "server", text: w.server.welcome || `Welcome to ${w.server.name}` }]);
      setTab("channel");
      setPrivates([]);
      setMuted(false);
      setDeafened(false);
      // Remember the server (and the fingerprint you trusted)
      if (saved) {
        const rest = saved.bookmarks.filter((b) => b.address !== address);
        const old = saved.bookmarks.find((b) => b.address === address);
        saveSettings({ ...saved, nickname: name, bookmarks: [{ label: old?.label || w.server.name, address, name, fingerprint: r.fingerprint, password: old?.password ?? null, last_channel: null }, ...rest] });
      }
      await startAudio();
    } catch (e) {
      setNote({ ok: false, text: String(e) });
    } finally {
      setBusy(false);
    }
  };

  const disconnect = async () => {
    await api.voiceDisconnect();
    await engine.current?.stop();
    engine.current = null;
    setConn(null);
  };

  const send = (m: VoiceClientMsg) => api.voiceSend(m).catch((e) => add({ kind: "error", tab, text: String(e) }));
  const me = conn?.clients.find((c) => c.id === conn.you);
  const setState = (m: boolean, d: boolean) => {
    setMuted(m);
    setDeafened(d);
    if (engine.current) { engine.current.muted = m; engine.current.deafened = d; }
    send({ type: "set_state", muted: m, deafened: d, away: me?.away ?? null });
  };

  if (!saved) return <div className="page"><h1>Voice</h1>{note && <div className="error">{note.text}</div>}</div>;

  return (
    <div className={`page voice-page${conn ? " full" : ""}`}>
      {!conn ? (
        <Lobby saved={saved} busy={busy} note={note} canHost={canHost} onAddKey={onAddKey} onSave={saveSettings} onConnect={connect} />
      ) : (
        <>
          <header className="voice-bar">
            <div className="voice-title">
              <h1>{conn.server.name}</h1>
              <span className="small muted">{conn.address} · {ping ? `${ping} ms` : "…"} · encrypted to the server</span>
            </div>
            <div className="toolbar">
              <span className={`mic-light${sending ? " on" : ""}`} title={saved.push_to_talk ? `Push-to-talk: ${saved.push_to_talk}` : "Voice activation"}>
                <span style={{ width: `${Math.min(100, level * 400)}%` }} />
              </span>
              <button className={muted ? "danger" : ""} aria-pressed={muted} onClick={() => setState(!muted, deafened)}><Icon name="mic" size={16} />{muted ? "Unmute" : "Mute"}</button>
              <button className={deafened ? "danger" : ""} aria-pressed={deafened} onClick={() => setState(!deafened, !deafened)}>{deafened ? "Undeafen" : "Deafen"}</button>
              <ServerMenu conn={conn} me={me} send={send} onError={(t) => add({ kind: "error", tab, text: t })} />
              <button className="quiet" onClick={disconnect}>Disconnect</button>
            </div>
          </header>
          {micError && <div className="notice warn"><Icon name="mic" /><div><strong>No microphone</strong><span>{micError}</span></div><button onClick={startAudio}>Try again</button></div>}
          <div className="voice-layout">
            <ChannelTree conn={conn} speaking={speaking} sendingMe={sending} send={send} engine={engine} onMessage={(c) => { setPrivates((p) => (p.some((x) => x.id === c.id) ? p : [...p, { id: c.id, name: c.name }])); setTab(`p${c.id}`); }} />
            <Chat conn={conn} lines={lines} tab={tab} setTab={setTab} privates={privates} onClosePrivate={(id) => { setPrivates((p) => p.filter((x) => x.id !== id)); setTab("channel"); }} send={send} />
          </div>
        </>
      )}
      {trust && (
        <TrustDialog
          trust={trust}
          onClose={() => setTrust(null)}
          onTrust={(fp) => { const t = trust; setTrust(null); connect(t.address, t.name, t.password, fp); }}
        />
      )}
    </div>
  );
}

function Lobby({ saved, busy, note, canHost, onAddKey, onSave, onConnect }: {
  saved: VoiceSaved; busy: boolean; note: { ok: boolean; text: string } | null; canHost: boolean; onAddKey: () => void;
  onSave: (s: VoiceSaved) => void; onConnect: (address: string, name: string, password: string | null, trusted: string | null, label?: string) => void;
}) {
  const [address, setAddress] = useState("");
  const [name, setName] = useState(saved.nickname);
  const [password, setPassword] = useState("");
  const [hosted, setHosted] = useState<VoiceHosted | null>(null);
  const [hostName, setHostName] = useState("My Sompio server");
  const [port, setPort] = useState(9987);
  const [hostError, setHostError] = useState<string | null>(null);
  useEffect(() => { api.voiceHostStatus().then(setHosted).catch(() => {}); }, []);
  const nick = name.trim() || "Guest";

  return (
    <>
      <div className="page-head">
        <div>
          <h1>Voice</h1>
          <p className="muted">Talk with friends on your own server, with channels, like TeamSpeak. Connect by IP or by your domain.</p>
        </div>
      </div>
      {note && <div className={note.ok ? "success" : "error"} role="status">{note.text}</div>}
      {!voiceSupported() && <div className="notice warn"><Icon name="mic" /><div><strong>Voice needs a newer WebView2.</strong><span>Run Windows Update, then restart Sompio. Chat still works.</span></div></div>}

      <div className="voice-lobby">
        <section className="panel voice-card">
          <h2>Connect to a server</h2>
          <form className="stack-form" onSubmit={(e) => { e.preventDefault(); const b = saved.bookmarks.find((x) => x.address === address.trim()); onConnect(address.trim(), nick, password || null, b?.fingerprint ?? null); }}>
            <label className="field"><span>Server address</span><input type="text" spellCheck={false} value={address} onChange={(e) => setAddress(e.target.value)} placeholder="voice.example.com or 203.0.113.5:9987" autoFocus /></label>
            <div className="field-row">
              <label className="field grow"><span>Your name there</span><input type="text" maxLength={40} value={name} onChange={(e) => setName(e.target.value)} placeholder="Sam" /></label>
              <label className="field grow"><span>Server password <span className="faint">(if it has one)</span></span><input type="password" value={password} onChange={(e) => setPassword(e.target.value)} /></label>
            </div>
            <div className="toolbar"><button className="primary" disabled={busy || !address.trim()}>{busy ? "Connecting…" : "Connect"}</button></div>
          </form>
          {saved.bookmarks.length > 0 && (
            <>
              <h3 className="small muted">Your servers</h3>
              <ul className="bookmarks">
                {saved.bookmarks.map((b) => (
                  <li key={b.address}>
                    <Icon name="mic" size={16} />
                    <span className="grow"><strong>{b.label}</strong><span className="small muted">{b.address} · as {b.name}</span></span>
                    <button className="small-btn primary" disabled={busy} onClick={() => onConnect(b.address, b.name, b.password, b.fingerprint, b.label)}>Connect</button>
                    <button className="small-btn ghost" onClick={() => onSave({ ...saved, bookmarks: saved.bookmarks.filter((x) => x.address !== b.address) })}>Remove</button>
                  </li>
                ))}
              </ul>
            </>
          )}
        </section>

        <section className="panel voice-card">
          <h2>Host a server on this PC</h2>
          {!canHost ? (
            <>
              <p className="small muted">Hosting needs Voice on your key. Joining other servers doesn't.</p>
              <div className="toolbar"><button onClick={onAddKey}>Add a key</button></div>
            </>
          ) : hosted ? (
            <>
              <p className="small muted">{hosted.name} is running while Sompio is open. Friends on your network connect to the address below; for friends elsewhere, forward UDP port {hosted.port} on your router, or run the server on a VPS.</p>
              <dl className="entry-meta small">
                {hosted.lan && <><dt>Address</dt><dd className="mono selectable">{hosted.lan}{hosted.port !== 9987 ? `:${hosted.port}` : ""}</dd></>}
                <dt>Fingerprint</dt><dd className="mono selectable">{hosted.fingerprint}</dd>
                {hosted.admin_key && <><dt>Admin key</dt><dd className="mono selectable">{hosted.admin_key}</dd></>}
              </dl>
              <div className="toolbar">
                <button className="primary" disabled={busy} onClick={() => onConnect(`127.0.0.1:${hosted.port}`, nick, null, hosted.fingerprint, hosted.name)}>Join it</button>
                <button onClick={async () => { await api.voiceHostStop(); setHosted(null); }}>Stop the server</button>
              </div>
              {hosted.admin_key && <p className="small muted">Once you've joined, use the admin key (server menu, Use an admin key) to become admin. It works once.</p>}
            </>
          ) : (
            <>
              <p className="small muted">Good for a quick session or friends at home. It runs while Sompio is open.</p>
              <div className="field-row">
                <label className="field grow"><span>Server name</span><input type="text" value={hostName} onChange={(e) => setHostName(e.target.value)} /></label>
                <label className="field"><span>UDP port</span><input type="number" min={1024} max={65535} value={port} onChange={(e) => setPort(Number(e.target.value))} /></label>
              </div>
              {hostError && <div className="error">{hostError}</div>}
              <div className="toolbar"><button onClick={async () => { setHostError(null); try { setHosted(await api.voiceHostStart(port, hostName)); } catch (e) { setHostError(String(e)); } }}>Start a server</button></div>
            </>
          )}
          <h3 className="small muted">Your own server, always on</h3>
          <p className="small muted">
            Put the Linux server on a VPS (in the "Sompio Voice server for Linux" folder): copy the folder over and run
            <span className="mono"> sudo sh install.sh</span>. Point a domain at it with an A record, like voice.yourdomain.com, and people connect to that.
          </p>
        </section>

        <AudioSettings saved={saved} onSave={onSave} />
      </div>
    </>
  );
}

function AudioSettings({ saved, onSave }: { saved: VoiceSaved; onSave: (s: VoiceSaved) => void }) {
  const [mics, setMics] = useState<MediaDeviceInfo[]>([]);
  const [testing, setTesting] = useState<VoiceEngine | null>(null);
  const [level, setLevel] = useState(0);
  const [open, setOpen] = useState(false);
  const [capturing, setCapturing] = useState(false);
  const [micError, setMicError] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => { microphones().then(setMics); }, []);
  useEffect(() => () => { testing?.stop(); }, [testing]);
  return (
    <section className="panel voice-card">
      <h2>Microphone</h2>
      <div className="inline-fields">
        <span>Talk with</span>
        <select value={saved.push_to_talk ? "ptt" : "va"} onChange={(e) => onSave({ ...saved, push_to_talk: e.target.value === "ptt" ? "F9" : "" })}>
          <option value="va">voice activation</option>
          <option value="ptt">push-to-talk</option>
        </select>
        {saved.push_to_talk && (
          <input ref={input} type="text" readOnly className="shortcut-input mono" value={capturing ? "Press a key…" : saved.push_to_talk}
            onFocus={() => setCapturing(true)} onBlur={() => setCapturing(false)}
            onKeyDown={(e) => {
              e.preventDefault();
              if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return;
              const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift"].filter(Boolean) as string[];
              const key = e.key.length === 1 ? e.key.toUpperCase() : /^F\d+$/.test(e.key) ? e.key : e.code.replace(/^Key|^Digit/, "");
              onSave({ ...saved, push_to_talk: [...mods, key].join("+") });
              input.current?.blur();
            }} />
        )}
      </div>
      {saved.push_to_talk ? (
        <p className="small muted">Hold {saved.push_to_talk} to talk, even while a game has focus.</p>
      ) : (
        <>
          <label className="inline-fields"><span>Start sending above</span>
            <input type="range" min={0.002} max={0.1} step={0.002} value={saved.threshold} onChange={(e) => { const t = Number(e.target.value); onSave({ ...saved, threshold: t }); if (testing) testing.threshold = t; }} />
          </label>
          <div className="level-meter" aria-label="Microphone level">
            <span className="level" style={{ width: `${Math.min(100, level * 1000)}%` }} />
            <span className="mark" style={{ left: `${Math.min(100, saved.threshold * 1000)}%` }} />
          </div>
        </>
      )}
      <div className="toolbar">
        {!testing ? (
          <button className="small-btn" disabled={!voiceSupported()} onClick={async () => {
            const e = new VoiceEngine({ threshold: saved.threshold, onLevel: (l) => setLevel(l) });
            setMicError(null);
            try { await e.start(); setTesting(e); microphones().then(setMics); } catch (err) { setLevel(0); setMicError(err instanceof Error ? err.message : String(err)); }
          }}>Test my microphone</button>
        ) : (
          <button className="small-btn" onClick={() => { testing.stop(); setTesting(null); setLevel(0); }}>Stop the test</button>
        )}
        <button type="button" className="link" onClick={() => setOpen(!open)}>{open ? "Hide" : "Microphones"}</button>
      </div>
      {micError && <div className="error">{micError.includes("Permission") || micError.includes("NotAllowed") ? "Sompio isn't allowed to use the microphone. Allow it in Windows Settings, Privacy, Microphone." : micError}</div>}
      {open && <ul className="small muted">{mics.length ? mics.map((m) => <li key={m.deviceId}>{m.label || "Microphone"}</li>) : <li>Allow the microphone once (Test my microphone) to see their names.</li>}</ul>}
      <p className="small faint">Echo cancellation, noise suppression and automatic volume are on. Sompio uses your default microphone; change it in Windows sound settings.</p>
    </section>
  );
}

function TrustDialog({ trust, onClose, onTrust }: { trust: Trust; onClose: () => void; onTrust: (fp: string) => void }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => { ref.current?.showModal(); }, []);
  const r = trust.result;
  return (
    <dialog ref={ref} className="sheet" onClose={onClose}>
      <div className="sheet-body">
        {r.kind === "new_server" ? (
          <>
            <h2>First time on {r.name}</h2>
            <p className="muted">
              Servers prove who they are with a fingerprint. Ask the person who runs this one whether theirs matches. If it does,
              nobody can pretend to be this server later.
            </p>
            <p className="fingerprint mono selectable">{r.fingerprint}</p>
            <div className="toolbar end">
              <button className="quiet" onClick={() => ref.current?.close()}>Cancel</button>
              <button className="primary" onClick={() => onTrust(r.fingerprint)}>It matches, connect</button>
            </div>
          </>
        ) : (
          <>
            <h2>This server's fingerprint changed</h2>
            <div className="notice warn"><Icon name="lock" /><div><strong>Someone may be pretending to be this server.</strong><span>Or the owner set it up again. Only continue if they told you so.</span></div></div>
            <dl className="entry-meta small">
              <dt>You trusted</dt><dd className="mono">{r.expected}</dd>
              <dt>It shows now</dt><dd className="mono">{r.got}</dd>
            </dl>
            <div className="toolbar end">
              <button className="primary" onClick={() => ref.current?.close()}>Don't connect</button>
              <button className="quiet danger" onClick={() => onTrust(r.got)}>Trust the new fingerprint</button>
            </div>
          </>
        )}
      </div>
    </dialog>
  );
}

function ChannelTree({ conn, speaking, sendingMe, send, engine, onMessage }: {
  conn: Conn; speaking: Set<number>; sendingMe: boolean; send: (m: VoiceClientMsg) => void;
  engine: React.MutableRefObject<VoiceEngine | null>; onMessage: (c: VoiceClient) => void;
}) {
  const me = conn.clients.find((c) => c.id === conn.you);
  const mod = me ? me.role !== "member" : false;
  const [askPassword, setAskPassword] = useState<VoiceChannel | null>(null);
  const [pw, setPw] = useState("");
  const kids = useMemo(() => {
    const m = new Map<number | null, VoiceChannel[]>();
    for (const c of conn.channels) m.set(c.parent, [...(m.get(c.parent) ?? []), c]);
    for (const l of m.values()) l.sort((a, b) => a.order - b.order || a.name.localeCompare(b.name));
    return m;
  }, [conn.channels]);
  const join = (c: VoiceChannel) => {
    if (c.id === me?.channel) return;
    if (c.has_password && !mod) { setPw(""); setAskPassword(c); } else send({ type: "join", channel: c.id, password: null });
  };
  const render = (parent: number | null, depth: number): React.ReactNode =>
    (kids.get(parent) ?? []).map((c) => {
      const people = conn.clients.filter((x) => x.channel === c.id).sort((a, b) => a.name.localeCompare(b.name));
      const here = me?.channel === c.id;
      return (
        <li key={c.id}>
          <div className={`chan${here ? " here" : ""}`} style={{ paddingLeft: `${0.6 + depth * 1}rem` }} onDoubleClick={() => join(c)} title={c.topic || undefined}>
            <span className="chan-name">{c.name}</span>
            {c.has_password && <Icon name="lock" size={13} />}
            {!c.permanent && <span className="faint small">temporary</span>}
            {c.max_users > 0 && <span className="faint small num">{people.length}/{c.max_users}</span>}
            <span className="chan-actions">
              {!here && <button className="small-btn ghost" onClick={() => join(c)}>Join</button>}
              {mod && <ChannelMenu c={c} send={send} isDefault={c.id === conn.server.default_channel} />}
            </span>
          </div>
          {people.length > 0 && (
            <ul className="people">
              {people.map((p) => (
                <Person key={p.id} p={p} me={me} isMe={p.id === conn.you} talking={p.id === conn.you ? sendingMe && !p.muted : speaking.has(p.id)}
                  depth={depth} conn={conn} send={send} engine={engine} onMessage={onMessage} />
              ))}
            </ul>
          )}
          <ul>{render(c.id, depth + 1)}</ul>
        </li>
      );
    });
  return (
    <section className="panel channel-tree" aria-label="Channels">
      <ul>{render(null, 0)}</ul>
      <p className="small faint tree-hint">Double-click a channel to join it.</p>
      {askPassword && (
        <form className="panel inset-form" onSubmit={(e) => { e.preventDefault(); send({ type: "join", channel: askPassword.id, password: pw }); setAskPassword(null); }}>
          <label className="field"><span>Password for {askPassword.name}</span><input type="password" autoFocus value={pw} onChange={(e) => setPw(e.target.value)} /></label>
          <div className="toolbar"><button className="primary">Join</button><button type="button" className="quiet" onClick={() => setAskPassword(null)}>Cancel</button></div>
        </form>
      )}
    </section>
  );
}

function Person({ p, me, isMe, talking, depth, send, engine, onMessage }: {
  p: VoiceClient; me: VoiceClient | undefined; isMe: boolean; talking: boolean; depth: number; conn: Conn;
  send: (m: VoiceClientMsg) => void; engine: React.MutableRefObject<VoiceEngine | null>; onMessage: (c: VoiceClient) => void;
}) {
  const [open, setOpen] = useState(false);
  const [vol, setVol] = useState(() => engine.current?.volumes.get(p.id) ?? 1);
  const canMod = !!me && me.role !== "member" && !isMe;
  const admin = me?.role === "admin" && !isMe;
  const act = (m: VoiceClientMsg) => { setOpen(false); send(m); };
  return (
    <li className={`person${talking ? " talking" : ""}`} style={{ paddingLeft: `${1.4 + depth * 1}rem` }}>
      <span className="talk-dot" aria-label={talking ? "talking" : undefined} />
      <span className="person-name">{p.name}{isMe && <span className="faint"> (you)</span>}</span>
      {ROLE[p.role] && <span className={`flag role-${p.role}`}>{ROLE[p.role]}</span>}
      {p.deafened ? <span className="faint small" title="Deafened">deafened</span> : p.muted ? <span className="faint small" title="Muted">muted</span> : null}
      {p.away && <span className="faint small">away</span>}
      {!isMe && (
        <div className="menu-wrap person-menu">
          <button className="small-btn ghost" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(!open)}>⋯</button>
          {open && (
            <div className="menu" role="menu" onMouseLeave={() => setOpen(false)}>
              <button role="menuitem" onClick={() => { setOpen(false); onMessage(p); }}>Message</button>
              <button role="menuitem" onClick={() => act({ type: "poke", client: p.id, text: "" })}>Poke</button>
              <label className="menu-slider small">Volume
                <input type="range" min={0} max={2} step={0.05} value={vol} onChange={(e) => { const v = Number(e.target.value); setVol(v); engine.current?.volumes.set(p.id, v); }} />
              </label>
              {canMod && <button role="menuitem" onClick={() => act({ type: "move", client: p.id, channel: me!.channel })}>Move to my channel</button>}
              {canMod && <button role="menuitem" onClick={() => act({ type: "kick", client: p.id, reason: "", from_server: false })}>Kick from channel</button>}
              {canMod && <button role="menuitem" onClick={() => act({ type: "kick", client: p.id, reason: "", from_server: true })}>Kick from server</button>}
              {canMod && <button role="menuitem" onClick={() => act({ type: "ban", client: p.id, reason: "", minutes: 60 * 24 })}>Ban for a day</button>}
              {admin && p.role !== "moderator" && <button role="menuitem" onClick={() => act({ type: "set_role", client: p.id, role: "moderator" })}>Make moderator</button>}
              {admin && p.role !== "admin" && <button role="menuitem" onClick={() => act({ type: "set_role", client: p.id, role: "admin" })}>Make admin</button>}
              {admin && p.role !== "member" && <button role="menuitem" onClick={() => act({ type: "set_role", client: p.id, role: "member" })}>Make member</button>}
              <span className="small faint menu-note">Key {p.identity}</span>
            </div>
          )}
        </div>
      )}
    </li>
  );
}

function ChannelMenu({ c, send, isDefault }: { c: VoiceChannel; send: (m: VoiceClientMsg) => void; isDefault: boolean }) {
  const [open, setOpen] = useState(false);
  const [edit, setEdit] = useState<{ name: string; topic: string; password: string; max: number } | null>(null);
  if (edit)
    return (
      <form className="panel inset-form chan-edit" onSubmit={(e) => { e.preventDefault(); send({ type: "edit_channel", id: c.id, name: edit.name, topic: edit.topic, password: edit.password === "" && c.has_password ? undefined : edit.password, max_users: edit.max }); setEdit(null); }}>
        <input type="text" value={edit.name} onChange={(e) => setEdit({ ...edit, name: e.target.value })} placeholder="Name" />
        <input type="text" value={edit.topic} onChange={(e) => setEdit({ ...edit, topic: e.target.value })} placeholder="Topic" />
        <input type="password" value={edit.password} onChange={(e) => setEdit({ ...edit, password: e.target.value })} placeholder={c.has_password ? "New password (empty keeps it)" : "Password (optional)"} />
        <div className="toolbar"><button className="small-btn primary">Save</button>{c.has_password && <button type="button" className="small-btn" onClick={() => { send({ type: "edit_channel", id: c.id, name: edit.name, topic: edit.topic, password: "", max_users: edit.max }); setEdit(null); }}>Remove password</button>}<button type="button" className="small-btn ghost" onClick={() => setEdit(null)}>Cancel</button></div>
      </form>
    );
  return (
    <div className="menu-wrap">
      <button className="small-btn ghost" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(!open)}>⋯</button>
      {open && (
        <div className="menu" role="menu" onMouseLeave={() => setOpen(false)}>
          <button role="menuitem" onClick={() => { setOpen(false); setEdit({ name: c.name, topic: c.topic, password: "", max: c.max_users }); }}>Edit</button>
          <button role="menuitem" onClick={() => { setOpen(false); send({ type: "create_channel", name: "New channel", parent: c.id, topic: "", password: null, permanent: true, max_users: 0 }); }}>Add a sub-channel</button>
          {!isDefault && <button role="menuitem" onClick={() => { setOpen(false); send({ type: "delete_channel", id: c.id }); }}>Delete</button>}
        </div>
      )}
    </div>
  );
}

function ServerMenu({ conn, me, send, onError }: { conn: Conn; me: VoiceClient | undefined; send: (m: VoiceClientMsg) => void; onError: (t: string) => void }) {
  const [open, setOpen] = useState(false);
  const [form, setForm] = useState<"channel" | "key" | "server" | null>(null);
  const [a, setA] = useState("");
  const [b, setB] = useState("");
  const [c, setC] = useState("");
  const [perm, setPerm] = useState(false);
  const ref = useRef<HTMLDialogElement>(null);
  const role = me?.role ?? "member";
  useEffect(() => { if (form) ref.current?.showModal(); }, [form]);
  const show = (f: typeof form, x = "", y = "") => { setOpen(false); setA(x); setB(y); setC(""); setPerm(role !== "member"); setForm(f); };
  return (
    <div className="menu-wrap">
      <button aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(!open)}>Server ▾</button>
      {open && (
        <div className="menu" role="menu" onMouseLeave={() => setOpen(false)}>
          <button role="menuitem" onClick={() => show("channel")}>Create a channel</button>
          <button role="menuitem" onClick={() => show("key")}>Use an admin key</button>
          {role === "admin" && <button role="menuitem" onClick={() => { setOpen(false); send({ type: "make_privilege_key", role: "admin" }); }}>Make an admin key</button>}
          {role === "admin" && <button role="menuitem" onClick={() => { setOpen(false); send({ type: "make_privilege_key", role: "moderator" }); }}>Make a moderator key</button>}
          {role === "admin" && <button role="menuitem" onClick={() => show("server", conn.server.name, conn.server.welcome)}>Server name and welcome</button>}
          <span className="small faint menu-note">Fingerprint {conn.fingerprint}</span>
        </div>
      )}
      {form && (
        <dialog ref={ref} className="sheet" onClose={() => setForm(null)}>
          <form className="sheet-body" onSubmit={(e) => {
            e.preventDefault();
            if (form === "channel") send({ type: "create_channel", name: a, parent: null, topic: b, password: c || null, permanent: perm, max_users: 0 });
            else if (form === "key") send({ type: "use_privilege_key", key: a });
            else send({ type: "edit_server", name: a, welcome: b });
            if (!a.trim() && form !== "server") return onError("Fill in the first box.");
            ref.current?.close();
          }}>
            <h2>{form === "channel" ? "Create a channel" : form === "key" ? "Use an admin key" : "Server name and welcome"}</h2>
            <label className="field"><span>{form === "channel" ? "Name" : form === "key" ? "Key" : "Server name"}</span><input type="text" autoFocus value={a} onChange={(e) => setA(e.target.value)} className={form === "key" ? "mono" : ""} placeholder={form === "key" ? "XXXX-XXXX-XXXX-XXXX-XXXX" : ""} /></label>
            {form !== "key" && <label className="field"><span>{form === "channel" ? "Topic (optional)" : "Welcome message"}</span><input type="text" value={b} onChange={(e) => setB(e.target.value)} /></label>}
            {form === "channel" && <label className="field"><span>Password (optional)</span><input type="password" value={c} onChange={(e) => setC(e.target.value)} /></label>}
            {form === "channel" && (
              <label className="check"><input type="checkbox" checked={perm} disabled={role === "member"} onChange={(e) => setPerm(e.target.checked)} />
                <span>Keep it when it's empty{role === "member" && <small className="faint">Only moderators and admins make lasting channels. Yours closes when everyone leaves.</small>}</span></label>
            )}
            {form === "key" && <p className="small muted">The server owner got one when the server first started. It makes you admin (or moderator) on this server, and works once.</p>}
            <div className="toolbar end"><button type="button" className="quiet" onClick={() => ref.current?.close()}>Cancel</button><button className="primary">{form === "key" ? "Use it" : "Save"}</button></div>
          </form>
        </dialog>
      )}
    </div>
  );
}

function Chat({ conn, lines, tab, setTab, privates, onClosePrivate, send }: {
  conn: Conn; lines: Line[]; tab: string; setTab: (t: string) => void; privates: { id: number; name: string }[];
  onClosePrivate: (id: number) => void; send: (m: VoiceClientMsg) => void;
}) {
  const [text, setText] = useState("");
  const list = useRef<HTMLDivElement>(null);
  const me = conn.clients.find((c) => c.id === conn.you);
  const channel = conn.channels.find((c) => c.id === me?.channel);
  const shown = lines.filter((l) => l.tab === tab);
  useEffect(() => { list.current?.scrollTo({ top: list.current.scrollHeight }); }, [shown.length, tab]);
  const target: ChatTarget = tab === "server" ? { to: "server" } : tab === "channel" ? { to: "channel" } : { to: "client", id: Number(tab.slice(1)) };
  return (
    <section className="panel chat" aria-label="Chat">
      <div className="chat-tabs" role="tablist">
        <button role="tab" aria-selected={tab === "channel"} onClick={() => setTab("channel")}>{channel?.name ?? "Channel"}</button>
        <button role="tab" aria-selected={tab === "server"} onClick={() => setTab("server")}>Server</button>
        {privates.map((p) => (
          <span key={p.id} className="chat-tab-private">
            <button role="tab" aria-selected={tab === `p${p.id}`} onClick={() => setTab(`p${p.id}`)}>{p.name}</button>
            <button className="quiet" aria-label={`Close chat with ${p.name}`} onClick={() => onClosePrivate(p.id)}>×</button>
          </span>
        ))}
      </div>
      {channel?.topic && tab === "channel" && <div className="chat-topic small muted">{channel.topic}</div>}
      <div className="chat-lines" ref={list}>
        {shown.length === 0 && <p className="small faint">{tab === "channel" ? "Messages here go to everyone in this channel." : tab === "server" ? "Messages here go to everyone on the server." : "Only the two of you see this."}</p>}
        {shown.map((l) => (
          <div key={l.id} className={`line ${l.kind}`}>
            <span className="faint small num">{time(l.at)}</span>
            {l.kind === "chat" ? <><strong>{l.from}</strong><span className="selectable">{l.text}</span></> : l.kind === "poke" ? <span><strong>{l.from}</strong> poked you{l.text && l.text !== "poked you" ? `: ${l.text}` : ""}</span> : <span>{l.text}</span>}
          </div>
        ))}
      </div>
      <form className="chat-input" onSubmit={(e) => { e.preventDefault(); if (text.trim()) { send({ type: "chat", target, text }); setText(""); } }}>
        <input type="text" maxLength={2000} value={text} onChange={(e) => setText(e.target.value)} placeholder={tab === "server" ? "Message everyone" : tab === "channel" ? `Message ${channel?.name ?? "the channel"}` : "Private message"} />
        <button className="primary" disabled={!text.trim()}>Send</button>
      </form>
    </section>
  );
}

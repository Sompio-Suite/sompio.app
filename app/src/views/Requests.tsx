// Trace's request centre: get your data from each platform, look for things right away, ask
// companies to delete or hand over what they hold, and keep track of the one-month deadlines.
// Sompio never sends anything itself: letters open in your own email app or the company's form.

import { useEffect, useMemo, useState } from "react";
import { api, copyText, openExternal, type PrivacyRequest, type SourceView, type TraceData } from "../api";
import type { View } from "../App";
import Icon from "../Icon";
import {
  EXPORTS, KIND_LABEL, KIND_SHORT, addDays, addMonth, complaintText, daysUntil, letter, loadAuthorities, loadCompanies,
  mailto, niceDate, reminderLetter, searchCompanies, searches, today, type Authority, type Company, type RequestKind,
} from "../trace";

type Section = "get" | "find" | "ask" | "track";

const QUICK = ["discord", "reddit", "google", "facebook", "instagram", "whatsapp", "tiktok", "twitter", "snapchat", "linkedin", "microsoft", "telegram"];
const BROKER_CATS = ["addresses", "credit agency", "ads", "collection agency"];
const EMPTY: TraceData = { profile: { name: "", email: "", country: "", handles: {} }, exports: {}, requests: [] };

const countryName = (() => {
  try {
    const d = new Intl.DisplayNames(["en"], { type: "region" });
    return (c: string) => d.of(c.toUpperCase()) ?? c.toUpperCase();
  } catch {
    return (c: string) => c.toUpperCase();
  }
})();

export default function Requests({ readOnly, onGo }: { readOnly: boolean; onGo: (v: View) => void }) {
  const [section, setSection] = useState<Section>("get");
  const [data, setData] = useState<TraceData | null>(null);
  const [sources, setSources] = useState<SourceView[]>([]);
  const [connected, setConnected] = useState<{ telegram: boolean; email: boolean }>({ telegram: false, email: false });
  const [terms, setTerms] = useState<string[]>([]);
  const [toast, setToast] = useState<string | null>(null);
  const [remind, setRemind] = useState(true);
  const [authorities, setAuthorities] = useState<Authority[]>([]);

  useEffect(() => {
    api.traceData().then((d) => setData({ ...EMPTY, ...d, profile: { ...EMPTY.profile, ...d.profile, handles: d.profile?.handles ?? {} } }));
    api.sources().then(setSources).catch(() => {});
    api.watchTerms().then(setTerms).catch(() => {});
    Promise.all([api.tgStatus().catch(() => null), api.emailStatus().catch(() => null)]).then(([t, e]) =>
      setConnected({ telegram: !!t?.connected, email: !!e?.connected }),
    );
    loadAuthorities().then(setAuthorities).catch(() => {});
  }, []);
  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 6000);
    return () => clearTimeout(t);
  }, [toast]);

  const save = async (next: TraceData, text?: string) => {
    if (readOnly) return setToast("Trace is read-only without a licence.");
    try {
      await api.traceSave(next);
      setData(next);
      if (text) setToast(text);
    } catch (e) {
      setToast(String(e));
    }
  };
  const reminder = async (date: string, title: string, text: string) => {
    if (!remind) return "";
    try {
      await api.traceRemind(date, title, text);
      return ` Calendar will remind you on ${niceDate(date)}.`;
    } catch {
      return " The Calendar reminder couldn't be added.";
    }
  };

  if (!data) return <div className="xc-page"><div className="empty">Loading…</div></div>;

  const waiting = data.requests.filter((r) => r.status === "waiting");
  const overdue = waiting.filter((r) => r.sent_on && daysUntil(addMonth(r.sent_on)) < 0).length;
  const nav: [Section, string, string, number | null][] = [
    ["get", "Get your data", "sources", null],
    ["find", "Find it now", "search", null],
    ["ask", "Ask a company", "mail", null],
    ["track", "Your requests", "clock", data.requests.length || null],
  ];

  return (
    <div className="xc-page requests-page">
      <div className="xc-body">
        <aside className="xc-tree" aria-label="Requests">
          {nav.map(([id, label, icon, count]) => (
            <button key={id} className="xc-node root" aria-current={section === id} onClick={() => setSection(id)}>
              <Icon name={icon as "mail"} size={16} />
              <span>{label}</span>
              {count != null && <span className="num">{count}</span>}
            </button>
          ))}
          <div className="xc-tree-foot small muted">
            {overdue > 0 ? <strong className="overdue-text">{overdue} past the deadline</strong> : waiting.length > 0 ? `${waiting.length} waiting for an answer` : null}
            <label className="switch small">
              <input type="checkbox" checked={remind} onChange={(e) => setRemind(e.target.checked)} />
              Calendar reminders
            </label>
          </div>
        </aside>
        <div className="req-pane">
          {section === "get" && (
            <GetData data={data} sources={sources} connected={connected} onGo={onGo} remind={reminder} save={save} />
          )}
          {section === "find" && <FindNow data={data} terms={terms} save={save} onGo={onGo} />}
          {section === "ask" && (
            <AskCompany data={data} save={save} remind={reminder} onTracked={() => setSection("track")} />
          )}
          {section === "track" && (
            <Track data={data} authorities={authorities} save={save} remind={reminder} onAsk={() => setSection("ask")} />
          )}
        </div>
      </div>
      {toast && (
        <div className="toast" role="status" aria-live="polite">
          <span>{toast}</span>
          <button className="toast-close" aria-label="Dismiss" onClick={() => setToast(null)}>×</button>
        </div>
      )}
    </div>
  );
}

type Save = (d: TraceData, text?: string) => Promise<void>;
type Remind = (date: string, title: string, text: string) => Promise<string>;

// ---------- Get your data ----------

function GetData(p: {
  data: TraceData; sources: SourceView[]; connected: { telegram: boolean; email: boolean };
  onGo: (v: View) => void; remind: Remind; save: Save;
}) {
  const imported = new Set(p.sources.map((s) => s.platform));
  const asked = async (key: string, name: string, days: number) => {
    const on = today();
    const extra = days > 0 ? await p.remind(addDays(on, days), `Check your ${name} data`, `Your ${name} data export should be ready. Download it and import it in Trace, under Sources.`) : "";
    p.save({ ...p.data, exports: { ...p.data.exports, [key]: { requested_on: on } } }, `Noted.${extra}`);
  };
  const undo = (key: string) => {
    const exports = { ...p.data.exports };
    delete exports[key];
    p.save({ ...p.data, exports });
  };
  return (
    <div className="req-section">
      <header>
        <h2>Get your data</h2>
        <p className="muted">
          Every platform must give you a copy of what it holds about you. Ask for it on each platform you use, then import the download
          under <button className="link" onClick={() => p.onGo("sources")}>Sources</button>. Most arrive within days. Telegram and email can be
          connected for results right away.
        </p>
      </header>
      <div className="req-list">
        {EXPORTS.map((e) => {
          const done = imported.has(e.platform);
          const isOn = e.platform === "telegram" ? p.connected.telegram : e.platform === "email" ? p.connected.email : false;
          const on = p.data.exports[e.key]?.requested_on;
          const ready = on ? addDays(on, e.days) : null;
          return (
            <div key={e.key} className={`req-row${done ? " is-done" : ""}`}>
              <div className="req-main">
                <div className="req-title">
                  <strong>{e.name}</strong>
                  {done && <span className="state ok"><Icon name="check" size={13} />Imported</span>}
                  {isOn && <span className="state ok"><Icon name="check" size={13} />Connected</span>}
                  {!done && on && (
                    <span className="state wait">
                      Asked {niceDate(on)}{ready && daysUntil(ready) > 0 ? `, ready around ${niceDate(ready)}` : ", should be ready"}
                    </span>
                  )}
                </div>
                <p className="small">{e.how}</p>
                <p className="small muted">Usual wait: {e.wait}</p>
              </div>
              <div className="req-actions">
                {e.live && !isOn && (
                  <button className="primary" onClick={() => p.onGo("sources")}><Icon name="arrow" size={16} />Connect now</button>
                )}
                {e.url && (
                  <button onClick={() => openExternal(e.url!)}><Icon name="external" size={16} />Open the page</button>
                )}
                {!done && !e.live && (on ? (
                  <button className="quiet" onClick={() => undo(e.key)}>Not asked yet</button>
                ) : (
                  <button onClick={() => asked(e.key, e.name, e.days)}><Icon name="check" size={16} />I asked for it</button>
                ))}
                {done && <button className="quiet" onClick={() => p.onGo("sources")}>Sources</button>}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ---------- Find it now ----------

function FindNow(p: { data: TraceData; terms: string[]; save: Save; onGo: (v: View) => void }) {
  const [handles, setHandles] = useState(p.data.profile.handles);
  const [extra, setExtra] = useState("");
  const [copied, setCopied] = useState<string | null>(null);
  const words = useMemo(() => {
    const all = [...p.terms, ...(p.data.profile.email ? [p.data.profile.email] : []), ...(extra.trim() ? [extra.trim()] : [])];
    return [...new Set(all)];
  }, [p.terms, p.data.profile.email, extra]);
  const rows = searches(words, handles);
  const changed = JSON.stringify(handles) !== JSON.stringify(p.data.profile.handles);
  const copy = async (text: string) => {
    await copyText(text);
    setCopied(text);
    setTimeout(() => setCopied(null), 2000);
  };
  return (
    <div className="req-section">
      <header>
        <h2>Find it now</h2>
        <p className="muted">
          No need to wait for an export to start. These are ready-made searches for your watch terms on the platforms themselves, limited to
          your own posts when you add your username. Delete what you find there.
        </p>
        <p className="small privacy-note">
          <Icon name="lock" size={14} /> Searching happens on that platform's website, so the platform sees what you search for, just as if you
          typed it there. Discord searches are copied for you to paste into Discord (Ctrl+F).
        </p>
      </header>
      <div className="handles">
        {(["discord", "reddit", "x"] as const).map((k) => (
          <label key={k} className="field">
            <span>Your {k === "x" ? "X" : k === "discord" ? "Discord" : "Reddit"} username</span>
            <input type="text" value={handles[k] ?? ""} placeholder={k === "x" ? "@name" : k === "reddit" ? "u/name" : "name"} onChange={(e) => setHandles({ ...handles, [k]: e.target.value.trim() })} />
          </label>
        ))}
        <button disabled={!changed} onClick={() => p.save({ ...p.data, profile: { ...p.data.profile, handles } }, "Usernames saved.")}>Save</button>
      </div>
      <label className="field narrow">
        <span>Search for something else too</span>
        <input type="text" value={extra} onChange={(e) => setExtra(e.target.value)} placeholder="An old address, a nickname…" />
      </label>
      {words.length === 0 ? (
        <div className="empty small">
          Add watch terms (your name, street, school) on the Findings page first, or type something above.
          <button onClick={() => p.onGo("review")}>Go to Findings</button>
        </div>
      ) : (
        <div className="table-wrap">
          <table className="req-table">
            <thead><tr><th>Looking for</th><th>Discord</th><th>Reddit</th><th>X</th><th>The web</th></tr></thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.term}>
                  <td><strong className="selectable">{r.term}</strong></td>
                  <td><button className="small-btn" onClick={() => copy(r.discord)} title={r.discord}><Icon name="copy" size={14} />{copied === r.discord ? "Copied" : "Copy search"}</button></td>
                  <td><button className="small-btn" onClick={() => openExternal(r.reddit)}><Icon name="external" size={14} />Search</button></td>
                  <td><button className="small-btn" onClick={() => openExternal(r.x)}><Icon name="external" size={14} />Search</button></td>
                  <td><button className="small-btn" onClick={() => openExternal(r.web)}><Icon name="external" size={14} />DuckDuckGo</button></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <p className="small muted">
        Telegram and email go further: connect them under <button className="link" onClick={() => p.onGo("sources")}>Sources</button>, and Trace
        checks every message you've sent in minutes, then deletes what you pick straight from Findings.
      </p>
    </div>
  );
}

// ---------- Ask a company ----------

function AskCompany(p: { data: TraceData; save: Save; remind: Remind; onTracked: () => void }) {
  const [all, setAll] = useState<Company[]>([]);
  const [q, setQ] = useState("");
  const [brokers, setBrokers] = useState(false);
  const [picked, setPicked] = useState<Company | null>(null);
  const [custom, setCustom] = useState(false);
  const [kind, setKind] = useState<RequestKind>("erasure");
  const [name, setName] = useState(p.data.profile.name);
  const [account, setAccount] = useState(p.data.profile.email);
  const [subject, setSubject] = useState("");
  const [body, setBody] = useState("");
  const [edited, setEdited] = useState(false);
  const [customName, setCustomName] = useState("");
  const [customEmail, setCustomEmail] = useState("");

  useEffect(() => {
    loadCompanies().then(setAll).catch(() => {});
  }, []);
  const companyName = custom ? customName : picked?.n ?? "";
  useEffect(() => {
    if (edited || (!picked && !custom)) return;
    const l = letter({ kind, company: companyName || "[company]", name, account, date: niceDate(today()) });
    setSubject(l.subject);
    setBody(l.body);
  }, [kind, name, account, picked, custom, companyName, edited]);

  const country = p.data.profile.country;
  const results = q.trim().length >= 2
    ? searchCompanies(all, q)
    : brokers
      ? all.filter((c) => c.k?.some((k) => BROKER_CATS.includes(k)) && (!country || !c.c || c.c.includes(country))).slice(0, 40)
      : [];
  const quick = QUICK.map((s) => all.find((c) => c.s === s)).filter(Boolean) as Company[];
  const to = custom ? customEmail.trim() : picked?.e ?? "";
  const pick = (c: Company | null, isCustom = false) => {
    setPicked(c);
    setCustom(isCustom);
    setEdited(false);
  };

  const track = async () => {
    const on = today();
    const r: PrivacyRequest = {
      id: `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`,
      company: custom ? "" : picked!.s, company_name: companyName.trim() || "Unnamed company", kind, account,
      sent_on: on, status: "waiting", reminders: [], complained_on: null, authority: "", note: "",
    };
    const profile = { ...p.data.profile, name: name || p.data.profile.name, email: p.data.profile.email || account };
    const extra = await p.remind(addMonth(on), `GDPR deadline: ${r.company_name}`, `${r.company_name} had one month to answer your ${KIND_SHORT[kind].toLowerCase()} request. If they haven't, open Trace → Requests to send a reminder.`);
    await p.save({ ...p.data, profile, requests: [r, ...p.data.requests] }, `Added to your requests. The deadline is ${niceDate(addMonth(on))}.${extra}`);
    pick(null);
    setQ("");
    p.onTracked();
  };

  if (!picked && !custom) {
    return (
      <div className="req-section">
        <header>
          <h2>Ask a company</h2>
          <p className="muted">
            Under the GDPR, any company must delete your data, send you a copy of it, or stop using it when you ask, and answer within one month.
            Trace writes the letter. You send it from your own email or the company's form, and Trace tracks the deadline.
          </p>
        </header>
        <div className="company-search">
          <span className="search-box"><Icon name="search" size={16} />
            <input type="text" autoFocus value={q} onChange={(e) => { setQ(e.target.value); setBrokers(false); }} placeholder={`Search ${all.length ? all.length.toLocaleString() : ""} companies by name or website`} aria-label="Search companies" />
          </span>
          <button aria-pressed={brokers} onClick={() => { setBrokers(!brokers); setQ(""); }} title="Companies that collect and sell addresses, credit and ad profiles">
            Data brokers{country ? ` in ${countryName(country)}` : ""}
          </button>
          <button className="quiet" onClick={() => pick(null, true)}>Not listed? Write to any company</button>
        </div>
        {results.length > 0 ? (
          <div className="company-list">
            {results.map((c) => <CompanyRow key={c.s} c={c} onPick={() => pick(c)} />)}
          </div>
        ) : q.trim().length >= 2 ? (
          <div className="empty small">No company found. <button onClick={() => { setCustomName(q); pick(null, true); }}>Write to "{q}" anyway</button></div>
        ) : (
          <>
            <p className="section-title">Often asked</p>
            <div className="quick-picks">
              {quick.map((c) => <button key={c.s} onClick={() => pick(c)}>{c.s === "twitter" ? "X (Twitter)" : c.n.split(/ (Netherlands|Ireland|Platforms|Technology|UK|International|B\.?V|LLC|Ltd|Limited|Operations)/)[0]}</button>)}
            </div>
          </>
        )}
        <p className="small muted">Company directory: Datenanfragen.de (public domain), bundled with Sompio. Searching it never goes online.</p>
      </div>
    );
  }

  return (
    <div className="req-section">
      <header className="row-head">
        <button className="quiet" onClick={() => pick(null)}>← Back</button>
        <h2>{custom ? "Write to a company" : picked!.n}</h2>
      </header>
      {custom ? (
        <div className="grid2">
          <label className="field"><span>Company name</span><input type="text" value={customName} onChange={(e) => setCustomName(e.target.value)} /></label>
          <label className="field"><span>Their privacy email <small className="muted">(optional)</small></span><input type="email" value={customEmail} onChange={(e) => setCustomEmail(e.target.value)} placeholder="privacy@…" /></label>
        </div>
      ) : (
        <CompanyFacts c={picked!} />
      )}
      <fieldset className="kinds">
        <legend className="section-title">What do you want?</legend>
        {(Object.keys(KIND_LABEL) as RequestKind[]).map((k) => (
          <label key={k} className={`kind${kind === k ? " on" : ""}`}>
            <input type="radio" name="kind" checked={kind === k} onChange={() => { setKind(k); setEdited(false); }} />
            <strong>{KIND_LABEL[k]}</strong>
            <span className="small muted">{k === "erasure" ? "Article 17: erase your account, messages and everything else they hold." : k === "access" ? "Article 15: what they hold, why, and who they shared it with." : "Article 21: no more ads, profiling or selling your data."}</span>
          </label>
        ))}
      </fieldset>
      <div className="grid2">
        <label className="field"><span>Your name</span><input type="text" value={name} onChange={(e) => { setName(e.target.value); setEdited(false); }} /></label>
        <label className="field"><span>Email or username you use with them</span><input type="text" value={account} onChange={(e) => { setAccount(e.target.value); setEdited(false); }} /></label>
      </div>
      <div className="letter">
        <label className="field"><span>Subject</span><input type="text" value={subject} onChange={(e) => { setSubject(e.target.value); setEdited(true); }} /></label>
        <label className="field"><span>Letter <small className="muted">(you can edit it)</small></span>
          <textarea rows={14} value={body} onChange={(e) => { setBody(e.target.value); setEdited(true); }} />
        </label>
        {edited && <button className="link small" onClick={() => setEdited(false)}>Start the letter again</button>}
      </div>
      <div className="send">
        <p className="section-title">Send it</p>
        <div className="req-actions left">
          {to && <button className="primary" onClick={() => openExternal(mailto(to, { subject, body }))}><Icon name="mail" size={16} />Open in my email app</button>}
          {!custom && picked!.f && <button className={to ? undefined : "primary"} onClick={() => openExternal(picked!.f!)}><Icon name="external" size={16} />Open their form</button>}
          <button onClick={() => copyText(`${subject}\n\n${body}`)}><Icon name="copy" size={16} />Copy the letter</button>
        </div>
        <p className="small muted">
          Sompio doesn't send anything: your email app sends it from your own address, so the answer comes back to you.
          {!custom && picked!.f && " With a web form, paste the letter into it."}
          {" "}Keep a copy of what you sent; you'll need it if they don't answer.
        </p>
        <button className="primary big" disabled={!companyName.trim()} onClick={track}><Icon name="check" size={16} />I've sent it: track the deadline</button>
      </div>
    </div>
  );
}

function CompanyRow({ c, onPick }: { c: Company; onPick: () => void }) {
  return (
    <button className="company-row" onClick={onPick}>
      <span className="grow">
        <strong>{c.n}</strong>
        <span className="small muted">{(c.w ?? "").replace(/^https?:\/\/(www\.)?/, "").replace(/\/$/, "")}</span>
      </span>
      <span className="small muted">{c.e ? "Email" : c.f ? "Web form" : c.a ? "Post" : ""}</span>
      {c.v && <span className="tag" title="Checked against the company's privacy policy">Checked</span>}
    </button>
  );
}

function CompanyFacts({ c }: { c: Company }) {
  return (
    <div className="company-facts">
      {c.e && <div><span className="muted small">Privacy email</span><strong className="selectable">{c.e}</strong></div>}
      {c.f && <div><span className="muted small">Web form</span><span className="selectable small">{c.f}</span></div>}
      {c.a && <div><span className="muted small">By post</span><span className="selectable small pre">{c.a}</span></div>}
      {c.id && <p className="small note">This company may ask you to prove who you are. Send only what's needed, and black out anything else on an ID.</p>}
      {c.m && <p className="small muted">{c.m}</p>}
    </div>
  );
}

// ---------- Your requests ----------

function Track(p: { data: TraceData; authorities: Authority[]; save: Save; remind: Remind; onAsk: () => void }) {
  const [open, setOpen] = useState<string | null>(null);
  const [mode, setMode] = useState<"reminder" | "complaint" | null>(null);
  const [country, setCountry] = useState(p.data.profile.country);
  const [authority, setAuthority] = useState("");
  const [confirmRemove, setConfirmRemove] = useState<string | null>(null);

  const countries = [...new Set(p.authorities.flatMap((a) => a["relevant-countries"]))].sort((a, b) => countryName(a).localeCompare(countryName(b)));
  const dpas = p.authorities.filter((a) => a["relevant-countries"].includes(country));
  const dpa = dpas.find((a) => a.slug === authority) ?? dpas.find((a) => a.slug === "debfdi") ?? dpas[0];

  const update = (id: string, patch: Partial<PrivacyRequest>, text?: string) =>
    p.save({ ...p.data, requests: p.data.requests.map((r) => (r.id === id ? { ...r, ...patch } : r)) }, text);

  if (p.data.requests.length === 0) {
    return (
      <div className="req-section">
        <header><h2>Your requests</h2></header>
        <div className="empty">
          <strong>No requests yet</strong>
          <span className="muted">When you ask a company to delete or send your data, it shows up here with its deadline.</span>
          <button className="primary" onClick={p.onAsk}>Ask a company</button>
        </div>
      </div>
    );
  }

  const status = (r: PrivacyRequest) => {
    if (r.status === "done") return { cls: "ok", text: "Done" };
    if (r.status === "answered") return { cls: "info", text: "Answered" };
    if (r.complained_on) return { cls: "warn", text: `Complaint filed ${niceDate(r.complained_on)}` };
    const due = r.sent_on ? daysUntil(addMonth(r.sent_on)) : 0;
    return due < 0 ? { cls: "late", text: `${-due} ${-due === 1 ? "day" : "days"} overdue` } : { cls: "wait", text: due === 0 ? "Due today" : `${due} ${due === 1 ? "day" : "days"} left` };
  };

  return (
    <div className="req-section">
      <header className="row-head">
        <h2>Your requests</h2>
        <span className="grow" />
        <button onClick={p.onAsk}><Icon name="plus" size={16} />New request</button>
      </header>
      <div className="table-wrap">
        <table className="req-table">
          <thead><tr><th>Company</th><th>Asked for</th><th>Sent</th><th>Deadline</th><th>Status</th><th /></tr></thead>
          <tbody>
            {p.data.requests.map((r) => {
              const s = status(r);
              const isOpen = open === r.id;
              const input = { kind: r.kind, company: r.company_name, name: p.data.profile.name, account: r.account, date: niceDate(today()), sentOn: r.sent_on ? niceDate(r.sent_on) : "[date]" };
              const rem = reminderLetter(input);
              const comp = complaintText({ ...input, reminders: r.reminders.map(niceDate) });
              return [
                <tr key={r.id} className={isOpen ? "open" : undefined}>
                  <td><strong>{r.company_name}</strong>{r.account && <div className="small muted">{r.account}</div>}</td>
                  <td>{KIND_SHORT[r.kind]}</td>
                  <td>{r.sent_on ? niceDate(r.sent_on) : ""}</td>
                  <td>{r.sent_on ? niceDate(addMonth(r.sent_on)) : ""}</td>
                  <td><span className={`state ${s.cls}`}>{s.text}</span>{r.reminders.length > 0 && <div className="small muted">Reminded {r.reminders.map(niceDate).join(", ")}</div>}</td>
                  <td className="right"><button className="small-btn" aria-expanded={isOpen} onClick={() => { setOpen(isOpen ? null : r.id); setMode(null); setConfirmRemove(null); }}>{isOpen ? "Close" : "Manage"}</button></td>
                </tr>,
                isOpen && (
                  <tr key={`${r.id}-d`} className="req-detail-row">
                    <td colSpan={6}>
                      <div className="req-detail">
                        <div className="req-actions left">
                          {r.status !== "answered" && <button onClick={() => update(r.id, { status: "answered" }, "Marked as answered.")}>They answered</button>}
                          {r.status !== "done" && <button onClick={() => update(r.id, { status: "done" }, "Marked as done.")}><Icon name="check" size={16} />It's done</button>}
                          {r.status !== "waiting" && <button className="quiet" onClick={() => update(r.id, { status: "waiting" })}>Still waiting</button>}
                          <button aria-pressed={mode === "reminder"} onClick={() => setMode(mode === "reminder" ? null : "reminder")}><Icon name="mail" size={16} />Send a reminder</button>
                          <button aria-pressed={mode === "complaint"} onClick={() => setMode(mode === "complaint" ? null : "complaint")}>Complain to the authority</button>
                          <span className="grow" />
                          {confirmRemove === r.id ? (
                            <>
                              <button className="danger" onClick={() => { setOpen(null); p.save({ ...p.data, requests: p.data.requests.filter((x) => x.id !== r.id) }, "Removed."); }}>Yes, remove it</button>
                              <button className="quiet" onClick={() => setConfirmRemove(null)}>Cancel</button>
                            </>
                          ) : (
                            <button className="quiet" onClick={() => setConfirmRemove(r.id)}><Icon name="trash" size={16} />Remove</button>
                          )}
                        </div>
                        <label className="field">
                          <span>Notes <small className="muted">(only you see these)</small></span>
                          <textarea rows={2} defaultValue={r.note} onBlur={(e) => e.target.value !== r.note && update(r.id, { note: e.target.value })} placeholder="Ticket number, what they said…" />
                        </label>
                        {mode === "reminder" && (
                          <div className="letter">
                            <p className="small muted">{s.cls === "late" ? "The deadline has passed, so a reminder is the next step." : "The deadline hasn't passed yet, but you can remind them any time."}</p>
                            <textarea rows={10} readOnly value={`${rem.subject}\n\n${rem.body}`} />
                            <div className="req-actions left">
                              <ReminderSend company={r.company} subject={rem.subject} body={rem.body} />
                              <button onClick={() => copyText(`${rem.subject}\n\n${rem.body}`)}><Icon name="copy" size={16} />Copy</button>
                              <button className="primary" onClick={async () => {
                                const extra = await p.remind(addDays(today(), 14), `Reminder deadline: ${r.company_name}`, `${r.company_name} had 14 days to answer your reminder. If they still haven't, file a complaint from Trace → Requests.`);
                                update(r.id, { reminders: [...r.reminders, today()] }, `Reminder noted.${extra}`);
                                setMode(null);
                              }}><Icon name="check" size={16} />I sent the reminder</button>
                            </div>
                          </div>
                        )}
                        {mode === "complaint" && (
                          <div className="letter">
                            <p className="small muted">
                              Every EU country has a data protection authority that handles complaints for free. Pick your country; you can complain
                              there even if the company sits elsewhere. Attach a copy of your request{r.reminders.length ? " and reminder" : ""}.
                            </p>
                            <div className="grid2">
                              <label className="field"><span>Your country</span>
                                <select value={country} onChange={(e) => { setCountry(e.target.value); setAuthority(""); }}>
                                  <option value="">Choose…</option>
                                  {countries.map((c) => <option key={c} value={c}>{countryName(c)}</option>)}
                                </select>
                              </label>
                              {dpas.length > 1 && (
                                <label className="field"><span>Authority</span>
                                  <select value={dpa?.slug ?? ""} onChange={(e) => setAuthority(e.target.value)}>
                                    {dpas.map((a) => <option key={a.slug} value={a.slug}>{a.name}</option>)}
                                  </select>
                                </label>
                              )}
                            </div>
                            {dpa && (
                              <>
                                <p><strong>{dpa.name}</strong>{dpa["complaint-language"] && <span className="small muted"> · complaints in {dpa["complaint-language"].toUpperCase()}{dpa["complaint-language"] !== "en" ? " (many also accept English)" : ""}</span>}</p>
                                <textarea rows={10} readOnly value={`${comp.subject}\n\n${comp.body}`} />
                                <div className="req-actions left">
                                  {dpa.webform && <button className="primary" onClick={() => openExternal(dpa.webform!)}><Icon name="external" size={16} />Open their complaint form</button>}
                                  {dpa.email && <button className={dpa.webform ? undefined : "primary"} onClick={() => openExternal(mailto(dpa.email!, comp))}><Icon name="mail" size={16} />Email it</button>}
                                  <button onClick={() => copyText(`${comp.subject}\n\n${comp.body}`)}><Icon name="copy" size={16} />Copy</button>
                                  <button onClick={() => {
                                    const profile = { ...p.data.profile, country };
                                    p.save({ ...p.data, profile, requests: p.data.requests.map((x) => (x.id === r.id ? { ...x, complained_on: today(), authority: dpa.name } : x)) }, "Complaint noted. Authorities often take a few months.");
                                    setMode(null);
                                  }}><Icon name="check" size={16} />I filed it</button>
                                </div>
                              </>
                            )}
                          </div>
                        )}
                      </div>
                    </td>
                  </tr>
                ),
              ];
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}

/** The reminder goes to the same privacy address as the request, when the directory has one. */
function ReminderSend({ company, subject, body }: { company: string; subject: string; body: string }) {
  const [to, setTo] = useState<string | null>(null);
  useEffect(() => {
    if (company) loadCompanies().then((all) => setTo(all.find((c: Company) => c.s === company)?.e ?? null)).catch(() => {});
  }, [company]);
  return to ? <button onClick={() => openExternal(mailto(to, { subject, body }))}><Icon name="mail" size={16} />Open in my email app</button> : null;
}

import { useVirtualizer } from "@tanstack/react-virtual";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  api, copyText, listen, openLabel, openLink as openUrl, pickSavePath, platformName, severityLabel,
  type FindingView, type KindKey, type Platform, type ScanSummary, type Status, type StatusTab,
} from "../api";
import type { Jump } from "../App";
import { dateTime, n, plural } from "../format";
import Icon from "../Icon";
import { usePaged } from "../usePaged";

interface Props {
  scan: ScanSummary | null;
  demo: boolean;
  initialKind: KindKey | null;
  onScan: (s: ScanSummary) => void;
  onOpen: (j: Jump) => void;
}

interface Toast { text: string; undo?: () => void }

const STATUS_DONE_TEXT: Record<Status, string> = {
  deleted: "Marked as deleted",
  ok: "Marked as not a problem",
  later: "Saved for later",
};

const NOUN: Record<KindKey, string> = {
  credential: "password or token",
  card: "card number",
  iban: "account number",
  watch_term: "term",
  address: "address",
  phone: "phone number",
  email: "email address",
  ip_address: "IP address",
  postcode: "postcode",
  crypto_wallet: "wallet",
  sensitive_file: "file",
};

export default function Review({ scan, demo, initialKind, onScan, onOpen }: Props) {
  const [tab, setTab] = useState<StatusTab>("open");
  const [kind, setKind] = useState<KindKey | null>(initialKind);
  const [platform, setPlatform] = useState<Platform | null>(null);
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [revealAll, setRevealAll] = useState(false);
  const [revealed, setRevealed] = useState<Map<number, FindingView>>(new Map());
  const [focus, setFocus] = useState(0);
  const [listVersion, setListVersion] = useState(0);
  const [toast, setToast] = useState<Toast | null>(null);
  const [scanning, setScanning] = useState<[number, number] | null>(null);
  const [terms, setTerms] = useState<string[]>([]);
  const [scannedTerms, setScannedTerms] = useState<string[]>([]);
  const [draft, setDraft] = useState("");
  const [confirmRecycle, setConfirmRecycle] = useState(false);

  useEffect(() => {
    api.watchTerms().then((t) => {
      setTerms(t);
      setScannedTerms(t);
    });
    api.sources().then((s) => setPlatforms(s.map((x) => x.platform)));
    const un = listen<[number, number]>("scan-progress", setScanning);
    return () => void un.then((f) => f());
  }, []);

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 6000);
    return () => clearTimeout(t);
  }, [toast]);

  const scanned = scan?.scanned_at_ms != null;
  const listKey = scanned ? `${tab}|${kind}|${platform}|${revealAll}|${listVersion}` : null;
  const { total, items, ensure } = usePaged<FindingView>(listKey, (o, l) => api.findings(kind, platform, tab, o, l, revealAll));

  // If the selected type has nothing left (say, after "never flag this again"), go back to everything.
  useEffect(() => {
    if (kind && scan && !scan.by_kind.some((k) => k.kind === kind && k.count > 0)) setKind(null);
  }, [scan, kind]);

  useEffect(() => {
    if (total != null) setFocus((f) => Math.max(0, Math.min(f, total - 1)));
  }, [total]);
  useEffect(() => setConfirmRecycle(false), [focus, listVersion]);

  const parentRef = useRef<HTMLDivElement>(null);
  const v = useVirtualizer({
    count: total ?? 0,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 96,
    overscan: 8,
  });
  const virtualItems = v.getVirtualItems();
  useEffect(() => {
    virtualItems.forEach((r) => !items[r.index] && ensure(r.index));
  }, [virtualItems, items, ensure]);

  const reload = () => setListVersion((x) => x + 1);

  const decide = useCallback(
    async (f: FindingView, status: Status | null) => {
      const previous = f.status;
      onScan(await api.setStatus(f.id, status));
      reload();
      if (status) {
        setToast({
          text: STATUS_DONE_TEXT[status],
          undo: async () => {
            onScan(await api.setStatus(f.id, previous));
            reload();
            setToast(null);
          },
        });
      } else {
        setToast({ text: "Moved back to To review" });
      }
    },
    [onScan],
  );

  const ignoreEverywhere = async (f: FindingView) => {
    const [count, summary] = await api.ignoreEverywhere(f.id);
    onScan(summary);
    reload();
    setToast({ text: `Won't flag this again. Cleared ${plural(count, "place", "places")}. You can undo this in Settings.` });
  };

  const toggleReveal = useCallback(async (f: FindingView) => {
    if (revealed.has(f.id)) {
      const next = new Map(revealed);
      next.delete(f.id);
      setRevealed(next);
      return;
    }
    const full = await api.revealFinding(f.id);
    if (full) setRevealed(new Map(revealed).set(f.id, full));
  }, [revealed]);

  /** Takes you to the item: its web page, or the file in File Explorer. */
  const goTo = useCallback(async (f: FindingView) => {
    try {
      if (f.platform === "pc") {
        await api.showInFolder(f.id);
        setToast({ text: "Opened the folder with the file selected." });
      } else if (f.link) {
        await openUrl(f.link);
        setToast({ text: "Opened in your browser. Delete it there, then mark it as deleted here." });
      } else {
        setToast({ text: `${platformName[f.platform]} has no web link for private chats. Open the chat in the app and find it by its date.` });
      }
    } catch (e) {
      setToast({ text: String(e) });
    }
  }, []);
  const copyLink = useCallback(async (f: FindingView) => {
    const value = f.link ?? f.file_path;
    if (!value) return;
    await copyText(value);
    setToast({ text: f.link ? "Link copied." : "File path copied." });
  }, []);
  const recycle = async (f: FindingView) => {
    try {
      const [path, summary] = await api.recycleFile(f.id);
      onScan(summary);
      reload();
      setToast({ text: `Moved ${path.split(/[\\/]/).pop()} to the Recycle Bin. You can restore it from there.` });
    } catch (e) {
      setToast({ text: String(e) });
    }
  };

  // Keyboard: works anywhere on the page except while typing in a field.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement;
      if (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT" || e.metaKey || e.ctrlKey || e.altKey) return;
      if (!total) return;
      const f = items[focus];
      const move = (d: number) => {
        e.preventDefault();
        const next = Math.max(0, Math.min(total - 1, focus + d));
        setFocus(next);
        v.scrollToIndex(next, { align: "auto" });
      };
      switch (e.key) {
        case "j": case "ArrowDown": return move(1);
        case "k": case "ArrowUp": return move(-1);
      }
      if (!f) return;
      const act = (fn: () => void) => { e.preventDefault(); fn(); };
      switch (e.key.toLowerCase()) {
        case " ": return act(() => toggleReveal(f));
        case "d": return f.status ? undefined : act(() => decide(f, "deleted"));
        case "n": return f.status ? undefined : act(() => decide(f, "ok"));
        case "l": return f.status ? undefined : act(() => decide(f, "later"));
        case "u": return f.status ? act(() => decide(f, null)) : undefined;
        case "o": return act(() => goTo(f));
        case "c": return act(() => copyLink(f));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [total, items, focus, decide, toggleReveal, goTo, copyLink, v]);

  const runScan = async (list = terms) => {
    setScanning([0, 1]);
    try {
      onScan(await api.runScan(list));
      setScannedTerms(list);
      setRevealed(new Map());
      reload();
    } catch (e) {
      setToast({ text: `The scan stopped: ${e}` });
    } finally {
      setScanning(null);
    }
  };

  const addTerm = () => {
    const t = draft.trim();
    if (t.length < 2 || terms.some((x) => x.toLowerCase() === t.toLowerCase())) return;
    setTerms([...terms, t]);
    setDraft("");
  };

  const exportList = async () => {
    const path = await pickSavePath("veil-flagged-items.csv");
    if (!path) return;
    const count = await api.exportFlagged(path);
    setToast({ text: `Saved ${plural(count, "item", "items")} with links. The file has no message text in it.` });
  };

  const termsChanged = scanned && JSON.stringify(terms) !== JSON.stringify(scannedTerms);
  const pct = scanning && scanning[1] > 0 ? (scanning[0] / scanning[1]) * 100 : 0;
  const all = scan ? scan.total_findings : 0;
  const base = items[focus];
  const current = base && (revealed.get(base.id) ? { ...base, ...pickText(revealed.get(base.id)!) } : base);

  return (
    <div className="page full review-page">
      <div className="review-head">
        <div className="lede">
          <h1 style={{ fontSize: "var(--t-xl)" }}>
            {!scanned ? "Review" : scan!.open > 0 ? `${plural(scan!.open, "thing", "things")} to review` : all > 0 ? "All reviewed" : "Nothing found"}
          </h1>
          <p className="muted small">Sensitive text stays hidden until you click it.</p>
        </div>
        {scanned && all > 0 && (
          <div className="meter">
            <div className="bar" aria-hidden><span style={{ width: `${(scan!.done / all) * 100}%` }} /></div>
            <span className="num">{n(scan!.done)} of {n(all)} reviewed</span>
          </div>
        )}
      </div>

      <WatchTerms
        terms={terms}
        draft={draft}
        demo={demo}
        scanned={scanned}
        changed={termsChanged}
        scanning={!!scanning}
        onDraft={setDraft}
        onAdd={addTerm}
        onRemove={(t) => setTerms(terms.filter((x) => x !== t))}
        onScan={() => runScan()}
      />
      {scanning && (
        <div className="progress" role="status" aria-live="polite">
          <span>Checking {n(Math.round(scanning[0]))} of {n(scanning[1])} items</span>
          <div className="bar"><span style={{ width: `${pct}%` }} /></div>
        </div>
      )}

      {scanned && all > 0 && (
        <>
          <div className="toolbar">
            <div className="segmented" role="tablist" aria-label="Review status">
              {([["open", "To review", scan!.open], ["later", "Later", scan!.later], ["done", "Done", scan!.done]] as const).map(([id, label, count]) => (
                <button key={id} role="tab" aria-selected={tab === id} onClick={() => { setTab(id); setFocus(0); }}>
                  {label} <span className="num">{n(count)}</span>
                </button>
              ))}
            </div>
            <select aria-label="Type" value={kind ?? ""} onChange={(e) => { setKind((e.target.value || null) as KindKey | null); setFocus(0); }}>
              <option value="">All types</option>
              {scan!.by_kind.filter((k) => k.count > 0).map((k) => (
                <option key={k.kind} value={k.kind}>{k.label} ({n(k.open)})</option>
              ))}
            </select>
            {platforms.length > 1 && (
              <select aria-label="Source" value={platform ?? ""} onChange={(e) => { setPlatform((e.target.value || null) as Platform | null); setFocus(0); }}>
                <option value="">All sources</option>
                {platforms.map((p) => <option key={p} value={p}>{platformName[p]}</option>)}
              </select>
            )}
            <span className="grow" />
            <label className="switch">
              <input type="checkbox" checked={revealAll} onChange={(e) => setRevealAll(e.target.checked)} />
              Show all hidden text
            </label>
            <button className="quiet" onClick={exportList}>Save list</button>
          </div>

          <div className="review-grid">
            <div className="review-list" ref={parentRef} role="listbox" aria-label="Findings" aria-activedescendant={base ? `f-${base.id}` : undefined}>
              {total === 0 && (
                <div className="empty">
                  {tab === "open" ? "Nothing left to review here." : tab === "later" ? "Nothing saved for later." : "Nothing marked as done yet."}
                </div>
              )}
              <div style={{ height: v.getTotalSize(), position: "relative" }}>
                {virtualItems.map((row) => {
                  const f = items[row.index];
                  return (
                    <div
                      key={row.key}
                      data-index={row.index}
                      ref={v.measureElement}
                      style={{ position: "absolute", top: 0, left: 0, right: 0, transform: `translateY(${row.start}px)` }}
                    >
                      {f ? (
                        <div id={`f-${f.id}`} role="option" aria-selected={row.index === focus} className={`item${row.index === focus ? " active" : ""}`} onClick={() => setFocus(row.index)}>
                          <div className="item-top">
                            <span className={`sev sev-${f.severity}`} aria-label={severityLabel(f.severity)} />
                            <span className="what">{f.label}</span>
                            <span className="when">{dateTime(f.ts)}</span>
                          </div>
                          <div className="item-snippet">
                            {f.before.slice(-50)}
                            <span className={`redact${f.masked ? "" : " shown"}`}>{f.matched}</span>
                            {f.after}
                          </div>
                          <div className="item-where">{platformName[f.platform]}, {f.channel_name}</div>
                        </div>
                      ) : (
                        <div className="item"><div className="placeholder" /></div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>

            <div className="detail">
              {current ? (
                <>
                  <div className="detail-head">
                    <span className={`risk risk-${current.severity}`}>{severityLabel(current.severity)}</span>
                    <h2>{current.label}</h2>
                    <div className="detail-meta">
                      <span className="tag">{platformName[current.platform]}</span>
                      <span>{current.channel_name}</span>
                      <span>{dateTime(current.ts)}</span>
                      {current.status && (
                        <span className="status-chip"><Icon name="check" size={14} />{current.status === "deleted" ? "Deleted" : current.status === "ok" ? "Not a problem" : "Saved for later"}</span>
                      )}
                    </div>
                  </div>

                  <p className="quote">
                    {current.before}
                    <button
                      className={`redact${current.masked ? "" : " shown"}`}
                      onClick={() => toggleReveal(base!)}
                      aria-label={current.masked ? `Show hidden ${current.label.toLowerCase()}` : "Hide again"}
                    >
                      {current.matched}
                    </button>
                    {current.after}
                    <span className="hint">{current.masked ? "Click the dark bar, or press Space, to see what was found." : "Click it again to hide."}</span>
                  </p>

                  <div className="advice">
                    <strong>What to do</strong>
                    <p>{current.advice}</p>
                  </div>

                  <div className="go-row">
                    {current.platform === "pc" ? (
                      <>
                        <button className="primary" onClick={() => goTo(current)}><Icon name="folder" size={16} />Show in folder</button>
                        {!current.status &&
                          (confirmRecycle ? (
                            <>
                              <button className="danger" onClick={() => recycle(current)}><Icon name="trash" size={16} />Yes, move it to the Recycle Bin</button>
                              <button className="quiet" onClick={() => setConfirmRecycle(false)}>Cancel</button>
                            </>
                          ) : (
                            <button onClick={() => setConfirmRecycle(true)}><Icon name="trash" size={16} />Move to Recycle Bin…</button>
                          ))}
                        <button className="quiet" onClick={() => copyLink(current)}><Icon name="copy" size={16} />Copy path</button>
                      </>
                    ) : (
                      <>
                        <button className="primary" onClick={() => goTo(current)}><Icon name="external" size={16} />{openLabel(current.platform)}</button>
                        {current.link && <button onClick={() => copyLink(current)}><Icon name="copy" size={16} />Copy link</button>}
                        <button className="quiet" onClick={() => onOpen({ channelId: current.channel_id, messageId: current.message_id })}>
                          <Icon name="messages" size={16} />Show conversation
                        </button>
                      </>
                    )}
                  </div>

                  <div className="decide">
                    <span className="small muted">{current.status ? "You already decided on this one." : "When you're done, mark it:"}</span>
                    {current.status ? (
                      <div><button onClick={() => decide(current, null)}>Move back to review <span className="faint">U</span></button></div>
                    ) : (
                      <div className="decide-row">
                        <button onClick={() => decide(current, "deleted")}><Icon name="check" size={16} />I deleted it <span className="key">D</span></button>
                        <button onClick={() => decide(current, "ok")}>Not a problem <span className="key">N</span></button>
                        <button onClick={() => decide(current, "later")}><Icon name="clock" size={16} />Later <span className="key">L</span></button>
                      </div>
                    )}
                    {!current.status && (
                      <div>
                        <button className="link small" onClick={() => ignoreEverywhere(current)}>Never flag this {NOUN[current.kind]} again</button>
                      </div>
                    )}
                  </div>

                  <div className="detail-foot" aria-label="Keyboard shortcuts">
                    <span><kbd>J</kbd> <kbd>K</kbd> next, previous</span>
                    <span><kbd>Space</kbd> show or hide</span>
                    <span><kbd>O</kbd> open where it is</span>
                    <span><kbd>C</kbd> copy link</span>
                  </div>
                </>
              ) : (
                <div className="detail-empty">
                  <span className="step-icon good"><Icon name="check" size={24} /></span>
                  <h3>{tab === "open" ? "Nothing to review here" : "Nothing in this list"}</h3>
                  <p className="small">Try another filter, or add more sources to check.</p>
                </div>
              )}
            </div>
          </div>
        </>
      )}

      {toast && (
        <div className="toast" role="status" aria-live="polite">
          <span>{toast.text}</span>
          {toast.undo && <button className="link" onClick={toast.undo}>Undo</button>}
          <button className="toast-close" aria-label="Dismiss" onClick={() => setToast(null)}>×</button>
        </div>
      )}
    </div>
  );
}

const pickText = (f: FindingView) => ({ before: f.before, matched: f.matched, after: f.after, masked: false });

function WatchTerms(props: {
  terms: string[]; draft: string; demo: boolean; scanned: boolean; changed: boolean; scanning: boolean;
  onDraft: (s: string) => void; onAdd: () => void; onRemove: (t: string) => void; onScan: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const open = editing || (props.terms.length === 0 && !dismissed) || props.changed || !props.scanned;
  if (!open && props.terms.length === 0) {
    return (
      <div className="watch-compact">
        <span className="muted">No watch terms yet.</span>
        <button className="small-btn ghost" onClick={() => { setDismissed(false); setEditing(true); }}>Add your name or street</button>
      </div>
    );
  }
  if (!open) {
    return (
      <div className="watch-compact">
        <span className="muted">Watch terms</span>
        <span className="chips">{props.terms.map((t) => <span className="chip plain" key={t}>{t}</span>)}</span>
        <button className="small-btn ghost" onClick={() => setEditing(true)}>Edit</button>
      </div>
    );
  }
  const p = props;
  const empty = p.terms.length === 0;
  return (
    <div className={`watch slim${empty && p.scanned ? " prompt" : ""}`}>
      <div className="watch-title">
        <h3>{empty && p.scanned ? "Add what only you know is sensitive" : "Watch terms"}</h3>
        <span className="muted small">
          Your real name, street, school or employer. Patterns can't guess these. They never leave this PC.
          {p.demo && " In the sample, try Sam Rivers."}
        </span>
      </div>
      {!empty && (
        <div className="chips">
          {p.terms.map((t) => (
            <span className="chip" key={t}>
              {t}
              <button aria-label={`Remove ${t}`} onClick={() => p.onRemove(t)}>×</button>
            </span>
          ))}
        </div>
      )}
      <div className="watch-row">
        <input
          type="text"
          value={p.draft}
          placeholder="Your full name, street, school…"
          onChange={(e) => p.onDraft(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && p.onAdd()}
          aria-label="New watch term"
        />
        <button onClick={p.onAdd} disabled={p.draft.trim().length < 2}><Icon name="plus" size={16} />Add</button>
        <button className={p.changed || !p.scanned ? "primary" : undefined} onClick={p.onScan} disabled={p.scanning}>
          {p.scanned ? "Check again" : "Start checking"}
        </button>
        {!empty && !p.changed && p.scanned && <button className="quiet" onClick={() => setEditing(false)}>Done</button>}
        {empty && p.scanned && <button className="quiet" onClick={() => { setEditing(false); setDismissed(true); }}>Not now</button>}
      </div>
      {p.changed && !p.scanning && <p className="small note">Your watch terms changed. Check again to include them. Your decisions are kept.</p>}
    </div>
  );
}

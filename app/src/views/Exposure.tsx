import { useVirtualizer } from "@tanstack/react-virtual";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  api, copyText, listen, openLabel, openLink as openUrl, pickSavePath, platformName, severityLabel,
  type FindingView, type KindKey, type ScanSummary, type Status, type StatusTab,
} from "../api";
import type { Jump } from "../App";
import { dateTime, n, plural } from "../format";
import { usePaged } from "../usePaged";

interface Props {
  scan: ScanSummary | null;
  demo: boolean;
  onScan: (s: ScanSummary) => void;
  onOpen: (j: Jump) => void;
}

interface Toast { text: string; undo?: () => void }

const STATUS_DONE_TEXT: Record<Status, string> = {
  deleted: "Marked as deleted",
  ok: "Marked as not a problem",
  later: "Saved for later",
};

export default function Exposure({ scan, demo, onScan, onOpen }: Props) {
  const [tab, setTab] = useState<StatusTab>("open");
  const [kind, setKind] = useState<KindKey | null>(null);
  const [revealAll, setRevealAll] = useState(false);
  const [revealed, setRevealed] = useState<Map<number, FindingView>>(new Map());
  const [focus, setFocus] = useState(0);
  const [listVersion, setListVersion] = useState(0);
  const [toast, setToast] = useState<Toast | null>(null);
  const [scanning, setScanning] = useState<[number, number] | null>(null);
  const [terms, setTerms] = useState<string[]>([]);
  const [scannedTerms, setScannedTerms] = useState<string[]>([]);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    api.watchTerms().then((t) => {
      setTerms(t);
      setScannedTerms(t);
    });
    const un = listen<[number, number]>("scan-progress", setScanning);
    return () => void un.then((f) => f());
  }, []);

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 6000);
    return () => clearTimeout(t);
  }, [toast]);

  const scanned = scan?.scanned_at_ms != null;
  const listKey = scanned ? `${tab}|${kind}|${revealAll}|${listVersion}` : null;
  const { total, items, ensure } = usePaged<FindingView>(listKey, (o, l) => api.findings(kind, null, tab, o, l, revealAll));

  // If the selected type has nothing left (say, after "never flag this again"), go back to everything.
  useEffect(() => {
    if (kind && scan && !scan.by_kind.some((k) => k.kind === kind && k.count > 0)) setKind(null);
  }, [scan, kind]);

  // Keep focus inside the list as it shrinks
  useEffect(() => {
    if (total != null) setFocus((f) => Math.max(0, Math.min(f, total - 1)));
  }, [total]);

  const parentRef = useRef<HTMLDivElement>(null);
  const v = useVirtualizer({
    count: total ?? 0,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 120,
    overscan: 6,
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
  const openLink = async (f: FindingView) => {
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
  };
  const copyLink = async (f: FindingView) => {
    const value = f.link ?? f.file_path;
    if (!value) return;
    await copyText(value);
    setToast({ text: f.link ? "Link copied." : "File path copied." });
  };
  const recycle = async (f: FindingView) => {
    try {
      const [path, summary] = await api.recycleFile(f.id);
      onScan(summary);
      reload();
      const name = path.split(/[\\/]/).pop();
      setToast({ text: `Moved ${name} to the Recycle Bin. You can restore it from there.` });
    } catch (e) {
      setToast({ text: String(e) });
    }
  };

  // Keyboard: works anywhere on the page except while typing in a field.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement;
      if (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || e.metaKey || e.ctrlKey || e.altKey) return;
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
        case "d": return act(() => decide(f, "deleted"));
        case "n": return act(() => decide(f, "ok"));
        case "l": return act(() => decide(f, "later"));
        case "u": return f.status ? act(() => decide(f, null)) : undefined;
        case "o": return act(() => openLink(f));
        case "c": return act(() => copyLink(f));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [total, items, focus, decide, toggleReveal]);

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
    const path = await pickSavePath("veil-flagged-messages.csv");
    if (!path) return;
    const count = await api.exportFlagged(path);
    setToast({ text: `Saved ${plural(count, "message", "messages")} with links. The file has no message text in it.` });
  };

  const termsChanged = scanned && JSON.stringify(terms) !== JSON.stringify(scannedTerms);
  const pct = scanning && scanning[1] > 0 ? (scanning[0] / scanning[1]) * 100 : 0;
  const reviewed = scan ? scan.done : 0;
  const all = scan ? scan.total_findings : 0;
  const selectedKind = scan?.by_kind.find((k) => k.kind === kind);

  return (
    <div className="page full review-page">
      <div className="lede">
        {!scanned ? (
          <h1>Find personal information you've posted.</h1>
        ) : all === 0 ? (
          <h1>No personal information found.</h1>
        ) : scan!.open > 0 ? (
          <h1>{plural(scan!.open, "thing", "things")} to review.</h1>
        ) : scan!.later > 0 ? (
          <h1>Only the ones you saved for later are left.</h1>
        ) : (
          <h1>All reviewed. Nice work.</h1>
        )}
        {scanned && all > 0 && (
          <div className="review-progress">
            <div className="bar" aria-hidden><span style={{ width: `${(reviewed / all) * 100}%` }} /></div>
            <span className="small muted num">{n(reviewed)} of {n(all)} reviewed</span>
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
          <span className="small">Checking {n(Math.round(scanning[0]))} of {n(scanning[1])} messages</span>
          <div className="bar"><span style={{ width: `${pct}%` }} /></div>
        </div>
      )}

      {scanned && all > 0 && (
        <div className="exposure">
          <div className="kinds" role="group" aria-label="Filter by type">
            <button className="kind" aria-pressed={kind === null} onClick={() => setKind(null)}>
              <span />
              Everything
              <span className="num">{n(scan!.open + scan!.later)}</span>
            </button>
            {scan!.by_kind.filter((k) => k.count > 0).map((k) => (
              <button key={k.kind} className="kind" aria-pressed={kind === k.kind} onClick={() => { setKind(k.kind); setFocus(0); }}>
                <span className={`sev sev-${k.severity}`} aria-hidden />
                {k.label}
                <span className="num">{n(k.open)}</span>
              </button>
            ))}
            <p className="small muted kinds-note">Numbers show what's still to review.</p>
          </div>

          <div className="review-col">
            <div className="toolbar">
              <div className="tabs" role="tablist" aria-label="Review status">
                {([["open", "To review", scan!.open], ["later", "Later", scan!.later], ["done", "Done", scan!.done]] as const).map(([id, label, count]) => (
                  <button key={id} role="tab" aria-selected={tab === id} onClick={() => { setTab(id); setFocus(0); }}>
                    {label} <span className="num muted">{n(count)}</span>
                  </button>
                ))}
              </div>
              <span className="grow" />
              <label className="switch small">
                <input type="checkbox" checked={revealAll} onChange={(e) => setRevealAll(e.target.checked)} />
                Show all hidden text
              </label>
              <button className="small-btn" onClick={exportList}>Save list</button>
            </div>

            {selectedKind && <p className="advice-box">{selectedKind.advice}</p>}

            <div className="list" ref={parentRef} role="listbox" aria-label="Findings" aria-activedescendant={items[focus] ? `f-${items[focus]!.id}` : undefined}>
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
                        <FindingRow
                          f={revealed.get(f.id) ? { ...f, ...pickText(revealed.get(f.id)!) } : f}
                          focused={row.index === focus}
                          onFocus={() => setFocus(row.index)}
                          onReveal={() => toggleReveal(f)}
                          onDecide={(s) => decide(f, s)}
                          onIgnore={() => ignoreEverywhere(f)}
                          onOpenLink={() => openLink(f)}
                          onCopyLink={() => copyLink(f)}
                          onRecycle={() => recycle(f)}
                          onOpenHere={() => onOpen({ channelId: f.channel_id, messageId: f.message_id })}
                        />
                      ) : (
                        <div className="finding"><div className="placeholder" /></div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
            <KeyHints />
          </div>
        </div>
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
  sensitive_file: "file name",
};

const pickText = (f: FindingView) => ({ before: f.before, matched: f.matched, after: f.after, masked: false });

interface RowProps {
  f: FindingView;
  focused: boolean;
  onFocus: () => void;
  onReveal: () => void;
  onDecide: (s: Status | null) => void;
  onIgnore: () => void;
  onOpenLink: () => void;
  onCopyLink: () => void;
  onRecycle: () => void;
  onOpenHere: () => void;
}

function FindingRow({ f, focused, onFocus, onReveal, onDecide, onIgnore, onOpenLink, onCopyLink, onRecycle, onOpenHere }: RowProps) {
  const [confirmRecycle, setConfirmRecycle] = useState(false);
  const isFile = f.platform === "pc";
  return (
    <article id={`f-${f.id}`} role="option" aria-selected={focused} className={`finding${focused ? " focused" : ""}`} onClick={onFocus}>
      <div className="finding-meta">
        <span className={`sev-pill sev-pill-${f.severity}`}>{severityLabel(f.severity)}</span>
        <span className="platform-chip">{platformName[f.platform]}</span>
        <span>{f.label}</span>
        <span className="where" title={f.channel_name}>{f.channel_name}</span>
        <time>{dateTime(f.ts)}</time>
        {f.status && <span className="status-chip">{f.status === "deleted" ? "Deleted" : f.status === "ok" ? "Not a problem" : "Later"}</span>}
      </div>
      <p className="snippet">
        {f.before}
        <button
          className={`redact${f.masked ? "" : " shown"}`}
          onClick={(e) => { e.stopPropagation(); onReveal(); }}
          aria-label={f.masked ? `Show hidden ${f.label.toLowerCase()}` : "Hide again"}
          title={f.masked ? "Click to show" : "Click to hide"}
        >
          {f.matched}
        </button>
        {f.after}
      </p>
      {focused && <p className="advice">{f.advice}</p>}
      <div className="actions" onClick={(e) => e.stopPropagation()}>
        {isFile ? (
          <>
            <button className="small-btn primary-soft" onClick={onOpenLink}>Show in folder</button>
            {!f.status &&
              (confirmRecycle ? (
                <>
                  <button className="small-btn danger" onClick={onRecycle}>Yes, move to Recycle Bin</button>
                  <button className="small-btn ghost" onClick={() => setConfirmRecycle(false)}>Cancel</button>
                </>
              ) : (
                <button className="small-btn" onClick={() => setConfirmRecycle(true)}>Move to Recycle Bin…</button>
              ))}
          </>
        ) : (
          <>
            <button className="small-btn primary-soft" onClick={onOpenLink}>{openLabel(f.platform)}</button>
            {f.link && <button className="small-btn" onClick={onCopyLink}>Copy link</button>}
            <button className="small-btn ghost" onClick={onOpenHere}>Show conversation</button>
          </>
        )}
        <span className="grow" />
        {f.status ? (
          <button className="small-btn" onClick={() => onDecide(null)}>Move back to review</button>
        ) : (
          <>
            <button className="small-btn" onClick={() => onDecide("deleted")}>I deleted it</button>
            <button className="small-btn" onClick={() => onDecide("ok")}>Not a problem</button>
            <button className="small-btn" onClick={() => onDecide("later")}>Later</button>
          </>
        )}
      </div>
      {focused && !f.status && (
        <button className="link small never" onClick={(e) => { e.stopPropagation(); onIgnore(); }}>
          Never flag this {NOUN[f.kind]} again
        </button>
      )}
    </article>
  );
}

function WatchTerms(props: {
  terms: string[]; draft: string; demo: boolean; scanned: boolean; changed: boolean; scanning: boolean;
  onDraft: (s: string) => void; onAdd: () => void; onRemove: (t: string) => void; onScan: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const open = editing || props.terms.length === 0 || props.changed || !props.scanned;
  if (!open) {
    return (
      <div className="watch-compact">
        <span className="muted">Watch terms</span>
        <span className="chips">
          {props.terms.map((t) => <span className="chip plain" key={t}>{t}</span>)}
        </span>
        <button className="small-btn" onClick={() => setEditing(true)}>Edit</button>
      </div>
    );
  }
  return <WatchTermsEditor {...props} onDone={props.terms.length > 0 && !props.changed ? () => setEditing(false) : undefined} />;
}

function WatchTermsEditor(p: {
  terms: string[]; draft: string; demo: boolean; scanned: boolean; changed: boolean; scanning: boolean;
  onDraft: (s: string) => void; onAdd: () => void; onRemove: (t: string) => void; onScan: () => void;
  onDone?: () => void;
}) {
  const empty = p.terms.length === 0;
  return (
    <div className={`watch${empty && p.scanned ? " prompt" : ""}`}>
      <div>
        <h3>{empty && p.scanned ? "Now add what only you know is sensitive" : "Watch terms"}</h3>
        <p className="muted small">
          Veil can't guess your real name or street. Add them, plus anything else you'd rather not have online, like
          your school, employer or a nickname people know offline. They never leave this computer.
          {p.demo && " In the sample data, try adding Sam Rivers."}
        </p>
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
        <button onClick={p.onAdd} disabled={p.draft.trim().length < 2}>Add</button>
        <button className={p.changed || !p.scanned ? "primary" : undefined} onClick={p.onScan} disabled={p.scanning}>
          {p.scanned ? "Scan again" : "Start scan"}
        </button>
        {p.onDone && <button className="ghost-btn" onClick={p.onDone}>Done</button>}
      </div>
      {p.changed && !p.scanning && <p className="small note">Your watch terms changed. Scan again to include them. Your decisions are kept.</p>}
    </div>
  );
}

function KeyHints() {
  const keys: [string, string][] = [
    ["J / K", "next, previous"],
    ["Space", "show or hide"],
    ["D", "deleted"],
    ["N", "not a problem"],
    ["L", "later"],
    ["O", "open where it is"],
  ];
  return (
    <div className="key-hints" aria-label="Keyboard shortcuts">
      {keys.map(([k, what]) => (
        <span key={k}><kbd>{k}</kbd> {what}</span>
      ))}
    </div>
  );
}

import { useVirtualizer } from "@tanstack/react-virtual";
import { useCallback, useEffect, useRef, useState } from "react";
import {
  api, copyText, listen, openLabel, openLink as openUrl, pickSavePath, platformName, severityLabel,
  type FindingView, type GroupInfo, type KindKey, type Platform, type Previous, type ScanSummary, type Status, type StatusTab,
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

/** One message to delete on its platform. */
interface Target { id: number; message_id: string; platform: Platform }

/** The guided clean-up: one finding at a time, with the right way to remove it. */
interface Guide { items: FindingView[]; i: number; deleted: number; ok: number; skipped: number; busy?: boolean; error?: string }

/** A run of deletions on a connected platform, one message at a time. */
interface Batch { label: string; done: number; total: number; failed: number }

const loadGrouped = () => {
  try {
    return localStorage.getItem("sompio.review.grouped2") === "1";
  } catch {
    return false;
  }
};

const loadReveal = () => {
  try {
    return localStorage.getItem("sompio.review.reveal") !== "0";
  } catch {
    return true;
  }
};

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
  id_number: "ID number",
  passport: "document number",
  backup_code: "backup code",
  birth_date: "date of birth",
  number_plate: "number plate",
};

export default function Review({ scan, demo, initialKind, onScan, onOpen }: Props) {
  const [tab, setTab] = useState<StatusTab>("open");
  const [kind, setKind] = useState<KindKey | null>(initialKind);
  const [platform, setPlatform] = useState<Platform | null>(null);
  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [revealAll, setRevealAll] = useState(loadReveal);
  const [showTerms, setShowTerms] = useState(false);
  const [revealed, setRevealed] = useState<Map<number, FindingView>>(new Map());
  const [focus, setFocus] = useState(0);
  const [listVersion, setListVersion] = useState(0);
  const [toast, setToast] = useState<Toast | null>(null);
  const [scanning, setScanning] = useState<[number, number] | null>(null);
  const [terms, setTerms] = useState<string[]>([]);
  const [scannedTerms, setScannedTerms] = useState<string[]>([]);
  const [draft, setDraft] = useState("");
  const [confirmRecycle, setConfirmRecycle] = useState(false);
  const [tgConnected, setTgConnected] = useState(false);
  const [emailConnected, setEmailConnected] = useState(false);
  const [grouped, setGrouped] = useState(loadGrouped);
  const [sameAs, setSameAs] = useState<FindingView | null>(null);
  const [selected, setSelected] = useState<Map<number, FindingView>>(new Map());
  const [anchor, setAnchor] = useState<number | null>(null);
  const [group, setGroup] = useState<GroupInfo | null>(null);
  const [batch, setBatch] = useState<Batch | null>(null);
  const stopBatch = useRef(false);
  const [confirmGroupDelete, setConfirmGroupDelete] = useState<Platform | null>(null);
  const [guide, setGuide] = useState<Guide | null>(null);

  useEffect(() => {
    api.watchTerms().then((t) => {
      setTerms(t);
      setScannedTerms(t);
    });
    api.sources().then((s) => {
      setPlatforms(s.map((x) => x.platform));
      // Only ask about the Telegram connection when there is Telegram data to act on.
      if (s.some((x) => x.platform === "telegram")) api.tgStatus().then((t) => setTgConnected(t.connected)).catch(() => {});
      if (s.some((x) => x.platform === "email")) api.emailStatus().then((t) => setEmailConnected(t.connected)).catch(() => {});
    });
    const un = listen<[number, number]>("scan-progress", setScanning);
    // Data that was never checked: check it now, so the findings are always there
    if (scan && scan.scanned_at_ms == null) api.watchTerms().then((t) => runScanRef.current(t));
    return () => void un.then((f) => f());
  }, []);

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setToast(null), 6000);
    return () => clearTimeout(t);
  }, [toast]);

  const scanned = scan?.scanned_at_ms != null;
  const groupList = grouped && !sameAs;
  const listKey = scanned ? `${tab}|${kind}|${platform}|${revealAll}|${groupList}|${sameAs?.id}|${listVersion}` : null;
  const { total, items, ensure } = usePaged<FindingView>(listKey, (o, l) =>
    api.findings(kind, platform, tab, o, l, revealAll, groupList, sameAs?.id ?? null),
  );
  useEffect(() => {
    setSelected(new Map());
    setAnchor(null);
  }, [tab, kind, platform, grouped, sameAs, listVersion]);
  useEffect(() => {
    try {
      localStorage.setItem("sompio.review.grouped2", grouped ? "1" : "0");
      localStorage.setItem("sompio.review.reveal", revealAll ? "1" : "0");
    } catch {
      /* not available: the setting just isn't remembered */
    }
  }, [grouped, revealAll]);

  // If the selected type has nothing left (say, after "never flag this again"), go back to everything.
  useEffect(() => {
    if (kind && scan && !scan.by_kind.some((k) => k.kind === kind && k.count > 0)) setKind(null);
  }, [scan, kind]);

  useEffect(() => {
    if (total != null) setFocus((f) => Math.max(0, Math.min(f, total - 1)));
  }, [total]);
  useEffect(() => {
    setConfirmRecycle(false);
    setConfirmGroupDelete(null);
  }, [focus, listVersion]);

  const parentRef = useRef<HTMLDivElement>(null);
  const v = useVirtualizer({
    count: total ?? 0,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 30,
    overscan: 8,
  });
  const virtualItems = v.getVirtualItems();
  useEffect(() => {
    virtualItems.forEach((r) => !items[r.index] && ensure(r.index));
  }, [virtualItems, items, ensure]);

  const reload = () => setListVersion((x) => x + 1);

  // Where else the focused item's value appears.
  const focusedId = items[focus]?.id;
  const focusedRepeats = items[focus]?.repeats ?? 0;
  useEffect(() => {
    setGroup(null);
    if (focusedId == null || focusedRepeats < 2) return;
    let live = true;
    api.groupInfo(focusedId, tab).then((g) => live && setGroup(g)).catch(() => {});
    return () => { live = false; };
  }, [focusedId, focusedRepeats, tab, listVersion]);

  /** One decision for many findings, with Undo. */
  const applyMany = useCallback(
    async (ids: number[], status: Status, text: string) => {
      const [previous, summary] = await api.setStatusMany(ids, status);
      onScan(summary);
      reload();
      setToast({
        text,
        undo: async () => {
          onScan(await api.restoreStatuses(previous as Previous));
          reload();
          setToast(null);
        },
      });
    },
    [onScan],
  );

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

  /** In the grouped list, a decision covers every place the value appears. */
  const groupIds = (f: FindingView) => (groupList && group && f.repeats > 1 && group.ids.includes(f.id) ? group.ids : null);
  const decideHere = useCallback(
    (f: FindingView, status: Status) => {
      const ids = groupIds(f);
      if (ids) return applyMany(ids, status, `${STATUS_DONE_TEXT[status]}: all ${n(ids.length)} places`);
      return decide(f, status);
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [groupList, group, applyMany, decide],
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
        setToast({ text: `There's no web link for this one. Open ${platformName[f.platform]} and find it by its date.` });
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
  /** Deleting on the platform itself, for connected accounts. */
  const liveDelete = (f: FindingView) =>
    f.status
      ? null
      : f.platform === "telegram" && tgConnected
        ? { ask: "Delete from Telegram…", confirm: "Yes, delete it for everyone", run: api.tgDelete, done: "Deleted from Telegram for everyone in that chat." }
        : f.platform === "email" && emailConnected
          ? { ask: "Move to Trash…", confirm: "Yes, move it to Trash", run: api.emailDelete, done: "Moved to your mailbox's Trash. You can restore it from there." }
          : null;
  const liveFor = (p: Platform) =>
    p === "telegram" && tgConnected
      ? { run: api.tgDelete, noun: "Telegram message", verb: "Deleting from Telegram", ask: (k: number) => `Delete ${plural(k, "message", "messages")} from Telegram…` }
      : p === "email" && emailConnected
        ? { run: api.emailDelete, noun: "email", verb: "Moving emails to Trash", ask: (k: number) => `Move ${plural(k, "email", "emails")} to Trash…` }
        : null;

  /** Deletes messages on their platform one at a time, with a pause so the platform doesn't block the account. */
  const runLive = async (targets: Target[]) => {
    const seen = new Set<string>();
    const todo = targets.filter((t) => liveFor(t.platform) && !seen.has(t.message_id) && seen.add(t.message_id));
    if (todo.length === 0) return;
    stopBatch.current = false;
    let failed = 0;
    let lastError = "";
    setBatch({ label: liveFor(todo[0].platform)!.verb, done: 0, total: todo.length, failed: 0 });
    for (let i = 0; i < todo.length; i++) {
      if (stopBatch.current) break;
      try {
        onScan(await liveFor(todo[i].platform)!.run(todo[i].id));
      } catch (e) {
        failed++;
        lastError = String(e);
      }
      setBatch((b) => (b ? { ...b, done: i + 1, failed } : b));
      if (i < todo.length - 1) await new Promise((r) => setTimeout(r, 400));
    }
    const done = todo.length - failed;
    setBatch(null);
    reload();
    setToast({
      text: stopBatch.current
        ? "Stopped. What was already deleted stays deleted."
        : failed > 0
          ? `Deleted ${n(done)} of ${n(todo.length)}. ${failed} didn't work: ${lastError}`
          : `Deleted ${plural(done, "message", "messages")}. Emails went to Trash, where you can restore them.`,
    });
  };

  /** The selected items, plus every place each one's value appears when the list is grouped. */
  const expandSelection = async (): Promise<Target[]> => {
    const out: Target[] = [];
    for (const f of selected.values()) {
      if (groupList && f.repeats > 1) {
        const g = await api.groupInfo(f.id, tab);
        g.ids.forEach((id, i) => out.push({ id, message_id: g.message_ids[i], platform: g.member_platforms[i] }));
      } else {
        out.push({ id: f.id, message_id: f.message_id, platform: f.platform });
      }
    }
    return out;
  };
  const decideSelected = async (status: Status) => {
    const targets = await expandSelection();
    await applyMany(targets.map((t) => t.id), status, `${STATUS_DONE_TEXT[status]}: ${plural(targets.length, "item", "items")}`);
  };
  const toggleSelect = (f: FindingView, index: number, range: boolean) => {
    const next = new Map(selected);
    if (range && anchor != null) {
      const [a, b] = [Math.min(anchor, index), Math.max(anchor, index)];
      for (let i = a; i <= b; i++) if (items[i]) next.set(items[i]!.id, items[i]!);
    } else if (next.has(f.id)) next.delete(f.id);
    else next.set(f.id, f);
    setSelected(next);
    setAnchor(index);
  };

  const deleteLive = async (f: FindingView) => {
    const how = liveDelete(f);
    if (!how) return;
    try {
      onScan(await how.run(f.id));
      reload();
      setToast({ text: how.done });
    } catch (e) {
      setToast({ text: String(e) });
    }
  };
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

  /** Starts the guided clean-up with the selected items, or everything still to review in this view. */
  const startGuide = async (fromSelection: boolean) => {
    let list: FindingView[];
    if (fromSelection) list = [...selected.values()].filter((f) => !f.status);
    else list = (await api.findings(kind, platform, "open", 0, 500, true, false, null)).items;
    if (list.length === 0) return setToast({ text: "Nothing left to clean up here." });
    setSelected(new Map());
    setGuide({ items: list, i: 0, deleted: 0, ok: 0, skipped: 0 });
  };
  /** Records what happened to the current item and moves on. */
  const guideNext = async (outcome: "deleted" | "ok" | "skipped", markIt = true) => {
    if (!guide) return;
    const f = guide.items[guide.i];
    if (markIt && outcome !== "skipped") {
      try {
        onScan(await api.setStatus(f.id, outcome));
      } catch (e) {
        return setGuide({ ...guide, error: String(e) });
      }
    }
    setGuide({ ...guide, i: guide.i + 1, busy: false, error: undefined, [outcome]: guide[outcome] + 1 });
  };
  /** Removes it for you where Sompio can: connected Telegram or email, or a file on this PC. */
  const guideRemove = async () => {
    if (!guide) return;
    const f = guide.items[guide.i];
    setGuide({ ...guide, busy: true, error: undefined });
    try {
      if (f.platform === "pc") onScan((await api.recycleFile(f.id))[1]);
      else onScan(await liveDelete(f)!.run(f.id));
      await guideNext("deleted", false);
    } catch (e) {
      setGuide({ ...guide, busy: false, error: String(e) });
    }
  };
  const closeGuide = () => {
    setGuide(null);
    reload();
  };

  // Keyboard: works anywhere on the page except while typing in a field.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (guide) return;
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
        case "x": return act(() => toggleSelect(f, focus, e.shiftKey));
        case "escape": return selected.size ? act(() => setSelected(new Map())) : undefined;
        case "d": return f.status ? undefined : act(() => decideHere(f, "deleted"));
        case "n": return f.status ? undefined : act(() => decideHere(f, "ok"));
        case "l": return f.status ? undefined : act(() => decideHere(f, "later"));
        case "u": return f.status ? act(() => decide(f, null)) : undefined;
        case "o": return act(() => goTo(f));
        case "c": return act(() => copyLink(f));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [total, items, focus, decide, decideHere, toggleReveal, goTo, copyLink, v, selected, anchor]);

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

  const runScanRef = useRef(runScan);
  runScanRef.current = runScan;

  const addTerm = () => {
    const t = draft.trim();
    if (t.length < 2 || terms.some((x) => x.toLowerCase() === t.toLowerCase())) return;
    setTerms([...terms, t]);
    setDraft("");
  };

  const exportList = async () => {
    const path = await pickSavePath("sompio-flagged-items.csv");
    if (!path) return;
    const count = await api.exportFlagged(path);
    setToast({ text: `Saved ${plural(count, "item", "items")} with links. The file has no message text in it.` });
  };

  const termsChanged = scanned && JSON.stringify(terms) !== JSON.stringify(scannedTerms);
  const pct = scanning && scanning[1] > 0 ? (scanning[0] / scanning[1]) * 100 : 0;
  const all = scan ? scan.total_findings : 0;
  const base = items[focus];
  const current = base && (revealed.get(base.id) ? { ...base, ...pickText(revealed.get(base.id)!) } : base);

  const kinds = scan ? scan.by_kind.filter((k) => k.count > 0) : [];
  const kindCount = (k: (typeof kinds)[number]) => (tab === "open" ? k.open : null);
  const bySeverity: [number, string][] = [[3, "Fix now"], [2, "Worth a look"], [1, "Low risk"]];
  const tabCount = scan ? (tab === "open" ? scan.open : tab === "later" ? scan.later : scan.done) : 0;

  return (
    <div className="xc-page review-page">
      <div className="xc-toolbar" role="toolbar" aria-label="Findings">
        <button onClick={() => runScan()} disabled={!!scanning} title="Check every message and file again"><Icon name="refresh" size={16} />Check again</button>
        <button aria-pressed={showTerms} onClick={() => setShowTerms(!showTerms)} title="Your name, street, school: things only you know are sensitive">
          <Icon name="filter" size={16} />Watch terms{terms.length ? ` (${terms.length})` : ""}
        </button>
        <button onClick={() => startGuide(false)} disabled={!scanned || !scan?.open} title="Go through everything still to review, one at a time, and remove it">
          <Icon name="trash" size={16} />Clean up
        </button>
        <button onClick={exportList} disabled={!scanned || all === 0} title="A list of everything found, with links. No message text."><Icon name="save" size={16} />Save list</button>
        <span className="sep" />
        {scanned && all > 0 && (
          <div className="segmented" role="tablist" aria-label="Review status">
            {([["open", "To review", scan!.open], ["later", "Later", scan!.later], ["done", "Done", scan!.done]] as const).map(([id, label, count]) => (
              <button key={id} role="tab" aria-selected={tab === id} onClick={() => { setTab(id); setFocus(0); }}>
                {label} <span className="num">{n(count)}</span>
              </button>
            ))}
          </div>
        )}
        <span className="grow" />
        <label className="switch" title="Show what was found. Turn off when someone can see your screen.">
          <input type="checkbox" checked={revealAll} onChange={(e) => { setRevealAll(e.target.checked); setRevealed(new Map()); }} />
          Show values
        </label>
        <label className="switch" title="Show each value once, with how many places it appears">
          <input type="checkbox" checked={grouped} onChange={(e) => { setGrouped(e.target.checked); setSameAs(null); setFocus(0); }} />
          Group repeats
        </label>
      </div>

      {(showTerms || !scanned) && (
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
          onClose={() => setShowTerms(false)}
        />
      )}
      {scanning && (
        <div className="progress xc-progress" role="status" aria-live="polite">
          <span>Checking {n(Math.round(scanning[0]))} of {n(scanning[1])} items for personal information</span>
          <div className="bar"><span style={{ width: `${pct}%` }} /></div>
        </div>
      )}
      {sameAs && (
        <div className="banner-row">
          <span>Every place this {NOUN[sameAs.kind]} appears{total != null ? ` (${n(total)})` : ""}. Decide on them one by one.</span>
          <button className="quiet" onClick={() => { setSameAs(null); setFocus(0); }}>Back to the full list</button>
        </div>
      )}
      {selected.size > 0 && (
        <BulkBar
          count={selected.size}
          grouped={groupList}
          tab={tab}
          live={(() => {
            const ps = new Set([...selected.values()].map((f) => f.platform));
            if (ps.size !== 1) return null;
            const how = liveFor([...ps][0]);
            return how && tab !== "done" ? how.ask(selected.size) : null;
          })()}
          onDecide={decideSelected}
          onGuide={() => startGuide(true)}
          onLive={async () => runLive(await expandSelection())}
          onClear={() => setSelected(new Map())}
        />
      )}

      <div className="xc-body">
        <aside className="xc-tree" aria-label="What was found">
          <button className="xc-node root" aria-current={kind == null} onClick={() => { setKind(null); setFocus(0); }}>
            <Icon name="review" size={16} /><span>Everything found</span><span className="num">{n(tabCount)}</span>
          </button>
          {bySeverity.map(([sev, title]) => {
            const ks = kinds.filter((k) => k.severity === sev);
            if (!ks.length) return null;
            return (
              <div key={sev} className="xc-branch">
                <div className="xc-branch-label"><span className={`sev sev-${sev}`} />{title}</div>
                {ks.map((k) => (
                  <button key={k.kind} className="xc-node" aria-current={kind === k.kind} onClick={() => { setKind(k.kind); setFocus(0); }}>
                    <span>{k.label}</span><span className="num">{kindCount(k) == null ? "" : n(kindCount(k)!)}</span>
                  </button>
                ))}
              </div>
            );
          })}
          {platforms.length > 1 && (
            <div className="xc-branch">
              <div className="xc-branch-label">Where</div>
              <button className="xc-node" aria-current={platform == null} onClick={() => { setPlatform(null); setFocus(0); }}><span>All sources</span></button>
              {platforms.map((p) => (
                <button key={p} className="xc-node" aria-current={platform === p} onClick={() => { setPlatform(p); setFocus(0); }}><span>{platformName[p]}</span></button>
              ))}
            </div>
          )}
          {scanned && all > 0 && (
            <div className="xc-tree-foot">
              <div className="bar" aria-hidden><span style={{ width: `${(scan!.done / all) * 100}%` }} /></div>
              <span className="small muted">{n(scan!.done)} of {n(all)} dealt with</span>
            </div>
          )}
        </aside>

        <div className="xc-main">
          <div className="xc-table">
            <div className="xc-thead" role="row">
              <span />
              <span>Type</span>
              <span>Found</span>
              <span>In the message</span>
              <span>Where</span>
              <span>Date</span>
            </div>
            <div className="xc-rows" ref={parentRef} role="listbox" aria-label="Findings" aria-activedescendant={base ? `f-${base.id}` : undefined} tabIndex={0}>
              {scanned && total === 0 && (
                <div className="empty">
                  {all === 0 ? "Nothing personal found. Add more sources, or watch terms like your name." : tab === "open" ? "Nothing left to review here." : tab === "later" ? "Nothing saved for later." : "Nothing marked as done yet."}
                </div>
              )}
              {!scanned && !scanning && <div className="empty">Not checked yet. Click Check again.</div>}
              <div style={{ height: v.getTotalSize(), position: "relative" }}>
                {virtualItems.map((row) => {
                  const f = items[row.index];
                  const shown = f && revealed.get(f.id);
                  const t = shown ? { ...f!, ...pickText(shown) } : f;
                  return (
                    <div key={row.key} style={{ position: "absolute", top: 0, left: 0, right: 0, height: row.size, transform: `translateY(${row.start}px)` }}>
                      {t ? (
                        <div
                          id={`f-${t.id}`}
                          role="option"
                          aria-selected={row.index === focus}
                          className={`xc-row${row.index === focus ? " active" : ""}${selected.has(t.id) ? " picked" : ""}${t.status ? " decided" : ""}`}
                          onClick={(e) => {
                            if (e.shiftKey || e.ctrlKey || e.metaKey) toggleSelect(t, row.index, e.shiftKey);
                            setFocus(row.index);
                          }}
                          onDoubleClick={() => goTo(t)}
                        >
                          <span>
                            <input type="checkbox" aria-label="Select" checked={selected.has(t.id)} onClick={(e) => e.stopPropagation()}
                              onChange={(e) => toggleSelect(t, row.index, (e.nativeEvent as MouseEvent).shiftKey)} />
                          </span>
                          <span className="c-type"><span className={`sev sev-${t.severity}`} aria-label={severityLabel(t.severity)} />{t.label}</span>
                          <span className="c-found">
                            {t.masked ? <span className="redact">{t.matched}</span> : <strong>{t.matched}</strong>}
                          </span>
                          <span className="c-msg">{t.before.slice(-40)}<mark>{t.masked ? "•••" : t.matched}</mark>{t.after.slice(0, 60)}</span>
                          <span className="c-where">{t.repeats > 1 && groupList ? `${n(t.repeats)} places` : `${platformName[t.platform]}, ${t.channel_name}`}</span>
                          <span className="c-date">{dateTime(t.ts)}</span>
                        </div>
                      ) : (
                        <div className="xc-row"><span /><div className="placeholder" /></div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          </div>

          <div className="xc-preview detail">
            {current ? (
              <>
                <div className="xc-preview-head">
                  <span className={`risk risk-${current.severity}`}>{severityLabel(current.severity)}</span>
                  <h2>{current.label}</h2>
                  <span className="tag">{platformName[current.platform]}</span>
                  <span className="small muted">{current.channel_name}, {dateTime(current.ts)}</span>
                  {current.status && (
                    <span className="status-chip"><Icon name="check" size={14} />{current.status === "deleted" ? "Deleted" : current.status === "ok" ? "Not a problem" : "Saved for later"}</span>
                  )}
                </div>
                <div className="xc-preview-body">
                  <div className="xc-preview-text">
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
                    </p>
                    <p className="small muted advice-line"><strong>What to do: </strong>{current.advice}</p>
                    {current.repeats > 1 && !sameAs && (
                      <div className="group-box">
                        <div>
                          <strong>{groupList ? "This" : "The same"} {NOUN[current.kind]} appears in {n(current.repeats)} places</strong>
                          <span className="small muted">
                            {group
                              ? `${group.platforms.map((p) => `${platformName[p.platform]} ${n(p.count)}`).join(", ")}. ${yearSpan(group.first_ts, group.last_ts)}`
                              : "Counting…"}
                          </span>
                        </div>
                        <div className="toolbar">
                          {group && tab !== "done" && group.platforms
                            .map((p) => ({ p: p.platform, how: liveFor(p.platform), count: p.count }))
                            .filter((x) => x.how)
                            .map((x) =>
                              confirmGroupDelete === x.p ? (
                                <span key={x.p} className="toolbar">
                                  <button
                                    className="danger"
                                    onClick={() => {
                                      setConfirmGroupDelete(null);
                                      runLive(group.ids.map((id, i) => ({ id, message_id: group.message_ids[i], platform: group.member_platforms[i] })).filter((t) => t.platform === x.p));
                                    }}
                                  >
                                    <Icon name="trash" size={16} />Yes, delete them
                                  </button>
                                  <button className="quiet" onClick={() => setConfirmGroupDelete(null)}>Cancel</button>
                                </span>
                              ) : (
                                <button key={x.p} onClick={() => setConfirmGroupDelete(x.p)}><Icon name="trash" size={16} />{x.how!.ask(x.count)}</button>
                              ),
                            )}
                          <button className="quiet" onClick={() => { setSameAs(current); setFocus(0); }}>Show each place</button>
                        </div>
                      </div>
                    )}
                  </div>

                  <div className="xc-preview-actions">
                    <div className="go-row">
                      {current.platform === "pc" ? (
                        <>
                          <button className="primary" onClick={() => goTo(current)}><Icon name="folder" size={16} />Show in folder</button>
                          {!current.status &&
                            (confirmRecycle ? (
                              <>
                                <button className="danger" onClick={() => recycle(current)}><Icon name="trash" size={16} />Yes, to the Recycle Bin</button>
                                <button className="quiet" onClick={() => setConfirmRecycle(false)}>Cancel</button>
                              </>
                            ) : (
                              <button onClick={() => setConfirmRecycle(true)}><Icon name="trash" size={16} />Move to Recycle Bin…</button>
                            ))}
                          <button className="quiet" onClick={() => copyLink(current)}><Icon name="copy" size={16} />Copy path</button>
                        </>
                      ) : (
                        <>
                          {liveDelete(current) &&
                            (confirmRecycle ? (
                              <>
                                <button className="danger" onClick={() => deleteLive(current)}><Icon name="trash" size={16} />{liveDelete(current)!.confirm}</button>
                                <button className="quiet" onClick={() => setConfirmRecycle(false)}>Cancel</button>
                              </>
                            ) : (
                              <button className="primary" onClick={() => setConfirmRecycle(true)}><Icon name="trash" size={16} />{liveDelete(current)!.ask}</button>
                            ))}
                          {(current.link || !liveDelete(current)) && (
                            <button className={liveDelete(current) ? undefined : "primary"} onClick={() => goTo(current)}><Icon name="external" size={16} />{openLabel(current.platform)}</button>
                          )}
                          {current.link && <button onClick={() => copyLink(current)}><Icon name="copy" size={16} />Copy link</button>}
                          <button className="quiet" onClick={() => onOpen({ channelId: current.channel_id, messageId: current.message_id })}>
                            <Icon name="messages" size={16} />Show conversation
                          </button>
                        </>
                      )}
                    </div>
                    <div className="decide">
                      {current.status ? (
                        <button onClick={() => decide(current, null)}>Move back to review <span className="key">U</span></button>
                      ) : (
                        <div className="decide-row">
                          <button onClick={() => decideHere(current, "deleted")}><Icon name="check" size={16} />{groupIds(current) ? "I deleted them all" : "I deleted it"} <span className="key">D</span></button>
                          <button onClick={() => decideHere(current, "ok")}>Not a problem{groupIds(current) ? ` (all ${n(groupIds(current)!.length)})` : ""} <span className="key">N</span></button>
                          <button onClick={() => decideHere(current, "later")}><Icon name="clock" size={16} />Later <span className="key">L</span></button>
                        </div>
                      )}
                      {!current.status && (
                        <button className="link small" onClick={() => ignoreEverywhere(current)}>Never flag this {NOUN[current.kind]} again</button>
                      )}
                    </div>
                  </div>
                </div>
              </>
            ) : (
              <div className="detail-empty small muted">
                {scanned ? "Pick something in the list to see it here. Double-click opens it where it was posted." : ""}
              </div>
            )}
          </div>
        </div>
      </div>

      {guide && (
        <GuidePanel
          guide={guide}
          revealAll={revealAll}
          removeLabel={(f) => (f.platform === "pc" ? "Move to Recycle Bin" : liveDelete(f) ? (f.platform === "telegram" ? "Delete from Telegram" : "Move to Trash") : null)}
          onOpen={goTo}
          onRemove={guideRemove}
          onNext={guideNext}
          onClose={closeGuide}
        />
      )}
      {batch && (
        <div className="toast" role="status" aria-live="polite">
          <span>{batch.label}: {n(batch.done)} of {n(batch.total)}{batch.failed ? `, ${batch.failed} failed` : ""}</span>
          <button className="link" onClick={() => { stopBatch.current = true; }}>Stop</button>
        </div>
      )}
      {toast && !batch && (
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

const yearSpan = (a: number, b: number) => {
  const [x, y] = [new Date(a).getFullYear(), new Date(b).getFullYear()];
  return !a ? "" : x === y ? `All from ${x}.` : `From ${x} to ${y}.`;
};

function BulkBar(p: {
  count: number; grouped: boolean; tab: StatusTab; live: string | null;
  onDecide: (s: Status) => void; onGuide: () => void; onLive: () => void; onClear: () => void;
}) {
  const [confirm, setConfirm] = useState(false);
  return (
    <div className="bulk-bar" role="region" aria-label="Selected items">
      <strong>{plural(p.count, "selected", "selected")}</strong>
      {p.grouped && <span className="small muted">Includes every place each value appears.</span>}
      <span className="grow" />
      {p.live &&
        (confirm ? (
          <>
            <button className="danger" onClick={() => { setConfirm(false); p.onLive(); }}><Icon name="trash" size={16} />Yes, delete them</button>
            <button className="quiet" onClick={() => setConfirm(false)}>Cancel</button>
          </>
        ) : (
          <button onClick={() => setConfirm(true)}><Icon name="trash" size={16} />{p.live}</button>
        ))}
      {p.tab !== "done" && (
        <>
          <button onClick={p.onGuide} title="Open each one where it was posted, then mark it"><Icon name="arrow" size={16} />One by one</button>
          <button onClick={() => p.onDecide("deleted")}><Icon name="check" size={16} />I deleted them</button>
          <button onClick={() => p.onDecide("ok")}>Not a problem</button>
          {p.tab !== "later" && <button onClick={() => p.onDecide("later")}><Icon name="clock" size={16} />Later</button>}
        </>
      )}
      <button className="quiet" onClick={p.onClear}>Clear</button>
    </div>
  );
}

function WatchTerms(p: {
  terms: string[]; draft: string; demo: boolean; scanned: boolean; changed: boolean; scanning: boolean;
  onDraft: (s: string) => void; onAdd: () => void; onRemove: (t: string) => void; onScan: () => void; onClose: () => void;
}) {
  const empty = p.terms.length === 0;
  return (
    <div className="watch slim">
      <div className="watch-title">
        <h3>Watch terms</h3>
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
        {p.scanned && <button className="quiet" onClick={p.onClose}>Close</button>}
      </div>
      {p.changed && !p.scanning && <p className="small note">Your watch terms changed. Check again to include them. Your decisions are kept.</p>}
    </div>
  );
}

function GuidePanel(p: {
  guide: Guide; revealAll: boolean;
  removeLabel: (f: FindingView) => string | null;
  onOpen: (f: FindingView) => void; onRemove: () => void;
  onNext: (o: "deleted" | "ok" | "skipped") => void; onClose: () => void;
}) {
  const g = p.guide;
  const f = g.items[g.i];
  const [opened, setOpened] = useState(false);
  useEffect(() => setOpened(false), [g.i]);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).tagName === "TEXTAREA") return;
      if (e.key === "Escape") return p.onClose();
      if (!f || g.busy) return;
      if (e.key === "d") p.onNext("deleted");
      else if (e.key === "s") p.onNext("skipped");
      else if (e.key === "o") { p.onOpen(f); setOpened(true); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
  const remove = f ? p.removeLabel(f) : null;
  return (
    <div className="modal-backdrop" role="presentation">
      <div className="modal guide" role="dialog" aria-modal="true" aria-label="Clean up one by one">
        {f ? (
          <>
            <div className="guide-head">
              <strong>Clean up one by one</strong>
              <span className="muted small">{n(g.i + 1)} of {n(g.items.length)}</span>
              <span className="grow" />
              <button className="quiet" onClick={p.onClose}>Stop</button>
            </div>
            <div className="bar"><span style={{ width: `${(g.i / g.items.length) * 100}%` }} /></div>
            <div className="guide-item">
              <div className="guide-meta">
                <span className={`risk risk-${f.severity}`}>{severityLabel(f.severity)}</span>
                <strong>{f.label}</strong>
                <span className="tag">{platformName[f.platform]}</span>
                <span className="small muted">{f.channel_name}, {dateTime(f.ts)}</span>
              </div>
              <p className="quote">{f.before}<mark>{f.masked && !p.revealAll ? "•••" : f.matched}</mark>{f.after}</p>
              {f.advice && <p className="small"><strong>What to do:</strong> {f.advice}</p>}
            </div>
            {g.error && <p className="error small" role="alert">{g.error}</p>}
            <div className="guide-actions">
              {remove ? (
                <button className="primary" disabled={g.busy} onClick={p.onRemove}><Icon name="trash" size={16} />{g.busy ? "Removing…" : remove}</button>
              ) : (
                <button className={opened ? undefined : "primary"} onClick={() => { p.onOpen(f); setOpened(true); }}>
                  <Icon name="external" size={16} />{openLabel(f.platform)} <span className="key">O</span>
                </button>
              )}
              <button className={!remove && opened ? "primary" : undefined} disabled={g.busy} onClick={() => p.onNext("deleted")}>
                <Icon name="check" size={16} />I deleted it, next <span className="key">D</span>
              </button>
              <button disabled={g.busy} onClick={() => p.onNext("ok")}>Not a problem</button>
              <button className="quiet" disabled={g.busy} onClick={() => p.onNext("skipped")}>Skip <span className="key">S</span></button>
            </div>
            {!remove && f.platform !== "pc" && (
              <p className="small muted">Sompio opens the message in your browser or app. Delete it there, come back, and click "I deleted it".</p>
            )}
          </>
        ) : (
          <>
            <div className="guide-head"><strong>All done</strong></div>
            <p>
              {plural(g.deleted, "item removed", "items removed")}, {n(g.ok)} marked as not a problem, {n(g.skipped)} skipped.
              {g.skipped > 0 && " Skipped items are still under To review."}
            </p>
            <div className="guide-actions"><button className="primary" onClick={p.onClose}>Close</button></div>
          </>
        )}
      </div>
    </div>
  );
}

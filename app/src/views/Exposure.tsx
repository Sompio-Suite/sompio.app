import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useRef, useState } from "react";
import { api, listen, pickSavePath, type FindingView, type KindKey, type ScanSummary } from "../api";
import type { Jump } from "../App";
import { dateTime, n, plural } from "../format";
import { usePaged } from "../usePaged";

interface Props {
  scan: ScanSummary | null;
  onScanned: (s: ScanSummary) => void;
  onOpen: (j: Jump) => void;
}

export default function Exposure({ scan, onScanned, onOpen }: Props) {
  const [kind, setKind] = useState<KindKey | null>(null);
  const [reveal, setReveal] = useState(false);
  const [terms, setTerms] = useState<string[]>([]);
  const [draft, setDraft] = useState("");
  const [scanning, setScanning] = useState<[number, number] | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [scanVersion, setScanVersion] = useState(0);

  // The saved terms are the ones the last scan used.
  const [scannedTerms, setScannedTerms] = useState<string[]>([]);

  useEffect(() => {
    api.watchTerms().then((t) => {
      setTerms(t);
      setScannedTerms(t);
    });
    const un = listen<[number, number]>("scan-progress", setScanning);
    return () => void un.then((f) => f());
  }, []);

  const scanned = scan?.scanned_at_ms != null;
  const listKey = scanned ? `${kind}|${reveal}|${scanVersion}` : null;
  const { total, items, ensure } = usePaged<FindingView>(listKey, (o, l) => api.findings(kind, o, l, reveal));

  const runScan = async (list = terms) => {
    setScanning([0, 1]);
    setNote(null);
    try {
      onScanned(await api.runScan(list));
      setScannedTerms(list);
      setScanVersion((v) => v + 1);
    } catch (e) {
      setNote(`The scan stopped: ${e}`);
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

  const termsChanged = scanned && JSON.stringify(terms) !== JSON.stringify(scannedTerms);

  const exportList = async () => {
    const path = await pickSavePath("veil-flagged-messages.csv");
    if (!path) return;
    const count = await api.exportFlagged(path);
    setNote(`Saved ${plural(count, "message ID", "message IDs")}. The file holds IDs and dates only, no message text.`);
  };

  const parentRef = useRef<HTMLDivElement>(null);
  const v = useVirtualizer({
    count: total ?? 0,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 92,
    overscan: 8,
  });
  const virtualItems = v.getVirtualItems();
  useEffect(() => {
    virtualItems.forEach((r) => !items[r.index] && ensure(r.index));
  }, [virtualItems, items, ensure]);

  const pct = scanning && scanning[1] > 0 ? (scanning[0] / scanning[1]) * 100 : 0;

  return (
    <div className="page full" style={{ display: "flex" }}>
      <div className="lede">
        {!scanned ? (
          <h1>Find personal information you've posted.</h1>
        ) : scan!.flagged_messages > 0 ? (
          <h1>{plural(scan!.flagged_messages, "message contains", "messages contain")} something personal.</h1>
        ) : (
          <h1>No personal information found.</h1>
        )}
        <p className="muted">
          Veil looks for email addresses, phone numbers, street addresses, bank and card numbers, passwords and access
          tokens, plus anything you add below. Matches are hidden until you choose to show them.
        </p>
      </div>

      <div className="watch">
        <h3>Watch terms</h3>
        <p className="muted small">
          Things only you know are sensitive: your real name, street, school or employer. They stay on this computer.
        </p>
        {terms.length > 0 && (
          <div className="chips">
            {terms.map((t) => (
              <span className="chip" key={t}>
                {t}
                <button aria-label={`Remove ${t}`} onClick={() => setTerms(terms.filter((x) => x !== t))}>×</button>
              </span>
            ))}
          </div>
        )}
        <div className="watch-row">
          <input
            type="text"
            value={draft}
            placeholder="Add a term, then press Enter"
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && addTerm()}
            aria-label="New watch term"
          />
          <button onClick={addTerm} disabled={draft.trim().length < 2}>Add</button>
          <button className="primary" onClick={() => runScan()} disabled={!!scanning}>
            {scanned ? "Scan again" : "Start scan"}
          </button>
        </div>
        {termsChanged && !scanning && <p className="small muted">Your watch terms changed. Scan again to include them.</p>}
        {scanning && (
          <div className="progress" role="status" aria-live="polite">
            <span className="small">Scanning {n(Math.round(scanning[0]))} of {n(scanning[1])} messages</span>
            <div className="bar"><span style={{ width: `${pct}%` }} /></div>
          </div>
        )}
      </div>

      {scanned && (
        <div className="exposure">
          <div className="kinds" role="group" aria-label="Filter by type">
            <button className="kind" aria-pressed={kind === null} onClick={() => setKind(null)}>
              <span />
              All findings
              <span className="num">{n(scan!.total_findings)}</span>
            </button>
            {scan!.by_kind.map((k) => (
              <button
                key={k.kind}
                className="kind"
                aria-pressed={kind === k.kind}
                disabled={k.count === 0}
                onClick={() => setKind(k.kind)}
              >
                <span className={`sev sev-${k.severity}`} aria-hidden />
                {k.label}
                <span className="num">{n(k.count)}</span>
              </button>
            ))}
          </div>

          <div style={{ display: "flex", flexDirection: "column", minHeight: 0, gap: "0.75rem" }}>
            <div className="toolbar">
              <label className="switch">
                <input type="checkbox" checked={reveal} onChange={(e) => setReveal(e.target.checked)} />
                Show hidden text
              </label>
              <span className="grow" />
              <button onClick={exportList} disabled={!scan!.flagged_messages}>Save list of flagged messages</button>
            </div>
            {note && <p className="small muted">{note}</p>}

            <div className="list" ref={parentRef}>
              {total === 0 && <div className="empty">Nothing of this type was found.</div>}
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
                      {f ? <FindingRow f={f} onOpen={onOpen} /> : <div className="finding"><div className="placeholder" /></div>}
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function FindingRow({ f, onOpen }: { f: FindingView; onOpen: (j: Jump) => void }) {
  return (
    <article className="finding">
      <div className="finding-meta">
        <span className={`sev sev-${f.severity}`} aria-label={["", "Low", "Medium", "High"][f.severity] + " risk"} />
        <span className="where" title={f.channel_name}>{f.channel_name}</span>
        <time>{dateTime(f.ts)}</time>
        <span>{f.label}</span>
        <span className="grow" style={{ flex: 1 }} />
        <button className="link small" onClick={() => onOpen({ channelId: f.channel_id, messageId: f.message_id })}>
          Open in conversation
        </button>
      </div>
      <p className="snippet">
        {f.before}
        <span className={`redact${f.masked ? "" : " shown"}`}>{f.matched}</span>
        {f.after}
      </p>
    </article>
  );
}

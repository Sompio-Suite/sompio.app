import { useEffect, useState } from "react";
import { api, type ScanSummary, type Stats } from "../api";
import type { Jump, View } from "../App";
import { channelKind, month, n, plural } from "../format";

interface Props {
  scan: ScanSummary | null;
  onOpen: (j: Jump) => void;
  onGo: (v: View) => void;
}

export default function Overview({ scan, onOpen, onGo }: Props) {
  const [stats, setStats] = useState<Stats | null>(null);
  useEffect(() => {
    api.stats().then(setStats);
  }, []);
  if (!stats) return null;

  const maxYear = Math.max(1, ...stats.per_year.map((y) => y.count));
  const scanned = scan?.scanned_at_ms != null;

  return (
    <div className="page">
      <div className="lede">
        <h1>
          You've sent {plural(stats.messages, "message", "messages")} since {month(stats.first_ts)}.
        </h1>
        <div className="facts">
          <Fact count={stats.servers} one="server" many="servers" />
          <Fact count={stats.dms} one="direct message" many="direct messages" />
          <Fact count={stats.group_dms} one="group chat" many="group chats" />
          <Fact count={stats.with_attachments} one="message with files" many="messages with files" />
        </div>
      </div>

      {!scanned ? (
        <div className="callout">
          <div>
            <h3>Check for personal information</h3>
            <p className="muted">Scanning takes a few seconds and happens entirely on this computer.</p>
          </div>
          <button className="primary" onClick={() => onGo("exposure")}>Start a scan</button>
        </div>
      ) : scan!.flagged_messages > 0 ? (
        <div className="callout alert">
          <div>
            <h3>{plural(scan!.flagged_messages, "message needs", "messages need")} a look</h3>
            <p className="muted">They contain things like phone numbers, addresses or passwords.</p>
          </div>
          <button className="primary" onClick={() => onGo("exposure")}>Review them</button>
        </div>
      ) : (
        <div className="callout">
          <div>
            <h3>Nothing obvious turned up</h3>
            <p className="muted">Add your real name or street as watch terms to check for those too.</p>
          </div>
          <button onClick={() => onGo("exposure")}>Add watch terms</button>
        </div>
      )}

      <div className="split">
        <section className="years" aria-labelledby="years-h">
          <h3 id="years-h" style={{ marginBottom: "0.75rem" }}>Messages per year</h3>
          {stats.per_year.map((y) => (
            <div className="year" key={y.year}>
              <span className="num">{y.year}</span>
              <div className="track" aria-hidden>
                <span style={{ width: `${(y.count / maxYear) * 100}%` }} />
              </div>
              <span className="count num">{n(y.count)}</span>
            </div>
          ))}
        </section>

        <section aria-labelledby="top-h">
          <h3 id="top-h" style={{ marginBottom: "0.5rem" }}>Where you talk most</h3>
          <div className="rows">
            {stats.top_channels.map((c) => (
              <button key={c.id} className="row-btn" onClick={() => onOpen({ channelId: c.id })}>
                <span>
                  <span className="row-title" style={{ display: "block" }}>{c.name}</span>
                  <span className="small muted">{channelKind[c.kind]}</span>
                </span>
                <span className="num muted">{n(c.message_count)}</span>
              </button>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}

function Fact({ count, one, many }: { count: number; one: string; many: string }) {
  return (
    <span>
      <b className="num">{n(count)}</b> {count === 1 ? one : many}
    </span>
  );
}

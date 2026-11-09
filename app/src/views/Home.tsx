import { useEffect, useState } from "react";
import { api, platformName, severityLabel, type KindKey, type Platform, type ScanSummary, type Stats } from "../api";
import type { Jump, View } from "../App";
import { channelKind, month, n, plural } from "../format";
import Icon from "../Icon";

interface Props {
  scan: ScanSummary | null;
  onReview: (kind: KindKey | null) => void;
  onGo: (v: View) => void;
  onOpen: (j: Jump) => void;
}

const ALL_SOURCES: Platform[] = ["discord", "reddit", "telegram", "x", "pc"];
const firstSentence = (s: string) => s.split(/(?<=\.)\s/)[0];

export default function Home({ scan, onReview, onGo, onOpen }: Props) {
  const [stats, setStats] = useState<Stats | null>(null);
  const [terms, setTerms] = useState<string[] | null>(null);
  useEffect(() => {
    api.stats().then(setStats);
    api.watchTerms().then(setTerms);
  }, []);
  if (!stats || terms === null) return null;

  const scanned = scan?.scanned_at_ms != null;
  const total = scan?.total_findings ?? 0;
  const open = scan?.open ?? 0;
  const urgent = scan?.by_kind.filter((k) => k.severity === 3).reduce((a, k) => a + k.open, 0) ?? 0;
  const kinds = (scan?.by_kind ?? [])
    .filter((k) => k.open > 0)
    .sort((a, b) => b.severity - a.severity || b.open - a.open);
  const missing = ALL_SOURCES.filter((p) => !stats.sources.some((s) => s.platform === p));
  const maxYear = Math.max(1, ...stats.per_year.map((y) => y.count));

  return (
    <div className="page">
      <section className="home-hero">
        {!scanned ? (
          <h1>Let's check what you've shared.</h1>
        ) : open > 0 ? (
          <h1>{plural(open, "thing needs", "things need")} your attention.</h1>
        ) : total > 0 ? (
          <h1>You're all caught up.</h1>
        ) : (
          <h1>Nothing personal turned up.</h1>
        )}
        <p className="muted">
          {scanned && urgent > 0 && <>{plural(urgent, "of them is", "of them are")} worth fixing soon. </>}
          Sompio checked {stats.messages > 0 && plural(stats.messages, "message", "messages")}
          {stats.messages > 0 && stats.files > 0 && " and "}
          {stats.files > 0 && plural(stats.files, "file", "files")} from {stats.sources.map((s) => platformName[s.platform]).join(", ")}.
        </p>
        {scanned && total > 0 && (
          <div className="meter">
            <div className="bar" aria-hidden><span style={{ width: `${(scan!.done / total) * 100}%` }} /></div>
            <span className="num">{n(scan!.done)} of {n(total)} reviewed</span>
          </div>
        )}
      </section>

      <section className="section">
        <h2 className="section-title">Next steps</h2>
        <div className="steps-list">
          {kinds.map((k) => (
            <button key={k.kind} className="step" onClick={() => onReview(k.kind)}>
              <span className={`step-icon risk-${k.severity}`}><Icon name="review" /></span>
              <span className="step-text">
                <strong>{k.label}</strong>
                <span>
                  <span className={`risk risk-${k.severity}`}>{severityLabel(k.severity)}</span>
                  {"  "}{n(k.open)} to review. {firstSentence(k.advice)}
                </span>
              </span>
              <span className="step-go"><Icon name="arrow" /></span>
            </button>
          ))}
          {scanned && terms.length === 0 && (
            <button className="step" onClick={() => onReview(null)}>
              <span className="step-icon"><Icon name="search" /></span>
              <span className="step-text">
                <strong>Add your name and street as watch terms</strong>
                <span>Patterns can't recognise your real name. Tell Sompio what to look for and it checks everything again.</span>
              </span>
              <span className="step-go"><Icon name="arrow" /></span>
            </button>
          )}
          {missing.length > 0 && (
            <button className="step" onClick={() => onGo("sources")}>
              <span className="step-icon"><Icon name="plus" /></span>
              <span className="step-text">
                <strong>Add more sources</strong>
                <span>Not added yet: {missing.map((p) => platformName[p]).join(", ")}.</span>
              </span>
              <span className="step-go"><Icon name="arrow" /></span>
            </button>
          )}
          {scanned && kinds.length === 0 && (
            <div className="step" role="note">
              <span className="step-icon good"><Icon name="check" /></span>
              <span className="step-text">
                <strong>Nothing left to review</strong>
                <span>Check back after adding a newer export, or let Routines remind you later.</span>
              </span>
              <span />
            </div>
          )}
        </div>
      </section>

      {stats.messages > 0 && (
        <section className="section">
          <h2 className="section-title">Your history</h2>
          <div className="split">
            <div className="years" aria-label="Messages per year">
              <p className="small muted" style={{ marginBottom: "0.4rem" }}>
                Messages per year since {month(stats.first_ts)}
              </p>
              {stats.per_year.map((y) => (
                <div className="year" key={y.year}>
                  <span className="num faint">{y.year}</span>
                  <div className="track" aria-hidden><span style={{ width: `${(y.count / maxYear) * 100}%` }} /></div>
                  <span className="count-n num">{n(y.count)}</span>
                </div>
              ))}
            </div>
            <div>
              <p className="small muted" style={{ marginBottom: "0.2rem" }}>Where you talk most</p>
              <div className="rows">
                {stats.top_channels.slice(0, 6).map((c) => (
                  <button key={c.id} className="row-btn" onClick={() => onOpen({ channelId: c.id })}>
                    <span style={{ minWidth: 0 }}>
                      <span className="row-title" style={{ display: "block" }}>{c.name}</span>
                      <span className="small faint">{platformName[c.platform]}, {channelKind[c.kind].toLowerCase()}</span>
                    </span>
                    <span className="num muted small">{n(c.message_count)}</span>
                  </button>
                ))}
              </div>
            </div>
          </div>
        </section>
      )}
    </div>
  );
}

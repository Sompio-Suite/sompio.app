import { useEffect, useState } from "react";
import { api, openExternal, type LicenceStatus, type Occurrence, type ScanSummary } from "../api";
import { date, n, plural } from "../format";
import Icon from "../Icon";
import { accessText, BUY_URL, FEEDBACK_URL, SUITE } from "../licence";
import type { View } from "../App";

interface Props {
  licence: LicenceStatus | null;
  scan: ScanSummary | null;
  cleanReady: boolean;
  onGo: (v: View) => void;
}

export default function Overview({ licence, scan, cleanReady, onGo }: Props) {
  const now = licence?.now ?? Date.now();
  const trialOnly = licence?.apps.every((a) => a.trial_only) ?? false;
  const anyActive = licence?.apps.some((a) => a.active) ?? false;
  const [next, setNext] = useState<Occurrence | null>(null);
  useEffect(() => { api.calNext(1).then((n) => setNext(n[0] ?? null)).catch(() => {}); }, []);

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>Sompio Suite</h1>
          <p className="muted">Private tools that keep your data on this computer, locked to you.</p>
        </div>
      </div>

      {licence?.beta && (
        <div className="notice">
          <Icon name="sparkle" />
          <div>
            <strong>This is the Sompio beta: everything is unlocked, free.</strong>
            <span>
              Your 30-day free trial starts when Sompio launches. It's a beta, so things can still go wrong: keep a copy of
              your passwords (Vault, then Auto-type, import and export, then Save as KeePass file) and your recovery sheet.
              Found a problem or have an idea? Send it with the button.
            </span>
          </div>
          <div className="notice-actions">
            <button onClick={() => openExternal(FEEDBACK_URL).catch(() => {})}>Send feedback</button>
          </div>
        </div>
      )}
      {licence && !licence.beta && trialOnly && anyActive && licence.trial_ends_at && (
        <div className="notice">
          <Icon name="sparkle" />
          <div>
            <strong>You're on the free trial until {date(licence.trial_ends_at)}.</strong>
            <span>Everything is unlocked. Add a key before then to keep going; nothing you add here is lost either way.</span>
          </div>
          <div className="notice-actions">
            <button onClick={() => openExternal(BUY_URL).catch(() => {})}>Get a key</button>
            <button onClick={() => onGo("licences")}>Add a key</button>
          </div>
        </div>
      )}
      {licence && !anyActive && (
        <div className="notice warn">
          <Icon name="lock" />
          <div>
            <strong>Your access has ended.</strong>
            <span>Everything you added is still here and readable. Add a key to keep cleaning and syncing.</span>
          </div>
          <div className="notice-actions">
            <button className="primary" onClick={() => openExternal(BUY_URL).catch(() => {})}>Get a key</button>
            <button onClick={() => onGo("licences")}>Add a key</button>
          </div>
        </div>
      )}

      <div className="suite-grid">
        {SUITE.map((app) => {
          const access = accessText(licence?.apps.find((a) => a.app === app.key), now, licence?.beta);
          return (
            <article key={app.key} className={`suite-card${app.ready ? "" : " soon"}`}>
              <div className="suite-card-top">
                <span className="suite-icon"><Icon name={app.icon} /></span>
                <div>
                  <h2>{app.name}</h2>
                  <span className={`access access-${app.ready ? access.tone : "none"}`}>{app.ready ? access.text : app.when}</span>
                </div>
              </div>
              <p>{app.line}</p>
              {app.key === "clean" && cleanReady && scan && scan.open > 0 && (
                <p className="small"><strong>{plural(scan.open, "thing", "things")} to review</strong> from {n(scan.scanned)} scanned.</p>
              )}
              {app.key === "calendar" && next && (
                <p className="small"><strong>Next: {next.title}</strong> {next.at.slice(0, 10) === new Date().toLocaleDateString("sv") ? "at" : `on ${new Date(next.at).toLocaleDateString(undefined, { weekday: "long" })} at`} {next.at.slice(11, 16)}</p>
              )}
              <div className="suite-card-foot">
                {app.ready ? (
                  app.key === "clean" ? (
                    <button className="primary" onClick={() => onGo("home")}>{cleanReady ? "Open Trace" : "Set up Trace"}</button>
                  ) : (
                    <button className="primary" onClick={() => onGo(app.key as View)}>Open {app.name}</button>
                  )
                ) : (
                  <button onClick={() => onGo(app.key as View)}>See what's planned</button>
                )}
              </div>
            </article>
          );
        })}
      </div>

      <section className="promise">
        <div><Icon name="lock" /><strong>Stays on this computer</strong><span>No account is needed to use the apps. Your data is never uploaded.</span></div>
        <div><Icon name="ticket" /><strong>Keys, not logins</strong><span>A key unlocks what you bought. It's checked here, offline.</span></div>
        <div><Icon name="check" /><strong>Never held hostage</strong><span>When a key runs out, everything stays readable and exportable.</span></div>
      </section>
    </div>
  );
}

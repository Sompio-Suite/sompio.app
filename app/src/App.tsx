import { useCallback, useEffect, useState, type ReactNode } from "react";
import { api, type KindKey, type LicenceStatus, type ScanSummary } from "./api";
import { n } from "./format";
import Icon, { type IconName } from "./Icon";
import Welcome from "./views/Welcome";
import Home from "./views/Home";
import Review from "./views/Review";
import Messages from "./views/Messages";
import Settings from "./views/Settings";
import Sources from "./views/Sources";
import Overview from "./views/Overview";
import Licences from "./views/Licences";
import Upcoming from "./views/Upcoming";
import { SUITE, accessText } from "./licence";

/** Suite pages, then Clean's pages. */
export type View =
  | "overview" | "calendar" | "vault" | "voice" | "licences"
  | "home" | "review" | "messages" | "sources" | "settings";
const CLEAN_VIEWS: View[] = ["home", "review", "messages", "sources", "settings"];
export interface Jump { channelId: string; messageId?: string }

export default function App() {
  const [ready, setReady] = useState<boolean | null>(null);
  const [demo, setDemo] = useState(false);
  const [view, setView] = useState<View>("overview");
  const [licence, setLicence] = useState<LicenceStatus | null>(null);
  const [jump, setJump] = useState<Jump | null>(null);
  const [reviewKind, setReviewKind] = useState<KindKey | null>(null);
  const [scan, setScan] = useState<ScanSummary | null>(null);
  const [version, setVersion] = useState(0); // bumps after data changes so views reload
  const [connected, setConnected] = useState<string[]>([]);
  const checkConnections = useCallback(async () => {
    const [tg, mail] = await Promise.all([api.tgStatus().catch(() => null), api.emailStatus().catch(() => null)]);
    setConnected([...(tg?.connected ? ["Telegram"] : []), ...(mail?.connected ? ["email"] : [])]);
  }, []);

  const refresh = useCallback(async () => {
    const has = await api.hasData();
    setReady(has);
    if (has) {
      setScan(await api.scanSummary());
      setDemo(await api.isDemo());
    }
  }, []);

  useEffect(() => {
    refresh();
    checkConnections();
    api.licenceStatus().then(setLicence).catch(() => {});
  }, [refresh, checkConnections]);

  const openChannel = (j: Jump) => {
    setJump(j);
    setView("messages");
  };
  const openReview = (kind: KindKey | null) => {
    setReviewKind(kind);
    setView("review");
  };

  const leave = async () => {
    await api.wipeData();
    setReady(false);
    setDemo(false);
    setScan(null);
    setVersion((v) => v + 1);
  };

  if (ready === null) return null;

  const toReview = scan ? scan.open : 0;
  const cleanAccess = licence?.apps.find((a) => a.app === "clean");
  const cleanLocked = licence != null && !cleanAccess?.active;
  const inClean = CLEAN_VIEWS.includes(view);

  const navButton = (id: View, label: string, icon: string, extra?: ReactNode, sub = false) => (
    <button
      key={id}
      className={`nav${sub ? " sub" : ""}`}
      aria-current={view === id ? "page" : undefined}
      aria-label={label}
      onClick={() => (id === "review" ? openReview(null) : setView(id))}
    >
      <Icon name={icon as IconName} />
      <span className="label">{label}</span>
      {extra}
    </button>
  );

  const cleanPage = () => {
    if (!ready) {
      return (
        <Welcome
          onReady={async (summary) => {
            setScan(summary);
            setDemo(await api.isDemo());
            setReady(true);
            setVersion((v) => v + 1);
            setView("home");
          }}
        />
      );
    }
    switch (view) {
      case "home":
        return <Home key={version} scan={scan} onReview={openReview} onGo={setView} onOpen={openChannel} />;
      case "review":
        return <Review key={`${version}-${reviewKind}`} scan={scan} demo={demo} initialKind={reviewKind} onScan={setScan} onOpen={openChannel} />;
      case "messages":
        return <Messages key={version} jump={jump} onJumpHandled={() => setJump(null)} />;
      case "sources":
        return (
          <Sources
            demo={demo}
            onScan={setScan}
            onChanged={async () => {
              setVersion((v) => v + 1);
              checkConnections();
              const has = await api.hasData();
              if (!has) return leave();
              setDemo(await api.isDemo());
            }}
          />
        );
      default:
        return <Settings scan={scan} demo={demo} onScan={setScan} onLeave={leave} />;
    }
  };

  return (
    <div className="shell">
      <nav className="rail" aria-label="Sections">
        <div className="brand">
          <svg className="brand-mark" viewBox="0 0 26 26" aria-hidden>
            <rect width="26" height="26" rx="8" />
            <path d="M5 18.5 10.2 11l3 4 2.8-3.8 5 7.3" />
          </svg>
          <span className="brand-name">Sompio</span>
        </div>
        {navButton("overview", "Overview", "grid")}
        <div className="rail-group">
          {navButton("home", "Clean", "review", cleanLocked ? <Icon name="lock" size={14} /> : undefined)}
          {ready && inClean && (
            <>
              {navButton("review", "Review", "check", toReview > 0 ? <span className="count" aria-label={`${toReview} to review`}>{n(toReview)}</span> : undefined, true)}
              {navButton("messages", "Messages", "messages", undefined, true)}
              {navButton("sources", "Sources", "sources", undefined, true)}
              {navButton("settings", "Settings", "settings", undefined, true)}
            </>
          )}
        </div>
        {SUITE.filter((a) => !a.ready).map((a) => navButton(a.key as View, a.name, a.icon, <span className="soon-dot" title={a.when} />))}
        <div className="rail-spacer" />
        {navButton("licences", "Licences", "ticket")}
        <div className="rail-foot">
          <Icon name="lock" size={16} />
          {connected.length > 0 ? (
            <div>
              <strong>{connected.length > 1 ? "2 accounts connected" : connected[0] === "email" ? "Email connected" : "Telegram connected"}</strong>
              Sompio goes online only to sync or delete on {connected.join(" and ")}. Nothing is uploaded.
            </div>
          ) : (
            <div>
              <strong>Offline</strong>
              Nothing you add here leaves this PC.
            </div>
          )}
        </div>
      </nav>
      <main>
        {inClean && cleanLocked && (
          <div className="demo-bar locked-bar" role="status">
            <span>
              <Icon name="lock" size={16} /> Clean is read-only: {accessText(cleanAccess, licence?.now ?? Date.now()).text.toLowerCase()}. You can look at
              everything, but not import, scan or delete.
            </span>
            <button onClick={() => setView("licences")}>Add a key</button>
          </div>
        )}
        {inClean && demo && (
          <div className="demo-bar" role="status">
            <span>You're looking at sample data. Nothing here is yours.</span>
            <button onClick={leave}>Use my own data</button>
          </div>
        )}
        {view === "overview" && <Overview licence={licence} scan={scan} cleanReady={ready} onGo={setView} />}
        {view === "licences" && <Licences licence={licence} onChanged={setLicence} />}
        {(view === "calendar" || view === "vault" || view === "voice") && <Upcoming app={SUITE.find((a) => a.key === view)!} />}
        {inClean && cleanPage()}
      </main>
    </div>
  );
}

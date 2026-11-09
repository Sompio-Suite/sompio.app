import { useCallback, useEffect, useState, type ReactNode } from "react";
import { api, listen, type KindKey, type LicenceStatus, type LockStatus, type ScanSummary } from "./api";
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
import LockScreen from "./views/LockScreen";
import Security from "./views/Security";
import Calendar from "./views/Calendar";
import Vault from "./views/Vault";
import Samples from "./views/Samples";
import { SUITE, accessText } from "./licence";
import { autoCheckEnabled, checkForUpdate, dueForCheck, type UpdateInfo } from "./updates";

/** Suite pages, then Clean's pages. */
export type View =
  | "overview" | "calendar" | "vault" | "voice" | "licences" | "security" | "samples"
  | "home" | "review" | "messages" | "sources" | "settings";
const CLEAN_VIEWS: View[] = ["home", "review", "messages", "sources", "settings"];
export interface Jump { channelId: string; messageId?: string }

export default function App() {
  const [ready, setReady] = useState<boolean | null>(null);
  const [demo, setDemo] = useState(false);
  const [view, setView] = useState<View>("overview");
  const [licence, setLicence] = useState<LicenceStatus | null>(null);
  const [lock, setLock] = useState<LockStatus | null>(null);
  const [idleMinutes, setIdleMinutes] = useState(() => {
    try {
      const v = localStorage.getItem("sompio.idleMinutes");
      return v == null ? 15 : Number(v);
    } catch {
      return 15;
    }
  });
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
    api.lockStatus().then(setLock).catch(() => setLock({ set_up: true, unlocked: false, remembers_picture: false }));
    api.licenceStatus().then(setLicence).catch(() => {});
  }, []);
  const unlocked = lock?.unlocked ?? false;

  // Updates: checked quietly at start and every few hours, if the person hasn't turned it off.
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [installing, setInstalling] = useState<number | null | "error">(null);
  useEffect(() => {
    const run = () => {
      if (autoCheckEnabled() && dueForCheck()) checkForUpdate().then(setUpdate).catch(() => {});
    };
    run();
    const t = window.setInterval(run, 3_600_000);
    return () => window.clearInterval(t);
  }, []);
  const installUpdate = async () => {
    if (!update) return;
    setInstalling(0);
    try {
      await update.install((p) => setInstalling(p));
    } catch {
      setInstalling("error");
    }
  };
  useEffect(() => {
    if (!unlocked) return;
    refresh();
    checkConnections();
  }, [unlocked, refresh, checkConnections]);

  const lockNow = useCallback(async () => {
    await api.lockLock().catch(() => {});
    setLock(await api.lockStatus());
    setReady(null);
    setScan(null);
    setVersion((v) => v + 1);
  }, []);
  // "Lock Sompio" in the tray menu
  useEffect(() => {
    const un = listen("locked", () => lockNow());
    return () => { un.then((f) => f()); };
  }, [lockNow]);

  // Lock after a while without use
  useEffect(() => {
    try {
      localStorage.setItem("sompio.idleMinutes", String(idleMinutes));
    } catch {
      /* not remembered, fine */
    }
    if (!unlocked || idleMinutes <= 0) return;
    let last = Date.now();
    const seen = () => { last = Date.now(); };
    const events = ["mousemove", "keydown", "mousedown", "wheel", "touchstart"];
    events.forEach((e) => window.addEventListener(e, seen, { passive: true }));
    const timer = window.setInterval(() => {
      if (Date.now() - last > idleMinutes * 60_000) lockNow();
    }, 20_000);
    return () => {
      events.forEach((e) => window.removeEventListener(e, seen));
      window.clearInterval(timer);
    };
  }, [unlocked, idleMinutes, lockNow]);

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

  if (!lock) return null;
  if (!lock.set_up || !lock.unlocked) {
    return <LockScreen key={String(lock.set_up)} status={lock} onUnlocked={async () => setLock(await api.lockStatus())} />;
  }
  if (ready === null) return null;

  const toReview = scan ? scan.open : 0;
  const cleanAccess = licence?.apps.find((a) => a.app === "clean");
  const cleanLocked = licence != null && !cleanAccess?.active;
  const inClean = CLEAN_VIEWS.includes(view);
  const vaultLocked = licence != null && !licence.apps.find((a) => a.app === "vault")?.active;
  const calendarLocked = licence != null && !licence.apps.find((a) => a.app === "calendar")?.active;

  const navButton = (id: View, label: string, icon: string, extra?: ReactNode, sub = false) => (
    <button
      key={id}
      className={`nav${sub ? " sub" : ""}`}
      aria-current={view === id || (id === "home" && inClean) ? "page" : undefined}
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
          <svg className="brand-mark" viewBox="0 0 120 120" aria-hidden>
            <path className="sun-top" d="M14 56 A36 36 0 0 1 86 56 Z" />
            <path className="sun-bottom" d="M34 64 A36 36 0 0 0 106 64 Z" />
          </svg>
          <span className="brand-name">sompio</span>
        </div>
        {navButton("overview", "Overview", "grid")}
        {navButton("home", "Clean", "review", cleanLocked ? <Icon name="lock" size={14} /> : toReview > 0 && !inClean ? <span className="count" aria-label={`${toReview} to review`}>{n(toReview)}</span> : undefined)}
        {navButton("calendar", "Calendar", "calendar", calendarLocked ? <Icon name="lock" size={14} /> : undefined)}
        {navButton("vault", "Vault", "key", vaultLocked ? <Icon name="lock" size={14} /> : undefined)}
        {SUITE.filter((a) => !a.ready).map((a) => navButton(a.key as View, a.name, a.icon, <span className="soon-dot" title={a.when} />))}
        <div className="rail-spacer" />
        {navButton("samples", "Samples", "sparkle")}
        {navButton("security", "Security", "lock")}
        {navButton("licences", "Licences", "ticket")}
        {update && (
          <div className="update-card" role="status">
            <strong>Sompio {update.version} is ready</strong>
            {installing === null && <span>Your data stays as it is.</span>}
            {typeof installing === "number" && <span>Downloading{installing ? ` ${installing}%` : "…"}. Sompio restarts by itself.</span>}
            {installing === "error" && <span>The update didn't install. Try again later.</span>}
            <button className="primary" disabled={typeof installing === "number"} onClick={installUpdate}>
              {typeof installing === "number" ? "Updating…" : "Update now"}
            </button>
          </div>
        )}
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
              Your data never leaves this PC.
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
        {inClean && (
          <div className="app-head">
            <div className="app-title">
              <h1>Clean</h1>
              <span className="muted">Find and remove what you've shared over the years.</span>
            </div>
            {ready && (
              <div className="segmented" role="tablist" aria-label="Clean">
                {([["home", "Overview"], ["review", "Review"], ["messages", "Messages"], ["sources", "Sources"], ["settings", "Settings"]] as [View, string][]).map(([id, label]) => (
                  <button key={id} role="tab" aria-selected={view === id} onClick={() => (id === "review" ? openReview(null) : setView(id))}>
                    {label}
                    {id === "review" && toReview > 0 && <span className="count">{n(toReview)}</span>}
                  </button>
                ))}
              </div>
            )}
          </div>
        )}
        {view === "overview" && <Overview licence={licence} scan={scan} cleanReady={ready} onGo={setView} />}
        {view === "licences" && <Licences licence={licence} onChanged={setLicence} />}
        {view === "security" && (
          <Security
            status={lock}
            idleMinutes={idleMinutes}
            onIdleMinutes={setIdleMinutes}
            onLockNow={lockNow}
            onChanged={async () => setLock(await api.lockStatus())}
            onUpdateFound={setUpdate}
          />
        )}
        {view === "calendar" && <Calendar readOnly={calendarLocked} onAddKey={() => setView("licences")} />}
        {view === "vault" && <Vault readOnly={vaultLocked} onAddKey={() => setView("licences")} />}
        {view === "samples" && (
          <Samples
            cleanHasData={!!ready && !demo}
            onCleanDemo={async () => {
              await api.loadDemo();
              await refresh();
              setVersion((v) => v + 1);
              setView("home");
            }}
            onGo={setView}
          />
        )}
        {view === "voice" && <Upcoming app={SUITE.find((a) => a.key === view)!} />}
        {inClean && cleanPage()}
      </main>
    </div>
  );
}

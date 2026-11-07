import { useCallback, useEffect, useState } from "react";
import { api, type KindKey, type ScanSummary } from "./api";
import { n } from "./format";
import Icon, { type IconName } from "./Icon";
import Welcome from "./views/Welcome";
import Home from "./views/Home";
import Review from "./views/Review";
import Messages from "./views/Messages";
import Settings from "./views/Settings";
import Sources from "./views/Sources";

export type View = "home" | "review" | "messages" | "sources" | "settings";
export interface Jump { channelId: string; messageId?: string }

export default function App() {
  const [ready, setReady] = useState<boolean | null>(null);
  const [demo, setDemo] = useState(false);
  const [view, setView] = useState<View>("home");
  const [jump, setJump] = useState<Jump | null>(null);
  const [reviewKind, setReviewKind] = useState<KindKey | null>(null);
  const [scan, setScan] = useState<ScanSummary | null>(null);
  const [version, setVersion] = useState(0); // bumps after data changes so views reload

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
  }, [refresh]);

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

  const nav: [View, string, IconName][] = [
    ["home", "Home", "home"],
    ["review", "Review", "review"],
    ["messages", "Messages", "messages"],
    ["sources", "Sources", "sources"],
    ["settings", "Settings", "settings"],
  ];
  const toReview = scan ? scan.open : 0;

  return (
    <div className="shell">
      <nav className="rail" aria-label="Sections">
        <div className="brand">
          <span className="brand-mark" aria-hidden /> <span className="brand-name">Veil</span>
        </div>
        {nav.map(([id, label, icon]) => (
          <button
            key={id}
            className="nav"
            aria-current={view === id ? "page" : undefined}
            aria-label={label}
            onClick={() => (id === "review" ? openReview(null) : setView(id))}
          >
            <Icon name={icon} />
            <span className="label">{label}</span>
            {id === "review" && toReview > 0 && (
              <span className="count" aria-label={`${toReview} to review`}>{n(toReview)}</span>
            )}
          </button>
        ))}
        <div className="rail-foot">
          <Icon name="lock" size={16} />
          <div>
            <strong>Offline</strong>
            Nothing you add here leaves this PC.
          </div>
        </div>
      </nav>
      <main>
        {demo && (
          <div className="demo-bar" role="status">
            <span>You're looking at sample data. Nothing here is yours.</span>
            <button onClick={leave}>Use my own data</button>
          </div>
        )}
        {view === "home" && <Home key={version} scan={scan} onReview={openReview} onGo={setView} onOpen={openChannel} />}
        {view === "review" && (
          <Review key={`${version}-${reviewKind}`} scan={scan} demo={demo} initialKind={reviewKind} onScan={setScan} onOpen={openChannel} />
        )}
        {view === "messages" && <Messages key={version} jump={jump} onJumpHandled={() => setJump(null)} />}
        {view === "sources" && (
          <Sources
            demo={demo}
            onScan={setScan}
            onChanged={async () => {
              setVersion((v) => v + 1);
              const has = await api.hasData();
              if (!has) return leave();
              setDemo(await api.isDemo());
            }}
          />
        )}
        {view === "settings" && <Settings scan={scan} demo={demo} onScan={setScan} onLeave={leave} />}
      </main>
    </div>
  );
}

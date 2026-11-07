import { useCallback, useEffect, useState } from "react";
import { api, type ScanSummary } from "./api";
import { n } from "./format";
import Welcome from "./views/Welcome";
import Overview from "./views/Overview";
import Exposure from "./views/Exposure";
import Messages from "./views/Messages";
import Settings from "./views/Settings";

export type View = "overview" | "exposure" | "messages" | "settings";
export interface Jump { channelId: string; messageId?: string }

export default function App() {
  const [ready, setReady] = useState<boolean | null>(null);
  const [demo, setDemo] = useState(false);
  const [view, setView] = useState<View>("overview");
  const [jump, setJump] = useState<Jump | null>(null);
  const [scan, setScan] = useState<ScanSummary | null>(null);
  const [version, setVersion] = useState(0); // bumps after an import so views reload

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
          // Land where the work is.
          setView(summary.open > 0 ? "exposure" : "overview");
        }}
      />
    );
  }

  const nav: [View, string][] = [
    ["exposure", "Review"],
    ["overview", "Overview"],
    ["messages", "Messages"],
    ["settings", "Settings"],
  ];
  const toReview = scan ? scan.open : 0;

  return (
    <div className="shell">
      <nav className="rail" aria-label="Sections">
        <div className="brand">
          <span className="brand-mark" aria-hidden /> Veil
        </div>
        {nav.map(([id, label]) => (
          <button key={id} className="nav" aria-current={view === id ? "page" : undefined} onClick={() => setView(id)}>
            {label}
            {id === "exposure" && toReview > 0 && <span className="badge high" aria-label={`${toReview} to review`}>{n(toReview)}</span>}
          </button>
        ))}
        <div className="rail-foot">
          <strong>Offline</strong>
          Veil has no internet access. Your messages stay on this computer.
        </div>
      </nav>
      <main>
        {demo && (
          <div className="demo-bar" role="status">
            <span>You're looking at sample data. Nothing here is yours.</span>
            <button onClick={leave}>Open my own package</button>
          </div>
        )}
        {view === "overview" && <Overview key={version} scan={scan} onOpen={openChannel} onGo={setView} />}
        {view === "exposure" && <Exposure key={version} scan={scan} demo={demo} onScan={setScan} onOpen={openChannel} />}
        {view === "messages" && <Messages key={version} jump={jump} onJumpHandled={() => setJump(null)} />}
        {view === "settings" && <Settings scan={scan} demo={demo} onScan={setScan} onLeave={leave} />}
      </main>
    </div>
  );
}

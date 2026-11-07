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
  const [view, setView] = useState<View>("overview");
  const [jump, setJump] = useState<Jump | null>(null);
  const [scan, setScan] = useState<ScanSummary | null>(null);
  const [version, setVersion] = useState(0); // bumps after an import so views reload

  const refreshScan = useCallback(() => api.scanSummary().then(setScan), []);

  useEffect(() => {
    api.hasData().then((has) => {
      setReady(has);
      if (has) refreshScan();
    });
  }, [refreshScan]);

  const openChannel = (j: Jump) => {
    setJump(j);
    setView("messages");
  };

  if (ready === null) return null;
  if (!ready) {
    return (
      <Welcome
        onImported={() => {
          setReady(true);
          setVersion((v) => v + 1);
          setView("overview");
          refreshScan();
        }}
      />
    );
  }

  const nav: [View, string][] = [
    ["overview", "Overview"],
    ["exposure", "Exposure report"],
    ["messages", "Messages"],
    ["settings", "Settings"],
  ];
  const scanned = scan?.scanned_at_ms != null;

  return (
    <div className="shell">
      <nav className="rail" aria-label="Sections">
        <div className="brand">
          <span className="brand-mark" aria-hidden /> Veil
        </div>
        {nav.map(([id, label]) => (
          <button key={id} className="nav" aria-current={view === id ? "page" : undefined} onClick={() => setView(id)}>
            {label}
            {id === "exposure" && scanned && scan!.flagged_messages > 0 && (
              <span className="badge high">{n(scan!.flagged_messages)}</span>
            )}
          </button>
        ))}
        <div className="rail-foot">
          <strong>Offline</strong>
          Veil has no internet access. Your messages stay on this computer.
        </div>
      </nav>
      <main>
        {view === "overview" && <Overview key={version} scan={scan} onOpen={openChannel} onGo={setView} />}
        {view === "exposure" && <Exposure key={version} scan={scan} onScanned={setScan} onOpen={openChannel} />}
        {view === "messages" && <Messages key={version} jump={jump} onJumpHandled={() => setJump(null)} />}
        {view === "settings" && (
          <Settings
            onWiped={() => {
              setReady(false);
              setScan(null);
              setVersion((v) => v + 1);
            }}
          />
        )}
      </main>
    </div>
  );
}

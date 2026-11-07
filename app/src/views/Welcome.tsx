import { useEffect, useState } from "react";
import { api, listen, onFileDrop, pickPackage, type ImportProgress } from "../api";

export default function Welcome({ onImported }: { onImported: () => void }) {
  const [progress, setProgress] = useState<ImportProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [over, setOver] = useState(false);
  const busy = progress !== null;

  const run = async (path: string | null) => {
    if (!path || busy) return;
    setError(null);
    setProgress({ phase: "Opening package", done: 0, total: 0 });
    try {
      await api.importPackage(path);
      onImported();
    } catch (e) {
      setError(String(e));
      setProgress(null);
    }
  };

  useEffect(() => {
    const un = listen<ImportProgress>("import-progress", setProgress);
    const drop = onFileDrop((p) => run(p), setOver);
    return () => {
      un.then((f) => f());
      drop.then((f) => f());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const pct = progress && progress.total > 0 ? (progress.done / progress.total) * 100 : 0;

  return (
    <div className="welcome">
      <div className="welcome-inner">
        <div className="lede">
          <h1>
            See what your Discord history says about <span className="redact">y••</span>.
          </h1>
          <p className="muted">
            Veil reads the data package Discord gives you and points out messages where you shared things like your
            phone number, address or a password, so you can decide what to clean up.
          </p>
        </div>

        <div className={`drop${over ? " over" : ""}`}>
          {busy ? (
            <div className="progress" role="status" aria-live="polite">
              <span>
                {progress!.phase}
                {progress!.total > 0 && ` (${progress!.done} of ${progress!.total} conversations)`}
              </span>
              <div className="bar">
                <span style={{ width: `${pct}%` }} />
              </div>
            </div>
          ) : (
            <>
              <h3>Open your data package</h3>
              <p className="muted small">Drop the .zip from Discord here, or choose it. An extracted folder works too.</p>
              <div className="drop-actions">
                <button className="primary" onClick={async () => run(await pickPackage(false))}>
                  Choose .zip file
                </button>
                <button onClick={async () => run(await pickPackage(true))}>Choose folder</button>
              </div>
            </>
          )}
          {error && <div className="error">{error}</div>}
        </div>

        <div>
          <h3>Don't have it yet?</h3>
          <ol className="steps muted">
            <li>In Discord, open User Settings, then Data &amp; Privacy.</li>
            <li>Choose Request all of my data, and make sure Messages is selected.</li>
            <li>Discord emails you a download link, usually within a few days.</li>
          </ol>
        </div>

        <div className="promises">
          <div>
            <h3>Works offline</h3>
            <p>Veil never connects to the internet. There is no account, no analytics and no update check.</p>
          </div>
          <div>
            <h3>No Discord login</h3>
            <p>Veil doesn't ask for your password or token. It only reads the file you give it.</p>
          </div>
          <div>
            <h3>Gone when you say so</h3>
            <p>Everything Veil stores sits in one file on this computer. Settings has a button that deletes it.</p>
          </div>
        </div>
      </div>
    </div>
  );
}

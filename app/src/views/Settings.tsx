import { useEffect, useState } from "react";
import { api } from "../api";
import { dateTime } from "../format";

export default function Settings({ onWiped }: { onWiped: () => void }) {
  const [location, setLocation] = useState("");
  const [importedAt, setImportedAt] = useState<number | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.dataLocation().then(setLocation);
    api.stats().then((s) => setImportedAt(s.imported_at_ms));
  }, []);

  const wipe = async () => {
    setBusy(true);
    await api.wipeData();
    onWiped();
  };

  return (
    <div className="page">
      <h1>Settings</h1>

      <section className="setting">
        <h3>Your data on this computer</h3>
        <p className="muted">
          Imported {dateTime(importedAt)}. Veil keeps a copy of your messages, plus scan results and watch terms, in one
          file:
        </p>
        <code>{location}</code>
        <p className="muted small">
          The original package is never changed. Deleting this data doesn't touch your Discord account.
        </p>
      </section>

      <section className="setting">
        <h3>Import a different package</h3>
        <p className="muted">This replaces everything Veil has imported so far.</p>
        <div>
          <button onClick={wipe} disabled={busy}>Choose a new package</button>
        </div>
      </section>

      <section className="setting">
        <h3>Delete everything</h3>
        <p className="muted">
          Removes all imported messages, scan results and watch terms, and overwrites the space they used on disk.
        </p>
        {confirming ? (
          <div className="toolbar">
            <span>This can't be undone.</span>
            <button className="danger" onClick={wipe} disabled={busy}>Delete everything</button>
            <button onClick={() => setConfirming(false)} disabled={busy}>Keep my data</button>
          </div>
        ) : (
          <div>
            <button className="danger" onClick={() => setConfirming(true)}>Delete everything…</button>
          </div>
        )}
      </section>
    </div>
  );
}

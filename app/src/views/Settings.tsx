import { useEffect, useState } from "react";
import { api, type ScanSummary } from "../api";
import { dateTime, plural } from "../format";

interface Props {
  scan: ScanSummary | null;
  demo: boolean;
  onScan: (s: ScanSummary) => void;
  onLeave: () => void;
}

export default function Settings({ scan, demo, onScan, onLeave }: Props) {
  const [location, setLocation] = useState("");
  const [importedAt, setImportedAt] = useState<number | null>(null);
  const [confirm, setConfirm] = useState<"replace" | "delete" | null>(null);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);

  useEffect(() => {
    api.dataLocation().then(setLocation);
    api.stats().then((s) => setImportedAt(s.imported_at_ms));
  }, []);

  const clearIgnored = async () => {
    setBusy(true);
    const n = await api.clearIgnored();
    onScan(await api.runScan(await api.watchTerms()));
    setNote(`Cleared ${plural(n, "rule", "rules")} and scanned again.`);
    setBusy(false);
  };

  const ignored = scan?.ignored_rules ?? 0;

  return (
    <div className="page">
      <h1>Settings</h1>

      <section className="setting">
        <h3>Your data on this computer</h3>
        <p className="muted">
          {demo ? "Sample data loaded" : "Last import"} {dateTime(importedAt)}. Veil keeps a copy of what it read, your review
          progress and your watch terms in one file:
        </p>
        <code>{location}</code>
        <p className="muted small">
          Your exports and files are never changed by importing them. Adding and removing sources is on the Sources screen.
        </p>
      </section>

      <section className="setting">
        <h3>Things you told Veil never to flag</h3>
        {ignored > 0 ? (
          <>
            <p className="muted">
              {plural(ignored, "item is", "items are")} on the never-flag list. Veil keeps only a fingerprint of each, not the
              text itself. Clearing the list brings them back on the next scan.
            </p>
            <div><button onClick={clearIgnored} disabled={busy}>Clear the list and scan again</button></div>
          </>
        ) : (
          <p className="muted">
            Nothing yet. When something keeps getting flagged but is fine, like a public work email, choose "Never flag this
            again" on it.
          </p>
        )}
        {note && <p className="small muted">{note}</p>}
      </section>

      {demo && (
        <section className="setting">
          <h3>Use your own data</h3>
          <p className="muted">This removes the sample data and takes you back to the start.</p>
          <div><button onClick={onLeave}>Remove sample data</button></div>
        </section>
      )}

      <section className="setting">
        <h3>Delete everything</h3>
        <p className="muted">
          Removes all imported messages, review progress and watch terms, and overwrites the space they used on disk.
        </p>
        {confirm === "delete" ? (
          <div className="toolbar">
            <span>This can't be undone.</span>
            <button className="danger" onClick={onLeave}>Delete everything</button>
            <button onClick={() => setConfirm(null)}>Keep my data</button>
          </div>
        ) : (
          <div><button className="danger" onClick={() => setConfirm("delete")}>Delete everything…</button></div>
        )}
      </section>
    </div>
  );
}

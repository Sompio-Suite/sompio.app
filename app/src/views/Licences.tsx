import { useState } from "react";
import { api, openExternal, type LicenceStatus } from "../api";
import { date, dateTime } from "../format";
import Icon from "../Icon";
import { accessText, BUY_URL, lengthText, SUITE } from "../licence";

interface Props {
  licence: LicenceStatus | null;
  onChanged: (s: LicenceStatus) => void;
}

export default function Licences({ licence, onChanged }: Props) {
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);
  const now = licence?.now ?? Date.now();

  const add = async () => {
    setBusy(true);
    setResult(null);
    try {
      const added = await api.licenceAdd(key);
      onChanged(added.status);
      setKey("");
      setResult({
        ok: true,
        text: added.days >= 65535
          ? `Added ${added.label}, forever${added.test ? " (test key)" : ""}. Thank you!`
          : `Added ${added.label}, ${lengthText(added.days).toLowerCase()}${added.test ? " (test key)" : ""}. Its days start after what you already had.`,
      });
    } catch (e) {
      setResult({ ok: false, text: String(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>Licences</h1>
          <p className="muted">Keys are checked on this computer against Sompio's signature. Nothing is sent anywhere.</p>
        </div>
        <button onClick={() => openExternal(BUY_URL).catch(() => {})}><Icon name="ticket" size={16} />Get a key</button>
      </div>

      {licence?.clock_behind && (
        <div className="notice warn">
          <Icon name="clock" />
          <div>
            <strong>This computer's clock is behind.</strong>
            <span>Sompio has already seen a later date, so it keeps using that one. Set the clock right and this message goes away.</span>
          </div>
        </div>
      )}

      <section className="panel add-key">
        <label htmlFor="key">Add a key</label>
        <textarea
          id="key"
          rows={3}
          spellCheck={false}
          autoComplete="off"
          placeholder="SOMPIO-…"
          value={key}
          onChange={(e) => setKey(e.target.value)}
        />
        <div className="toolbar">
          <button className="primary" disabled={busy || key.trim().length < 20} onClick={add}>{busy ? "Checking…" : "Add key"}</button>
          <span className="small muted">Paste the whole key. Capitals, spaces and dashes don't matter.</span>
        </div>
        {result && <div className={result.ok ? "success" : "error"} role="status">{result.text}</div>}
      </section>

      <section className="section">
        <h2 className="section-title">What's unlocked</h2>
        <div className="panel">
          <table className="plain-table">
            <tbody>
              {SUITE.map((app) => {
                const a = licence?.apps.find((x) => x.app === app.key);
                const access = accessText(a, now, licence?.beta);
                return (
                  <tr key={app.key}>
                    <td><span className="row-name"><Icon name={app.icon} size={16} />{app.name}</span></td>
                    <td><span className={`access access-${access.tone}`}>{access.text}</span></td>
                    <td className="muted small">{app.ready ? "" : app.when}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
        {licence?.trial_ends_at && (
          <p className="small muted">The free trial on this computer {licence.trial_ends_at > now ? "runs until" : "ended"} {date(licence.trial_ends_at)}.</p>
        )}
      </section>

      <section className="section">
        <h2 className="section-title">Keys on this computer</h2>
        {licence && licence.keys.length > 0 ? (
          <div className="panel">
            <table className="plain-table">
              <thead><tr><th>Key</th><th>Length</th><th>Added</th><th>Serial</th></tr></thead>
              <tbody>
                {licence.keys.map((k, i) => (
                  <tr key={k.serial || i}>
                    <td>
                      {k.invalid ? <span className="access access-ended">Not valid</span> : k.label}
                      {k.test && <span className="tag">Test</span>}
                    </td>
                    <td>{k.invalid ? "" : lengthText(k.days)}</td>
                    <td>{dateTime(k.activated_at)}</td>
                    <td className="mono small">{k.serial}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p className="muted">No keys yet. You get one on sompio.org after buying, or from us if you're a tester. A month, a year or forever: each key works on all your own devices.</p>
        )}
      </section>
    </div>
  );
}

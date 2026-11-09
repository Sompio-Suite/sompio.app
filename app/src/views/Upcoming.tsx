import Icon from "../Icon";
import type { SuiteApp } from "../licence";

/** Calendar, Vault and Voice before they're built: what each will do. */
export default function Upcoming({ app }: { app: SuiteApp }) {
  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>Sompio {app.name}</h1>
          <p className="muted">{app.line}</p>
        </div>
      </div>
      <section className="panel upcoming">
        <div className="upcoming-head">
          <span className="suite-icon"><Icon name={app.icon} /></span>
          <div>
            <strong>{app.when}</strong>
            <span className="muted small">Testers in the Discord get it first.</span>
          </div>
        </div>
        <ul className="plan-list">
          {app.plans.map((p) => (
            <li key={p}><Icon name="check" size={16} />{p}</li>
          ))}
        </ul>
      </section>
    </div>
  );
}

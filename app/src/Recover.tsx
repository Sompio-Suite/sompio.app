import { Component, type ReactNode } from "react";
import { api } from "./api";

/** If a screen crashes, show a way back instead of an empty window, and note it in the problems log. */
export default class Recover extends Component<{ children: ReactNode }, { failed: string | null }> {
  state = { failed: null as string | null };
  static getDerivedStateFromError(e: unknown) {
    return { failed: e instanceof Error ? e.message : String(e) };
  }
  componentDidCatch(e: unknown, info: { componentStack?: string | null }) {
    const stack = (info.componentStack ?? "").trim().split("\n").slice(0, 4).map((l) => l.trim()).join(" < ");
    api.logProblem(`${window.location.hash || "#main"} screen crashed: ${e instanceof Error ? e.message : String(e)} [${stack}]`).catch(() => {});
  }
  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <div className="recover">
        <h1>Something went wrong on this screen</h1>
        <p className="muted">Your data is safe: nothing was lost or changed. A note about it was written to the problems log on this computer.</p>
        <p className="small faint mono">{this.state.failed}</p>
        <div className="toolbar">
          <button className="primary" onClick={() => window.location.reload()}>Reload Sompio</button>
          <button onClick={() => api.openProblemsFolder().catch(() => {})}>Open the problems log</button>
        </div>
      </div>
    );
  }
}

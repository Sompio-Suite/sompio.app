import { useEffect, useState } from "react";
import { api, inDesktopApp, listen, type AutoTypeMatch, type VaultEntry } from "../api";
import { Avatar } from "./VaultTools";

const close = async () => {
  if (!inDesktopApp) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  getCurrentWindow().close();
};

/** Shown by the auto-type shortcut when more than one entry fits the window, or none does. */
export default function AutoTypeChooser() {
  const [win, setWin] = useState("");
  const [matches, setMatches] = useState<AutoTypeMatch[]>([]);
  const [search, setSearch] = useState("");
  const [found, setFound] = useState<VaultEntry[]>([]);
  const [error, setError] = useState<string | null>(null);

  const load = () => api.vaultAutotypeMatches().then((r) => { setWin(r.window); setMatches(r.matches); }).catch((e) => setError(String(e)));
  useEffect(() => {
    document.body.classList.add("popup-body");
    load();
    const un = listen("vault-autotype", () => load());
    const key = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("keydown", key);
    return () => { un.then((f) => f()); window.removeEventListener("keydown", key); };
  }, []);
  useEffect(() => {
    if (!search.trim()) return setFound([]);
    api.vaultEntries(null, search).then((r) => setFound(r.filter((e) => e.has_password).slice(0, 8))).catch(() => {});
  }, [search]);

  const pick = (id: string, sequence: string | null) => api.vaultAutotypeChoose(id, sequence).catch((e) => setError(String(e)));
  const list: { entry: VaultEntry; sequence: string | null; note: string }[] = search.trim()
    ? found.map((e) => ({ entry: e, sequence: null, note: e.group_path }))
    : matches.map((m) => ({ entry: m.entry, sequence: m.sequence, note: m.because === "window rule" ? "window rule" : m.because === "title" ? "title matches" : "website matches" }));

  return (
    <div className="chooser">
      <header>
        <strong>Auto-type into</strong>
        <span className="small muted chooser-window">{win || "the window in front"}</span>
      </header>
      <input type="search" autoFocus placeholder={matches.length ? "Or search all entries" : "No entry fits this window. Search for one"} value={search} onChange={(e) => setSearch(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter" && list[0]) pick(list[0].entry.id, list[0].sequence); }} />
      <ul>
        {list.map((m, i) => (
          <li key={m.entry.id}>
            <button className="entry-row" onClick={() => pick(m.entry.id, m.sequence)} autoFocus={i === 0 && !search}>
              <Avatar title={m.entry.title} />
              <span className="entry-name"><strong>{m.entry.title}</strong><span className="small muted">{m.entry.username}</span></span>
              <span className="faint small">{m.note}</span>
            </button>
          </li>
        ))}
      </ul>
      {error && <div className="error">{error}</div>}
      <footer className="small faint">Enter types the first one · Esc closes</footer>
    </div>
  );
}

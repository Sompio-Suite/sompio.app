// Typed bridge to the Rust side. Shapes mirror veil-core's structs.
// Outside the desktop app (plain `npm run dev` in a browser) a mock with fake
// data is used, so the UI can be worked on without building Rust.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";

export type KindKey =
  | "credential" | "card" | "iban" | "watch_term" | "address"
  | "phone" | "email" | "ip_address" | "postcode" | "crypto_wallet";

export interface ChannelView {
  id: string;
  name: string;
  kind: "dm" | "group_dm" | "server" | "thread" | "unknown";
  guild_name: string | null;
  message_count: number;
  first_ts: number | null;
  last_ts: number | null;
}
export interface MessageView {
  id: string;
  channel_id: string;
  channel_name: string;
  ts: number;
  content: string;
  attachments: string[];
}
export interface Page<T> { total: number; offset: number; items: T[] }
export interface Stats {
  messages: number; channels: number; servers: number; dms: number; group_dms: number;
  with_attachments: number; first_ts: number | null; last_ts: number | null;
  imported_at_ms: number | null;
  per_year: { year: number; count: number }[];
  top_channels: ChannelView[];
}
export interface ImportSummary { channels: number; messages: number; servers: number; warnings: string[] }
export interface ImportProgress { phase: string; done: number; total: number }
export interface KindCount { kind: KindKey; label: string; severity: number; count: number; messages: number }
export interface ScanSummary {
  scanned: number; flagged_messages: number; total_findings: number;
  scanned_at_ms: number | null; by_kind: KindCount[];
}
export interface FindingView {
  id: number; message_id: string; channel_id: string; channel_name: string; ts: number;
  kind: KindKey; label: string; severity: number;
  before: string; matched: string; after: string; masked: boolean;
}

export const inDesktopApp = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
type Listen = <T>(event: string, cb: (payload: T) => void) => Promise<UnlistenFn>;

let backend: Promise<{ invoke: Invoke; listen: Listen }> = inDesktopApp
  ? Promise.resolve({
      invoke: tauriInvoke as Invoke,
      listen: (<T,>(e: string, cb: (p: T) => void) => tauriListen<T>(e, (ev) => cb(ev.payload))) as Listen,
    })
  : import("./mock").then((m) => m.mockBackend);

const call = async <T,>(cmd: string, args?: Record<string, unknown>) => (await backend).invoke<T>(cmd, args);
export const listen = async <T,>(event: string, cb: (payload: T) => void) => (await backend).listen<T>(event, cb);

export const api = {
  hasData: () => call<boolean>("has_data"),
  importPackage: (path: string) => call<ImportSummary>("import_package", { path }),
  stats: () => call<Stats>("stats"),
  channels: (filter?: string) => call<ChannelView[]>("channels", { filter: filter || null }),
  messages: (channelId: string, offset: number, limit: number) =>
    call<Page<MessageView>>("messages", { channelId, offset, limit }),
  messageIndex: (channelId: string, messageId: string) => call<number>("message_index", { channelId, messageId }),
  search: (text: string, channelId: string | null, offset: number, limit: number) =>
    call<Page<MessageView>>("search", { text, channelId, offset, limit }),
  runScan: (watchTerms: string[]) => call<ScanSummary>("run_scan", { watchTerms }),
  scanSummary: () => call<ScanSummary>("scan_summary"),
  findings: (kind: KindKey | null, offset: number, limit: number, reveal: boolean) =>
    call<Page<FindingView>>("findings", { kind, offset, limit, reveal }),
  watchTerms: () => call<string[]>("watch_terms"),
  exportFlagged: (path: string) => call<number>("export_flagged", { path }),
  wipeData: () => call<void>("wipe_data"),
  dataLocation: () => call<string>("data_location"),
};

export async function pickPackage(folder: boolean): Promise<string | null> {
  if (!inDesktopApp) return "demo-package.zip";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open(
    folder
      ? { directory: true, title: "Choose your extracted data package folder" }
      : { title: "Choose your Discord data package", filters: [{ name: "Discord data package", extensions: ["zip"] }] },
  );
  return typeof picked === "string" ? picked : null;
}

export async function pickSavePath(defaultName: string): Promise<string | null> {
  if (!inDesktopApp) return defaultName;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return save({ defaultPath: defaultName, filters: [{ name: "CSV", extensions: ["csv"] }] });
}

/** Files dropped onto the window (desktop app only). */
export async function onFileDrop(cb: (path: string) => void, hover: (over: boolean) => void): Promise<UnlistenFn> {
  if (!inDesktopApp) return () => {};
  const { getCurrentWebview } = await import("@tauri-apps/api/webview");
  return getCurrentWebview().onDragDropEvent((e) => {
    const p = e.payload;
    if (p.type === "enter" || p.type === "over") hover(true);
    else if (p.type === "leave") hover(false);
    else if (p.type === "drop") {
      hover(false);
      if (p.paths[0]) cb(p.paths[0]);
    }
  });
}

// For tests or hot reload
export function _setBackend(b: { invoke: Invoke; listen: Listen }) { backend = Promise.resolve(b); }

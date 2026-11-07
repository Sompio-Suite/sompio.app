// Typed bridge to the Rust side. Shapes mirror veil-core's structs.
// Outside the desktop app (plain `npm run dev` in a browser) a mock with fake
// data is used, so the UI can be worked on without building Rust.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";

export type KindKey =
  | "credential" | "card" | "iban" | "watch_term" | "address"
  | "phone" | "email" | "ip_address" | "postcode" | "crypto_wallet" | "sensitive_file";
export type Status = "deleted" | "ok" | "later";
export type Platform =
  | "discord" | "reddit" | "telegram" | "x" | "whatsapp" | "facebook" | "instagram"
  | "tiktok" | "snapchat" | "linkedin" | "email" | "pc";
export type StatusTab = "open" | "later" | "done";

export interface ChannelView {
  id: string;
  platform: Platform;
  name: string;
  kind: "dm" | "group_dm" | "server" | "thread" | "community" | "posts" | "channel" | "saved" | "folder" | "unknown";
  guild_name: string | null;
  message_count: number;
  first_ts: number | null;
  last_ts: number | null;
}
export interface Segment { text: string; sensitive: boolean }
export interface MessageView {
  id: string;
  channel_id: string;
  channel_name: string;
  platform: Platform;
  ts: number;
  content: string;
  segments: Segment[];
  has_sensitive: boolean;
  attachments: string[];
  link: string | null;
  file_path: string | null;
}
export interface Page<T> { total: number; offset: number; items: T[] }
export interface SourceView { platform: Platform; label: string; origin: string; imported_at: number; channels: number; messages: number }
export interface Stats {
  messages: number; channels: number; servers: number; dms: number; group_dms: number; files: number;
  with_attachments: number; first_ts: number | null; last_ts: number | null;
  imported_at_ms: number | null;
  per_year: { year: number; count: number }[];
  top_channels: ChannelView[];
  sources: SourceView[];
}
export interface ImportSummary { platform: Platform; channels: number; messages: number; servers: number; warnings: string[] }
export interface ImportProgress { phase: string; done: number; total: number }
export interface KindCount { kind: KindKey; label: string; severity: number; advice: string; count: number; open: number }
export interface ScanSummary {
  scanned: number; flagged_messages: number; total_findings: number;
  open: number; later: number; done: number; ignored_rules: number;
  scanned_at_ms: number | null; by_kind: KindCount[];
}
export interface FindingView {
  id: number; message_id: string; channel_id: string; channel_name: string; platform: Platform; ts: number;
  kind: KindKey; label: string; severity: number; advice: string;
  before: string; matched: string; after: string; masked: boolean;
  status: Status | null; link: string | null; file_path: string | null;
  /** How many findings in this list have the same value */
  repeats: number;
}

export interface GroupInfo {
  ids: number[];
  message_ids: string[];
  member_platforms: Platform[];
  platforms: { platform: Platform; count: number }[];
  first_ts: number;
  last_ts: number;
}
export type Previous = [number, Status | null][];

export interface TgStatus { has_app_key: boolean; connected: boolean; name: string | null; waiting_for: "code" | "password" | null }
export type TgStep = { step: "done"; name: string } | { step: "need_password"; hint: string | null };
export interface TgProgress { chats_done: number; chats_total: number; messages: number }

export interface EmailProvider { id: string; name: string; host: string; help_url: string }
export interface EmailStatus { connected: boolean; address: string | null; provider: string | null }
export interface EmailProgress { done: number; total: number }

export const inDesktopApp = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
type Listen = <T>(event: string, cb: (payload: T) => void) => Promise<UnlistenFn>;

const backend: Promise<{ invoke: Invoke; listen: Listen }> = inDesktopApp
  ? Promise.resolve({
      invoke: tauriInvoke as Invoke,
      listen: (<T,>(e: string, cb: (p: T) => void) => tauriListen<T>(e, (ev) => cb(ev.payload))) as Listen,
    })
  : import("./mock").then((m) => m.mockBackend);

const call = async <T,>(cmd: string, args?: Record<string, unknown>) => (await backend).invoke<T>(cmd, args);
export const listen = async <T,>(event: string, cb: (payload: T) => void) => (await backend).listen<T>(event, cb);

export const api = {
  hasData: () => call<boolean>("has_data"),
  tgStatus: () => call<TgStatus>("tg_status"),
  tgSetAppKey: (apiId: string, apiHash: string) => call<void>("tg_set_app_key", { apiId, apiHash }),
  tgStartLogin: (phone: string) => call<void>("tg_start_login", { phone }),
  tgSubmitCode: (code: string) => call<TgStep>("tg_submit_code", { code }),
  tgSubmitPassword: (password: string) => call<TgStep>("tg_submit_password", { password }),
  tgDisconnect: () => call<void>("tg_disconnect"),
  tgSync: () => call<ImportSummary>("tg_sync"),
  tgDelete: (id: number) => call<ScanSummary>("tg_delete", { id }),
  emailProviders: () => call<EmailProvider[]>("email_providers"),
  emailStatus: () => call<EmailStatus>("email_status"),
  emailConnect: (provider: string, host: string, address: string, password: string) =>
    call<EmailStatus>("email_connect", { provider, host, address, password }),
  emailDisconnect: () => call<void>("email_disconnect"),
  emailSync: () => call<ImportSummary>("email_sync"),
  emailDelete: (id: number) => call<ScanSummary>("email_delete", { id }),
  myNames: () => call<string[]>("my_names"),
  setMyNames: (names: string[]) => call<void>("set_my_names", { names }),
  isDemo: () => call<boolean>("is_demo"),
  loadDemo: () => call<void>("load_demo"),
  importPackage: (path: string) => call<ImportSummary>("import_package", { path }),
  scanFolders: (folders: string[]) => call<ImportSummary>("scan_folders", { folders }),
  sources: () => call<SourceView[]>("sources"),
  removeSource: (platform: Platform) => call<void>("remove_source", { platform }),
  recycleFile: (id: number) => call<[string, ScanSummary]>("recycle_file", { id }),
  showInFolder: (id: number) => call<void>("show_in_folder", { id }),
  stats: () => call<Stats>("stats"),
  channels: (filter?: string) => call<ChannelView[]>("channels", { filter: filter || null }),
  messages: (channelId: string, offset: number, limit: number) =>
    call<Page<MessageView>>("messages", { channelId, offset, limit }),
  messageIndex: (channelId: string, messageId: string) => call<number>("message_index", { channelId, messageId }),
  revealMessage: (messageId: string) => call<MessageView | null>("reveal_message", { messageId }),
  search: (text: string, channelId: string | null, offset: number, limit: number) =>
    call<Page<MessageView>>("search", { text, channelId, offset, limit }),
  runScan: (watchTerms: string[]) => call<ScanSummary>("run_scan", { watchTerms }),
  scanSummary: () => call<ScanSummary>("scan_summary"),
  findings: (
    kind: KindKey | null, platform: Platform | null, status: StatusTab, offset: number, limit: number, reveal: boolean,
    group = false, sameAs: number | null = null,
  ) => call<Page<FindingView>>("findings", { kind, platform, status, offset, limit, reveal, group, sameAs }),
  groupInfo: (id: number, status: StatusTab) => call<GroupInfo>("group_info", { id, status }),
  setStatusMany: (ids: number[], status: Status | null) => call<[Previous, ScanSummary]>("set_status_many", { ids, status }),
  restoreStatuses: (previous: Previous) => call<ScanSummary>("restore_statuses", { previous }),
  revealFinding: (id: number) => call<FindingView | null>("reveal_finding", { id }),
  setStatus: (id: number, status: Status | null) => call<ScanSummary>("set_status", { id, status }),
  ignoreEverywhere: (id: number) => call<[number, ScanSummary]>("ignore_everywhere", { id }),
  clearIgnored: () => call<number>("clear_ignored"),
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
      : {
          title: "Choose your data export",
          filters: [
            { name: "Data exports", extensions: ["zip", "json", "txt", "mbox"] },
            { name: "All files", extensions: ["*"] },
          ],
        },
  );
  return typeof picked === "string" ? picked : null;
}

export async function pickFolders(): Promise<string[]> {
  if (!inDesktopApp) return ["C:\\Users\\you\\Documents"];
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: true, multiple: true, title: "Choose folders to scan" });
  return Array.isArray(picked) ? picked : typeof picked === "string" ? [picked] : [];
}

export async function pickSavePath(defaultName: string): Promise<string | null> {
  if (!inDesktopApp) return defaultName;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return save({ defaultPath: defaultName, filters: [{ name: "CSV", extensions: ["csv"] }] });
}

const ALLOWED_LINKS = [
  "https://discord.com/channels/", "https://www.reddit.com/", "https://x.com/", "https://t.me/c/",
  "https://www.linkedin.com/", "https://mail.google.com/mail/",
  // Where email providers hand out app passwords
  "https://myaccount.google.com/apppasswords", "https://login.yahoo.com/account/security", "https://account.apple.com",
  "https://www.gmx.net", "https://app.fastmail.com/settings/security/apps",
];

/** Opens a message on its platform, in the browser where the person is signed in. Only known sites are allowed. */
export async function openLink(url: string | null) {
  if (!url || !ALLOWED_LINKS.some((p) => url.startsWith(p))) return;
  if (!inDesktopApp) {
    window.open(url, "_blank", "noopener");
    return;
  }
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(url);
}

export async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const t = document.createElement("textarea");
    t.value = text;
    document.body.appendChild(t);
    t.select();
    document.execCommand("copy");
    t.remove();
  }
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

export const platformName: Record<Platform, string> = {
  discord: "Discord",
  reddit: "Reddit",
  telegram: "Telegram",
  x: "X",
  whatsapp: "WhatsApp",
  facebook: "Facebook",
  instagram: "Instagram",
  tiktok: "TikTok",
  snapchat: "Snapchat",
  linkedin: "LinkedIn",
  email: "Email",
  pc: "This PC",
};

/** "Open in Discord", "Open on Reddit", "Open in Gmail", … */
export const openLabel = (p: Platform) =>
  p === "email" ? "Open in Gmail" : p === "discord" || p === "telegram" ? `Open in ${platformName[p]}` : `Open on ${platformName[p]}`;

export const severityLabel = (s: number) => (s >= 3 ? "Fix now" : s === 2 ? "Worth a look" : "Low risk");

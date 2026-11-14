// Typed bridge to the Rust side. Shapes mirror sompio-clean's structs.
// Outside the desktop app (plain `npm run dev` in a browser) a mock with fake
// data is used, so the UI can be worked on without building Rust.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type UnlistenFn } from "@tauri-apps/api/event";

export type KindKey =
  | "credential" | "card" | "iban" | "watch_term" | "address"
  | "phone" | "email" | "ip_address" | "postcode" | "crypto_wallet" | "sensitive_file"
  | "id_number" | "passport" | "backup_code" | "birth_date" | "number_plate" | "location";
export type Status = "deleted" | "ok" | "later";
export type Platform =
  | "discord" | "reddit" | "telegram" | "x" | "whatsapp" | "facebook" | "instagram"
  | "tiktok" | "snapchat" | "linkedin" | "email" | "pc";
export type StatusTab = "open" | "later" | "done";

/** The extra checks for a folder scan, both off unless the person turns them on. */
export interface PcScanOptions { image_text: boolean; photo_places: boolean }
/** Data exports the person asked for: platform name -> the day they asked (YYYY-MM-DD). */
export type ExportRequests = Record<string, string>;

/** Trace's request centre, stored encrypted with everything else */
export interface PrivacyRequest {
  id: string; company: string; company_name: string; kind: "erasure" | "access" | "objection"; account: string;
  sent_on: string | null; status: "waiting" | "answered" | "done"; reminders: string[];
  complained_on: string | null; authority: string; note: string;
}
export interface TraceData {
  profile: { name: string; email: string; country: string; handles: Record<string, string> };
  exports: Record<string, { requested_on: string | null }>;
  requests: PrivacyRequest[];
}

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

export type AppKey = "clean" | "calendar" | "vault";
export interface AppStatus { app: AppKey; name: string; active: boolean; ends_at: number | null; trial_only: boolean }
export interface KeyView { serial: string; label: string; days: number; activated_at: number; test: boolean; invalid: boolean }
export interface LicenceStatus { apps: AppStatus[]; keys: KeyView[]; trial_ends_at: number | null; clock_behind: boolean; now: number; beta: boolean }
export interface Added { label: string; days: number; test: boolean; status: LicenceStatus }

export interface HardwareKey { index: number; serial: number | null; label: string; slot: number; added_ms: number }
export interface LockStatus { set_up: boolean; unlocked: boolean; remembers_picture: boolean; hardware_keys: HardwareKey[] }


// Calendar. Times are local, as "YYYY-MM-DDTHH:MM:SS" without a zone.
export interface Occurrence {
  at: string; event_at: string; kind: "event" | "notice";
  source_id: number; source_name: string; mine: boolean; colour: string;
  title: string; text: string; notice: number; for_groups: string[];
  for_me: boolean; notify: boolean; quiet_allow: boolean;
  key: string; line: number; days_text: string; time_text: string; one_off: boolean;
  zone_date: string; zone_time: string;
}
export interface ScheduleView {
  id: number; name: string; mine: boolean; owned: boolean; editable: boolean; colour: string; enabled: boolean;
  sound: string | null; groups: string[]; my_groups: string[]; quiet_allow: boolean;
  timezone: string; updated: string; source: string | null; reminders: number; problems: { line: number; message: string }[];
}
export interface CalSettings {
  quiet: { on: boolean; from: number; to: number };
  sound: string; sound_on: boolean; volume: number; corner: string; popup_seconds: number; discreet: boolean; hide_from_capture: boolean;
}
export interface RowInput { days: string; time: string; skip?: boolean; title: string; text?: string; notice?: number; for_groups?: string[] }
export interface CalOverview { schedules: ScheduleView[]; occurrences: Occurrence[]; settings: CalSettings }
export interface Popup { id: number; occurrence: Occurrence; snoozed: boolean }
export interface PopupState {
  shown: { popups: Popup[]; paused_until: string | null };
  sound: string; sound_on: boolean; volume: number; seconds: number; discreet: boolean;
  /** Schedules with their own sound, by schedule id */
  sounds: Record<string, string>;
}
export type AlertModeKey = "mute" | "shout" | "hide";
export const SOUNDS = ["Soft chime", "Warm bell", "Marimba", "Kalimba", "Glass", "Music box", "Rising", "Soft ping", "Water drop"];

// Vault
export interface VaultGroup { id: string; parent: string | null; name: string; depth: number; entries: number; recycle_bin: boolean }
export interface VaultEntry {
  id: string; group_id: string; group_path: string; title: string; username: string; url: string; tags: string[];
  has_password: boolean; has_totp: boolean; expires: string | null; expired: boolean; modified: string | null; in_recycle_bin: boolean;
  /** Id of the entry's own icon (see vaultIcons) */
  icon: string | null;
}
export interface VaultField { key: string; value: string | null; protected: boolean }
export interface AutoTypeSettings { enabled: boolean; sequence: string; windows: string[] }
export interface VaultHistory { index: number; modified: string | null; title: string; username: string; password_changed: boolean }
export interface VaultEntryDetail extends VaultEntry {
  notes: string; custom: VaultField[]; autotype: AutoTypeSettings; created: string | null; password_modified: string | null; history: VaultHistory[];
  attachments: { name: string; size: number }[];
}
export interface VaultMerge { added: number; updated: number; moved: number; deleted: number; warnings: string[] }
export interface VaultLeak { entry: VaultEntry; seen: number }
/** A saved password that also shows up in your own chats or files (what Trace imported). */
export interface VaultInTrace { entry: VaultEntry; count: number; platforms: Platform[]; first_ts: number; last_ts: number }
export interface VaultEntryInput {
  group_id: string | null; title: string; username: string; password: string | null; url: string; notes: string; tags: string[];
  expires: string | null; custom: { key: string; value: string | null; protected: boolean }[]; otp: string | null; autotype: AutoTypeSettings | null;
}
export interface Totp { code: string; remaining: number; period: number }
export interface Strength { score: number; label: string; bits: number; crack_time: string; warning: string | null; suggestions: string[] }
export interface PasswordRules { length: number; lower: boolean; upper: boolean; digits: boolean; symbols: boolean; no_lookalikes: boolean; every_kind: boolean; exclude: string }
export interface PassphraseRules { words: number; separator: string; capitalize: boolean; number: boolean }
export type HealthIssue =
  | { kind: "weak"; score: number; label: string; hint: string | null }
  | { kind: "reused"; count: number; others: string[] }
  | { kind: "expired" }
  | { kind: "old"; days: number }
  | { kind: "no_password" };
export interface HealthReport { checked: number; weak: number; reused: number; expired: number; old: number; score: number; items: { entry: VaultEntry; issues: HealthIssue[]; severity: number }[] }
export interface VaultImport { entries: number; groups: number; skipped_attachments: number; group_id: string; warnings: string[] }
export interface VaultSettings { shortcut: string; clear_seconds: number; lock_after_type: boolean; auto_icons: boolean; browser: boolean }
export interface BrowserClient { id: string; browser: string; added: number; last_used: number }
export interface BrowserPairAsk { id: string; browser: string; code: string }
export interface AutoTypeMatch { entry: VaultEntry; sequence: string; because: string }


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
  lockStatus: () => call<LockStatus>("lock_status"),
  lockNewPicture: (picturePath: string) => call<void>("lock_new_picture", { picturePath }),
  lockSetupWithPicture: (password: string, remember: boolean) => call<string>("lock_setup_with_picture", { password, remember }),
  lockSetup: (password: string, picturePath: string, remember: boolean) => call<string>("lock_setup", { password, picturePath, remember }),
  lockUnlock: (password: string, picturePath: string | null, remember: boolean) => call<void>("lock_unlock", { password, picturePath, remember }),
  lockRecover: (code: string, newPassword: string, picturePath: string, remember: boolean) =>
    call<string>("lock_recover", { code, newPassword, picturePath, remember }),
  lockChangePassword: (old: string, nu: string, picturePath: string | null) => call<void>("lock_change_password", { old, new: nu, picturePath }),
  lockYubikeysPresent: () => call<{ name: string; serial: number | null }[]>("lock_yubikeys_present"),
  lockUnlockYubikey: (password: string) => call<void>("lock_unlock_yubikey", { password }),
  lockAddYubikey: (picturePath: string | null, slot: number, label: string) => call<void>("lock_add_yubikey", { picturePath, slot, label }),
  lockRemoveYubikey: (index: number) => call<void>("lock_remove_yubikey", { index }),
  lockChangePasswordYubikey: (old: string, nu: string) => call<void>("lock_change_password_yubikey", { old, new: nu }),
  lockForgetPicture: () => call<void>("lock_forget_picture"),
  lockLock: () => call<void>("lock_lock"),
  licenceStatus: () => call<LicenceStatus>("licence_status"),
  licenceAdd: (key: string) => call<Added>("licence_add", { key }),
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
  scanFolders: (folders: string[], opts: PcScanOptions = { image_text: false, photo_places: false }) =>
    call<ImportSummary>("scan_folders", { folders, opts }),
  exportRequests: async () => JSON.parse(await call<string>("export_requests")) as ExportRequests,
  setExportRequests: (r: ExportRequests) => call<void>("set_export_requests", { json: JSON.stringify(r) }),
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
  traceData: () => call<TraceData>("trace_data"),
  traceSave: (data: TraceData) => call<void>("trace_save", { data }),
  traceRemind: (date: string, title: string, text: string) => call<void>("trace_remind", { date, title, text }),
  exportFlagged: (path: string) => call<number>("export_flagged", { path }),
  wipeData: () => call<void>("wipe_data"),
  dataLocation: () => call<string>("data_location"),
  calOverview: (from: string, to: string) => call<CalOverview>("cal_overview", { from, to }),
  calNext: (count: number) => call<Occurrence[]>("cal_next", { count }),
  calAddRows: (scheduleId: number, rows: RowInput[]) => call<void>("cal_add_rows", { scheduleId, rows }),
  calUpdateRow: (scheduleId: number, line: number, row: RowInput) => call<void>("cal_update_row", { scheduleId, line, row }),
  calRemoveRow: (scheduleId: number, line: number) => call<void>("cal_remove_row", { scheduleId, line }),
  calSkipOnce: (scheduleId: number, day: string, time: string) => call<void>("cal_skip_once", { scheduleId, day, time }),
  calImportFile: (path: string) => call<number>("cal_import_file", { path }),
  calImportIcs: (path: string) => call<{ added: number; past: number; unsupported: number }>("cal_import_ics", { path }),
  calImportDeskReminders: (folder: string) => call<number>("cal_import_desk_reminders", { folder }),
  calExport: (scheduleId: number, path: string) => call<void>("cal_export", { scheduleId, path }),
  calCreateShared: (name: string, timezone: string, groups: string[]) => call<number>("cal_create_shared", { name, timezone, groups }),
  calSetOptions: (scheduleId: number, o: { colour?: string; enabled?: boolean; sound?: string; myGroups?: string[]; quietAllow?: boolean }) =>
    call<void>("cal_set_options", { scheduleId, colour: o.colour ?? null, enabled: o.enabled ?? null, sound: o.sound ?? null, myGroups: o.myGroups ?? null, quietAllow: o.quietAllow ?? null }),
  calRename: (scheduleId: number, name: string) => call<void>("cal_rename", { scheduleId, name }),
  calRemoveSchedule: (scheduleId: number) => call<void>("cal_remove_schedule", { scheduleId }),
  calSetAlert: (key: string, day: string | null, mode: AlertModeKey | null) => call<void>("cal_set_alert", { key, day, mode }),
  calSetSettings: (settings: CalSettings) => call<void>("cal_set_settings", { settings }),
  calPopups: () => call<PopupState>("cal_popups"),
  calPopupAction: (id: number, action: "done" | "snooze" | "close-all", minutes?: number) => call<void>("cal_popup_action", { id, action, minutes: minutes ?? null }),
  calPause: (minutes: number | null) => call<string | null>("cal_pause", { minutes }),
  calPaused: () => call<string | null>("cal_paused"),
  calAutostart: () => call<boolean>("cal_autostart"),
  calSetAutostart: (on: boolean) => call<void>("cal_set_autostart", { on }),
  vaultGroups: () => call<VaultGroup[]>("vault_groups"),
  vaultEntries: (group: string | null, search: string) => call<VaultEntry[]>("vault_entries", { group, search }),
  vaultEntry: (id: string) => call<VaultEntryDetail>("vault_entry", { id }),
  vaultReveal: (id: string, field: string, history: number | null = null) => call<string>("vault_reveal", { id, field, history }),
  vaultTotp: (id: string) => call<Totp | null>("vault_totp", { id }),
  vaultCopy: (id: string, field: string) => call<number>("vault_copy", { id, field }),
  vaultCopyText: (text: string) => call<number>("vault_copy_text", { text }),
  vaultAddEntry: (entry: VaultEntryInput) => call<string>("vault_add_entry", { entry }),
  vaultUpdateEntry: (id: string, entry: VaultEntryInput) => call<void>("vault_update_entry", { id, entry }),
  vaultDeleteEntry: (id: string) => call<boolean>("vault_delete_entry", { id }),
  vaultRestoreEntry: (id: string) => call<void>("vault_restore_entry", { id }),
  vaultRestoreVersion: (id: string, index: number) => call<void>("vault_restore_version", { id, index }),
  vaultIcons: () => call<Record<string, string>>("vault_icons"),
  vaultFetchIcon: (id: string, address: string | null) => call<string | null>("vault_fetch_icon", { id, address }),
  vaultIconFromFile: (id: string, path: string) => call<string | null>("vault_icon_from_file", { id, path }),
  vaultRemoveIcon: (id: string) => call<void>("vault_remove_icon", { id }),
  vaultFetchAllIcons: () => call<[number, number]>("vault_fetch_all_icons"),
  vaultCloneEntry: (id: string) => call<string>("vault_clone_entry", { id }),
  vaultMoveEntry: (id: string, group: string) => call<void>("vault_move_entry", { id, group }),
  vaultEmptyBin: () => call<number>("vault_empty_bin"),
  vaultAddGroup: (parent: string | null, name: string) => call<string>("vault_add_group", { parent, name }),
  vaultRenameGroup: (id: string, name: string) => call<void>("vault_rename_group", { id, name }),
  vaultMoveGroup: (id: string, parent: string) => call<void>("vault_move_group", { id, parent }),
  vaultDeleteGroup: (id: string) => call<void>("vault_delete_group", { id }),
  vaultGenerate: (kind: "password" | "passphrase", password: PasswordRules | null, passphrase: PassphraseRules | null) =>
    call<{ value: string; strength: Strength }>("vault_generate", { kind, password, passphrase }),
  vaultStrength: (password: string) => call<Strength>("vault_strength", { password }),
  vaultHealth: () => call<HealthReport>("vault_health"),
  vaultImportKdbx: (path: string, password: string, keyfile: string | null) => call<VaultImport>("vault_import_kdbx", { path, password, keyfile }),
  vaultImportCsv: (path: string) => call<VaultImport>("vault_import_csv", { path }),
  vaultExportKdbx: (path: string, password: string) => call<void>("vault_export_kdbx", { path, password }),
  vaultExportCsv: (path: string) => call<void>("vault_export_csv", { path }),
  vaultSettings: () => call<VaultSettings>("vault_settings"),
  browserClients: () => call<BrowserClient[]>("browser_clients"),
  browserForget: (id: string) => call<void>("browser_forget", { id }),
  browserAnswer: (id: string, allow: boolean) => call<void>("browser_answer", { id, allow }),
  browserExtensionFolder: () => call<string>("browser_extension_folder"),
  browserOpenExtensionFolder: () => call<void>("browser_open_extension_folder"),
  browserInstalled: () => call<string[]>("browser_installed"),
  browserOpenExtensionsPage: (browser: string) => call<void>("browser_open_extensions_page", { browser }),
  vaultSetSettings: (settings: VaultSettings) => call<void>("vault_set_settings", { settings }),
  vaultAutotypeMatches: () => call<{ window: string; matches: AutoTypeMatch[] }>("vault_autotype_matches"),
  vaultAutotypeChoose: (id: string, sequence: string | null) => call<void>("vault_autotype_choose", { id, sequence }),
  vaultAutotypeNow: (id: string) => call<void>("vault_autotype_now", { id }),
  vaultAddSamples: () => call<string>("vault_add_samples"),
  logProblem: (text: string) => call<void>("log_problem", { text }),
  appDetails: () => call<{ version: string; os: string; log: string; recent: string[] }>("app_details"),
  openProblemsFolder: () => call<void>("open_problems_folder"),
  vaultAddAttachment: (id: string, path: string) => call<string>("vault_add_attachment", { id, path }),
  vaultSaveAttachment: (id: string, name: string, path: string) => call<void>("vault_save_attachment", { id, name, path }),
  vaultRemoveAttachment: (id: string, name: string) => call<void>("vault_remove_attachment", { id, name }),
  vaultMergeKdbx: (path: string, password: string, keyfile: string | null) => call<VaultMerge>("vault_merge_kdbx", { path, password, keyfile }),
  vaultCheckLeaks: () => call<VaultLeak[]>("vault_check_leaks"),
  vaultInTrace: () => call<VaultInTrace[]>("vault_in_trace"),
  calAddSamples: () => call<number>("cal_add_samples"),
  samplesWrite: (which: "clean" | "calendar" | "vault") => call<{ folder: string; files: [string, string][] }>("samples_write", { which }),
};

export async function pickScheduleFile(): Promise<string | null> {
  if (!inDesktopApp) return "C:\\Users\\you\\Downloads\\Guild Schedule.csv";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ title: "Choose a schedule file", filters: [{ name: "Schedule", extensions: ["csv"] }] });
  return typeof picked === "string" ? picked : null;
}

/** A calendar file exported from a phone or another calendar app. */
export async function pickIcsFile(): Promise<string | null> {
  if (!inDesktopApp) return "C:\\Users\\you\\Downloads\\calendar.ics";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ title: "Choose a calendar file", filters: [{ name: "Calendar", extensions: ["ics", "ical", "ifb", "icalendar"] }] });
  return typeof picked === "string" ? picked : null;
}

export async function pickFolder(title: string): Promise<string | null> {
  if (!inDesktopApp) return "C:\\Users\\you\\Desktop\\DeskReminders";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: true, title });
  return typeof picked === "string" ? picked : null;
}

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

/** Where to save a new key picture. */
export async function pickPictureSave(): Promise<string | null> {
  if (!inDesktopApp) return "C:\\Users\\you\\Pictures\\Sompio key.png";
  const { save } = await import("@tauri-apps/plugin-dialog");
  return save({ title: "Save your key picture", defaultPath: "Sompio key.png", filters: [{ name: "Picture", extensions: ["png"] }] });
}

/** The key picture, to unlock. */
export async function pickPicture(): Promise<string | null> {
  if (!inDesktopApp) return "C:\\Users\\you\\Pictures\\Sompio key.png";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ title: "Choose your key picture", filters: [{ name: "Picture", extensions: ["png"] }] });
  return typeof picked === "string" ? picked : null;
}

/** Mirrors the rules the lock itself applies, so problems show while typing. */
export function passwordProblem(p: string): string | null {
  if ([...p].length < 10) return "Use at least 10 characters. A few unrelated words work well.";
  if (new Set(p).size < 5) return "That password repeats too much. Mix it up a little.";
  const l = p.toLowerCase();
  if (["password", "wachtwoord", "123456789", "qwertyuiop", "sompio"].some((w) => l.includes(w)) && [...p].length < 16)
    return "That password is easy to guess. Try a few unrelated words.";
  return null;
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

/** Opens any web page (https/http) or a ready-to-send email (mailto) in the person's own browser or mail app. */
export async function openExternal(url: string) {
  if (!/^(https?:\/\/|mailto:)/i.test(url.trim())) throw "Only web and email links can be opened.";
  if (!inDesktopApp) {
    window.open(url, "_blank", "noopener");
    return;
  }
  await call<void>("open_external", { url });
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

type Filter = { name: string; extensions: string[] };
/** Any file to open. */
export async function pickOpen(title: string, filters: Filter[], demo = "C:\\Users\\you\\Downloads\\file"): Promise<string | null> {
  if (!inDesktopApp) return demo;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ title, filters });
  return typeof picked === "string" ? picked : null;
}
/** Where to save a file. */
export async function pickSave(title: string, defaultPath: string, filters: Filter[]): Promise<string | null> {
  if (!inDesktopApp) return defaultPath;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return save({ title, defaultPath, filters });
}

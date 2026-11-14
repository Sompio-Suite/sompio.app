// Fake backend for working on the UI in a plain browser. Never used inside the desktop app.
// It mimics the Rust side closely enough to click through every screen.
import { calendarHandlers } from "./mockCalendar";
import { vaultHandlers } from "./mockVault";
import type { ChannelView, FindingView, KindKey, MessageView, Page, Platform, ScanSummary, Segment, SourceView, Stats, Status } from "./api";

const sentences = ["morning everyone", "has anyone found a good dentist near the centre?", "thanks, that helped a lot", "the meeting moved to 3 pm", "can you share the slides after the call?", "I'll be about ten minutes late", "great photos from the weekend", "the train was delayed again", "see you tomorrow", "my laptop keeps restarting after the update", "the flat viewing is at 6, want to come along?", "booked the flights, finally", "running late, start without me", "good luck with the interview!"];
let seed = 7;
const rand = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
const pick = <T,>(a: T[]) => a[Math.floor(rand() * a.length)];

const KINDS: Record<KindKey, [string, number, string]> = {
  credential: ["Passwords, tokens & keys", 3, "Change this password or revoke this token now. Deleting the message doesn't undo the leak, because others may already have seen it."],
  card: ["Payment cards", 3, "Contact your bank to block or replace this card, then delete the message."],
  iban: ["Bank accounts (IBAN)", 3, "An IBAN alone can't empty your account, but scammers use it to sound convincing. Delete the message unless you share this account number publicly anyway."],
  watch_term: ["Your watch terms", 3, "You told Sompio this is sensitive. Decide whether this message needs to go."],
  address: ["Street addresses", 3, "Delete the message unless you're fine with everyone in this chat knowing this address."],
  phone: ["Phone numbers", 2, "Delete the message if you don't want this number linked to your Discord account."],
  email: ["Email addresses", 2, "Check whether you want this address linked to your Discord account. If it's a public work address, mark it as not a problem everywhere."],
  ip_address: ["IP addresses", 2, "An IP address can show roughly where you are. Delete the message if this was your home connection."],
  sensitive_file: ["Sensitive files on this PC", 2, "This file's name suggests an identity document, money matters or secrets. Keep it only if you need it, ideally in an encrypted folder or password manager, and move it to the Recycle Bin if you don't."],
  postcode: ["Postcodes", 1, "A postcode narrows down where you live. Low risk on its own, more so next to a street name."],
  id_number: ["National ID numbers", 3, "A national ID number (like a BSN, CNP or social security number) is what fraudsters need to open accounts in your name. Delete the message, and only give this number to official bodies, your employer, bank or doctor."],
  passport: ["Passport & ID card numbers", 3, "With a passport or ID card number, someone can pose as you. Delete the message. If a photo of the document went out too, report it to your town hall or the police and consider replacing the document."],
  backup_code: ["Backup codes & recovery phrases", 3, "Backup codes and recovery phrases get around your two-factor login or open a crypto wallet outright. Make new backup codes on that site now (that cancels the old ones), or move the coins to a new wallet, then delete the message."],
  birth_date: ["Dates of birth", 2, "Your date of birth is a common security question and a key piece for identity fraud, together with your name. Delete the message if this is your own date of birth."],
  location: ["Exact locations", 2, "This photo or map link shows an exact spot. If it's your home, school or work, delete the post. Before sharing a photo again, remove its location. On Windows: right-click the file, Properties, Details, Remove Properties and Personal Information. On a Mac, in Photos: Image, Location, Hide Location."],
  number_plate: ["Number plates", 1, "A number plate can be tied to your car, and so to where you park and drive. Delete the message unless it's fine for this chat to know it."],
  crypto_wallet: ["Crypto wallets", 1, "Wallet addresses are public, but tying one to your account reveals its balance and history."],
};
const LEAKS: [string, KindKey, string][] = [
  ["my email is ", "email", "sam.rivers.private@example.org"],
  ["text me on ", "phone", "06-12345678"],
  ["send it to ", "address", "Keizersgracht 123"],
  ["IBAN is ", "iban", "NL91 ABNA 0417 1643 00"],
  ["login is sammy and ", "credential", "password: Winter2024!"],
  ["my home server is at ", "ip_address", "81.204.37.12"],
  ["card is ", "card", "4111 1111 1111 1111"],
  ["for the rental form, ", "id_number", "BSN 111222333"],
  ["photo of my ", "passport", "passport NXC4R9P12"],
  ["saving these here, ", "backup_code", "backup codes: 8f3k-2m9x"],
  ["party is on my birthday, ", "birth_date", "born on 14-03-1994"],
  ["I'm parked outside in ", "number_plate", "GZ-123-K"],
  ["come over, we're here ", "location", "https://www.google.com/maps/@52.3731234,4.8903456,17z"],
];

const channelDefs: [string, ChannelView["kind"], string | null][] = [
  ["general", "server", "Amsterdam Expats"], ["housing", "server", "Amsterdam Expats"], ["marketplace", "server", "Amsterdam Expats"],
  ["help", "server", "Home Lab Builders"], ["Direct message with sam", "dm", null], ["Direct message with noor", "dm", null], ["Weekend crew", "group_dm", null],
];

interface Msg { id: string; channel_id: string; channel_name: string; platform: Platform; guild: string | null; ts: number; content: string; attachments: string[]; file_path?: string }
let channels: ChannelView[] = [];
let messages: Msg[] = [];
let sources: SourceView[] = [];
let nextId = 1;
channelDefs.forEach(([name, kind, guild], ci) => {
  const cid = String(1000 + ci);
  const label = guild ? `${name} in ${guild}` : name;
  const n = 300 + Math.floor(rand() * 1600);
  let t = Date.UTC(2018, 4, 1) + rand() * 3e10;
  for (let i = 0; i < n; i++) {
    t += rand() * 9e7;
    let content = pick(sentences);
    if (rand() < 0.015) {
      const [pre, , value] = pick(LEAKS);
      content = pre + value + (rand() < 0.5 ? " if you need it" : "");
    }
    messages.push({ id: String(nextId++), channel_id: cid, channel_name: label, platform: "discord", guild: guild ? "9" + ci : null, ts: t, content, attachments: rand() < 0.04 ? ["https://cdn.discordapp.com/attachments/1/2/screenshot.png"] : [] });
  }
  channels.push({ id: cid, platform: "discord", name: label, kind, guild_name: guild, message_count: n, first_ts: null, last_ts: t });
});
channels.sort((a, b) => b.message_count - a.message_count);
const baseChannels = [...channels];
const baseMessages = [...messages];

const PC_FILES: [string, string][] = [
  ["taxes/2023/Passport_scan.jpg", ""],
  ["notes.txt", "wifi password: SuperSecret99 and the router admin"],
  ["cv.docx", "Sam Rivers, sam.rivers.private@example.org, 06-12345678"],
  ["holiday packing list.txt", "socks, charger, sunscreen"],
];
const SENSITIVE_WORDS = ["passport", "cv"];

interface Finding { id: number; msg: Msg; kind: KindKey; start: number; end: number }
let findings: Finding[] = [];
const statuses = new Map<number, Status>();
const ignored = new Set<string>();
let loaded = false;
let demo = false;
let scanned = false;
let terms: string[] = [];
let traceData = { profile: { name: "", email: "", country: "", handles: {} }, exports: {}, requests: [] } as unknown;

const url = (m: Msg) => (m.platform === "pc" ? null : `https://discord.com/channels/${m.guild ?? "@me"}/${m.channel_id}/${m.id}`);
const mask = (s: string) => s.replace(/(\p{L}|\d)([\p{L}\d]*)/gu, (_, a, b) => a + "•".repeat(b.length));
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const page = <T,>(all: T[], offset: number, limit: number): Page<T> => ({ total: all.length, offset, items: all.slice(offset, offset + limit) });
const listeners: Record<string, ((p: unknown) => void)[]> = {};
const emit = (e: string, p: unknown) => (listeners[e] || []).forEach((cb) => cb(p));
(globalThis as { __mockEmit?: typeof emit }).__mockEmit = emit; // the preview only: lets screenshots show events

function scan() {
  findings = [];
  let id = 1;
  for (const m of messages) {
    if (m.platform === "pc") {
      const nameEnd = m.content.indexOf("\n") < 0 ? m.content.length : m.content.indexOf("\n");
      const name = m.content.slice(0, nameEnd).toLowerCase();
      for (const w of SENSITIVE_WORDS) {
        const at = name.search(new RegExp(`(^|[^a-z])${w}([^a-z]|$)`));
        if (at >= 0) { const s0 = name.indexOf(w, at); findings.push({ id: id++, msg: m, kind: "sensitive_file", start: s0, end: s0 + w.length }); break; }
      }
      for (const [k, re] of [["credential", /password: \S+/], ["email", /\S+@example\.org/], ["phone", /06-\d{8}/]] as [KindKey, RegExp][]) {
        const mm = m.content.slice(nameEnd).match(re);
        if (mm && mm.index !== undefined) findings.push({ id: id++, msg: m, kind: k, start: nameEnd + mm.index, end: nameEnd + mm.index + mm[0].length });
      }
      continue;
    }
    for (const [, kind, value] of LEAKS) {
      const at = m.content.indexOf(value);
      if (at >= 0 && !ignored.has(kind + value)) findings.push({ id: id++, msg: m, kind, start: at, end: at + value.length });
    }
    for (const t of terms) {
      const at = m.content.toLowerCase().indexOf(t.toLowerCase());
      if (at >= 0) findings.push({ id: id++, msg: m, kind: "watch_term", start: at, end: at + t.length });
    }
  }
  scanned = true;
}

/** Same value, for grouping (the real app uses a hash of the normalised value). */
const valueKey = (f: Finding) => f.kind + "|" + f.msg.content.slice(f.start, f.end).toLowerCase().replace(/[^\p{L}\d@.]/gu, "");

function view(f: Finding, reveal: boolean, repeats = 1): FindingView {
  const [label, severity, advice] = KINDS[f.kind];
  const c = f.msg.content;
  const v = c.slice(f.start, f.end);
  return {
    id: f.id, message_id: f.msg.id, channel_id: f.msg.channel_id, channel_name: f.msg.channel_name, platform: f.msg.platform, ts: f.msg.ts,
    kind: f.kind, label, severity, advice, before: c.slice(0, f.start), matched: reveal ? v : mask(v), after: c.slice(f.end),
    masked: !reveal, status: statuses.get(f.id) ?? null, link: url(f.msg), file_path: f.msg.file_path ?? null, repeats,
  };
}

function messageView(m: Msg, reveal: boolean): MessageView {
  const spans = findings.filter((f) => f.msg === m).sort((a, b) => a.start - b.start);
  const segments: Segment[] = [];
  let pos = 0;
  for (const f of spans) {
    if (f.start < pos) continue;
    if (f.start > pos) segments.push({ text: m.content.slice(pos, f.start), sensitive: false });
    const v = m.content.slice(f.start, f.end);
    segments.push({ text: reveal ? v : mask(v), sensitive: true });
    pos = f.end;
  }
  if (pos < m.content.length || !segments.length) segments.push({ text: m.content.slice(pos), sensitive: false });
  return { ...m, content: segments.map((s) => s.text).join(""), segments, has_sensitive: spans.length > 0, link: url(m), file_path: m.file_path ?? null };
}

function summary(): ScanSummary {
  const st = (f: Finding) => statuses.get(f.id);
  return {
    scanned: scanned ? messages.length : 0,
    flagged_messages: new Set(findings.map((f) => f.msg.id)).size,
    total_findings: findings.length,
    open: findings.filter((f) => !st(f)).length,
    later: findings.filter((f) => st(f) === "later").length,
    done: findings.filter((f) => st(f) === "deleted" || st(f) === "ok").length,
    ignored_rules: ignored.size,
    scanned_at_ms: scanned ? Date.now() : null,
    by_kind: (Object.keys(KINDS) as KindKey[]).map((k) => {
      const fs = findings.filter((f) => f.kind === k);
      return { kind: k, label: KINDS[k][0], severity: KINDS[k][1], advice: KINDS[k][2], count: fs.length, open: fs.filter((f) => !st(f) || st(f) === "later").length };
    }),
  };
}

const matchesTab = (f: Finding, tab: string) => {
  const s = statuses.get(f.id);
  return tab === "open" ? !s : tab === "later" ? s === "later" : tab === "done" ? s === "deleted" || s === "ok" : true;
};

const reset = () => {
  loaded = false; demo = false; scanned = false; findings = []; statuses.clear(); ignored.clear();
  channels = [...baseChannels]; messages = [...baseMessages]; sources = [];
};
const discordSource = (): SourceView => ({ platform: "discord", label: "Discord", origin: "C:\\Users\\you\\Downloads\\package.zip", imported_at: Date.now(), channels: baseChannels.length, messages: baseMessages.length });

// Licences in the browser preview: a 30-day trial that started 3 days ago; any long SOMPIO- key adds 30 suite days.
const DAY = 86_400_000;
const trialStart = Date.now() - 3 * DAY;
const mockKeys: { key: string; at: number }[] = [];
function licenceStatus() {
  const now = Date.now();
  const trialEnd = trialStart + 30 * DAY;
  let end = trialEnd;
  for (const k of mockKeys) end = Math.max(end, k.at) + (k.key.includes("FOREVER") ? 65535 : 30) * DAY;
  let paid = mockKeys.length > 0;
  // Screenshots: window.__mockPass = days left on a paid pass
  const pass = (globalThis as { __mockPass?: number }).__mockPass;
  if (typeof pass === "number") {
    end = now + pass * DAY;
    paid = true;
  }
  return {
    apps: ["clean", "calendar", "vault"].map((app) => ({
      app, name: app[0].toUpperCase() + app.slice(1), active: end > now, ends_at: end, trial_only: !paid,
    })),
    keys: mockKeys.map((k, i) => ({ serial: `a1b2c3d4e5f6070${i}`, label: "Sompio Suite", days: k.key.includes("FOREVER") ? 65535 : 30, activated_at: k.at, test: true, invalid: false })).reverse(),
    trial_ends_at: trialEnd, clock_behind: false, now, beta: true,
  };
}

// The lock in the browser preview: starts not set up, like a first run.
let lockState: import("./api").LockStatus = { set_up: false, unlocked: false, remembers_picture: false, hardware_keys: [] };
let mockPassword = "";

let mail = { connected: false, address: null as string | null, provider: null as string | null };
let myNames: string[] = [];
let exportAsked: Record<string, string> = {};
let tg = { has_app_key: false, connected: false, name: null as string | null, waiting_for: null as null | "code" | "password" };
const handlers: Record<string, (a: any) => unknown> = {
  tg_status: () => ({ ...tg }),
  tg_set_app_key: ({ apiId }) => { if (!/^\d+$/.test(apiId)) throw "The api_id is a number, like 1234567."; tg.has_app_key = true; },
  tg_start_login: async () => { await wait(400); tg.waiting_for = "code"; },
  tg_submit_code: async ({ code }) => {
    await wait(300);
    if (code !== "12345") throw "That code isn't right. Check the newest message from Telegram and try again. (In the preview, use 12345.)";
    tg.waiting_for = "password";
    return { step: "need_password", hint: "the usual" };
  },
  tg_submit_password: async () => { await wait(300); tg = { ...tg, connected: true, name: "Sam Rivers", waiting_for: null }; return { step: "done", name: "Sam Rivers" }; },
  tg_disconnect: () => { tg = { ...tg, connected: false, name: null, waiting_for: null }; },
  tg_sync: async () => {
    for (let i = 0; i <= 12; i++) { emit("tg-progress", { chats_done: i, chats_total: 12, messages: i * 31 }); await wait(80); }
    sources = [...sources.filter((x) => x.platform !== "telegram"), { platform: "telegram", label: "Telegram", origin: "Connected Telegram account", imported_at: Date.now(), channels: 12, messages: 372 }];
    return { platform: "telegram", channels: 12, messages: 372, servers: 0, warnings: [] };
  },
  tg_delete: ({ id }) => { const f = findings.find((x) => x.id === id)!; findings.filter((x) => x.msg === f.msg).forEach((x) => statuses.set(x.id, "deleted")); return summary(); },
  email_providers: () => [
    { id: "gmail", name: "Gmail", host: "imap.gmail.com", help_url: "https://myaccount.google.com/apppasswords" },
    { id: "yahoo", name: "Yahoo Mail", host: "imap.mail.yahoo.com", help_url: "https://login.yahoo.com/account/security" },
    { id: "icloud", name: "iCloud Mail", host: "imap.mail.me.com", help_url: "https://account.apple.com" },
    { id: "gmx", name: "GMX", host: "imap.gmx.net", help_url: "https://www.gmx.net" },
    { id: "fastmail", name: "Fastmail", host: "imap.fastmail.com", help_url: "https://app.fastmail.com/settings/security/apps" },
    { id: "other", name: "Other (IMAP)", host: "", help_url: "" },
  ],
  email_status: () => ({ ...mail }),
  email_connect: async ({ provider, address }) => {
    await wait(500);
    if (!String(address).includes("@")) throw "Enter your full email address.";
    mail = { connected: true, address, provider };
    return { ...mail };
  },
  email_disconnect: () => { mail = { connected: false, address: null, provider: null }; },
  email_sync: async () => {
    for (let i = 0; i <= 1840; i += 100) { emit("email-progress", { done: Math.min(i, 1840), total: 1840 }); await wait(60); }
    sources = [...sources.filter((x) => x.platform !== "email"), { platform: "email", label: "Email", origin: "Connected email account", imported_at: Date.now(), channels: 1, messages: 1840 }];
    return { platform: "email", channels: 1, messages: 1840, servers: 0, warnings: [] };
  },
  email_delete: ({ id }) => { statuses.set(id, "deleted"); return summary(); },
  my_names: () => [...myNames],
  export_requests: () => JSON.stringify(exportAsked),
  set_export_requests: ({ json }) => { exportAsked = JSON.parse(json as string); },
  set_my_names: ({ names }) => { myNames = names; },
  lock_status: () => ({ ...lockState }),
  lock_new_picture: async () => {
    await wait(400);
  },
  lock_setup_with_picture: async ({ password }) => {
    await wait(600);
    if (String(password).length < 10) throw "Use at least 10 characters. A few unrelated words work well.";
    lockState = { set_up: true, unlocked: true, remembers_picture: true, hardware_keys: [] };
    mockPassword = String(password);
    return "7KQ2-XR9D-4M8T-PLWA-0G3H-ZE5N";
  },
  lock_setup: async ({ password }) => {
    await wait(600);
    if (String(password).length < 10) throw "Use at least 10 characters. A few unrelated words work well.";
    lockState = { set_up: true, unlocked: true, remembers_picture: true, hardware_keys: [] };
    mockPassword = String(password);
    return "7KQ2-XR9D-4M8T-PLWA-0G3H-ZE5N";
  },
  lock_unlock: async ({ password }) => {
    await wait(500);
    if (password !== mockPassword) throw "That password isn't right. Check Caps Lock and try again.";
    lockState = { ...lockState, unlocked: true };
  },
  lock_recover: async () => {
    await wait(600);
    lockState = { set_up: true, unlocked: true, remembers_picture: true, hardware_keys: [] };
    return "M3PA-9VX2-QH7K-W0TC-8RJD-5NFE";
  },
  lock_change_password: async ({ old, new: nu }) => {
    await wait(500);
    if (old !== mockPassword) throw "That password isn't right. Check Caps Lock and try again.";
    mockPassword = String(nu);
  },
  trace_data: () => JSON.parse(JSON.stringify(traceData)),
  trace_save: ({ data }) => { traceData = JSON.parse(JSON.stringify(data)); },
  trace_remind: ({ date, title }) => console.info("calendar reminder", date, title),
  open_external: ({ url }) => { window.open(String(url), "_blank", "noopener"); },
  log_problem: ({ text }) => console.warn("problem:", text),
  app_details: () => ({ version: "0.7.2", os: "windows 10.0.22631.4460 (x86_64)", log: "C:\\Users\\you\\AppData\\Local\\app.sompio.desktop\\problems.log", recent: ["2026-11-09 15:40:02 Sompio 0.6.0 started on windows x86_64"] }),
  open_problems_folder: () => undefined,
  lock_yubikeys_present: () => [{ name: "YubiKey OTP+FIDO+CCID", serial: 21786543 }],
  lock_unlock_yubikey: async ({ password }) => {
    emit("yubikey-touch", null);
    await wait(900);
    if (password !== mockPassword) throw "That password isn't right. Check Caps Lock and try again.";
    lockState = { ...lockState, unlocked: true };
  },
  lock_add_yubikey: async ({ label }) => {
    emit("yubikey-touch", null);
    await wait(900);
    lockState = { ...lockState, hardware_keys: [...lockState.hardware_keys, { index: lockState.hardware_keys.length, serial: 21786543, label: label || "YubiKey 21786543", slot: 2, added_ms: Date.now() }] };
  },
  lock_remove_yubikey: ({ index }) => { lockState = { ...lockState, hardware_keys: lockState.hardware_keys.filter((k) => k.index !== index) }; },
  lock_change_password_yubikey: async ({ old, new: nu }) => {
    await wait(500);
    if (old !== mockPassword) throw "That password isn't right. Check Caps Lock and try again.";
    mockPassword = String(nu);
  },
  lock_forget_picture: () => { lockState = { ...lockState, remembers_picture: false }; },
  lock_lock: () => { lockState = { ...lockState, unlocked: false }; },
  licence_status: () => licenceStatus(),
  licence_add: async ({ key }) => {
    await wait(300);
    const k = String(key).trim().toUpperCase();
    if (!k.startsWith("SOMPIO")) throw "That doesn't look like a Sompio key. Keys start with SOMPIO-.";
    if (k.replace(/[^A-Z0-9]/g, "").length < 120) throw "That key is shorter than it should be. Copy it again, all of it.";
    if (mockKeys.some((m) => m.key === k)) throw "This key is already added on this device.";
    mockKeys.push({ key: k, at: Date.now() });
    return { label: "Sompio Suite", days: k.includes("FOREVER") ? 65535 : 30, test: true, status: licenceStatus() };
  },
  has_data: () => loaded,
  is_demo: () => demo,
  load_demo: async () => { reset(); await wait(300); loaded = true; demo = true; sources = [discordSource()]; },
  import_package: async () => {
    reset();
    for (let i = 0; i <= channels.length; i++) { emit("import-progress", { phase: "Importing messages", done: i, total: channels.length }); await wait(80); }
    loaded = true;
    sources = [...sources.filter((x) => x.platform !== "discord"), discordSource()];
    return { platform: "discord", channels: channels.length, messages: messages.length, servers: 2, warnings: [] };
  },
  scan_folders: async ({ folders }) => {
    if (!loaded) reset();
    demo = false;
    channels = channels.filter((c) => c.platform !== "pc");
    messages = messages.filter((m) => m.platform !== "pc");
    const root = folders[0];
    channels.push({ id: "pc1", platform: "pc", name: root.split("\\").pop(), kind: "folder", guild_name: root, message_count: PC_FILES.length, first_ts: null, last_ts: null });
    PC_FILES.forEach(([rel, text], i) => messages.push({
      id: `pc-${i}`, channel_id: "pc1", channel_name: root.split("\\").pop()!, platform: "pc", guild: null, ts: Date.now() - i * 864e5,
      content: text ? `${rel}\n\n${text}` : rel, attachments: [], file_path: `${root}\\${rel.replace(/\//g, "\\")}`,
    }));
    for (let i = 0; i <= 5; i++) { emit("import-progress", { phase: "Reading files", done: i, total: 5 }); await wait(60); }
    loaded = true;
    sources = [...sources.filter((x) => x.platform !== "pc"), { platform: "pc", label: "This PC", origin: folders.join("\n"), imported_at: Date.now(), channels: 1, messages: PC_FILES.length }];
    return { platform: "pc", channels: 1, messages: PC_FILES.length, servers: 0, warnings: [] };
  },
  sources: () => sources,
  remove_source: ({ platform }) => {
    channels = channels.filter((c) => c.platform !== platform);
    messages = messages.filter((m) => m.platform !== platform);
    sources = sources.filter((x) => x.platform !== platform);
    findings = findings.filter((f) => f.msg.platform !== platform);
    if (!messages.length) loaded = false;
  },
  recycle_file: ({ id }) => {
    const f = findings.find((x) => x.id === id)!;
    findings.filter((x) => x.msg === f.msg).forEach((x) => statuses.set(x.id, "deleted"));
    return [f.msg.file_path, summary()];
  },
  show_in_folder: () => undefined,
  stats: (): Stats => {
    const years = new Map<number, number>();
    messages.forEach((m) => { const y = new Date(m.ts).getUTCFullYear(); years.set(y, (years.get(y) || 0) + 1); });
    const chat = messages.filter((m) => m.platform !== "pc");
    return {
      messages: chat.length, channels: channels.filter((c) => c.platform !== "pc").length, servers: 2, dms: 2, group_dms: 1,
      files: messages.length - chat.length, sources,
      with_attachments: messages.filter((m) => m.attachments.length).length,
      first_ts: Math.min(...messages.map((m) => m.ts)), last_ts: Math.max(...messages.map((m) => m.ts)),
      imported_at_ms: Date.now(), per_year: [...years].sort((a, b) => a[0] - b[0]).map(([year, count]) => ({ year, count })), top_channels: channels.slice(0, 8),
    };
  },
  channels: ({ filter }) => channels.filter((c) => !filter || c.name.toLowerCase().includes(filter.toLowerCase())),
  messages: ({ channelId, offset, limit }) => {
    const p = page(messages.filter((m) => m.channel_id === channelId), offset, limit);
    return { ...p, items: p.items.map((m) => messageView(m, false)) };
  },
  message_index: ({ channelId, messageId }) => messages.filter((m) => m.channel_id === channelId).findIndex((m) => m.id === messageId),
  reveal_message: ({ messageId }) => { const m = messages.find((x) => x.id === messageId); return m ? messageView(m, true) : null; },
  search: ({ text, channelId, offset, limit }) => {
    const q = text.toLowerCase();
    const p = page(messages.filter((m) => (!channelId || m.channel_id === channelId) && m.content.toLowerCase().includes(q)).reverse(), offset, limit);
    return { ...p, items: p.items.map((m) => messageView(m, false)) };
  },
  run_scan: async ({ watchTerms }) => {
    terms = watchTerms;
    for (let i = 0; i <= 10; i++) { emit("scan-progress", [(i * messages.length) / 10, messages.length]); await wait(60); }
    scan();
    return summary();
  },
  scan_summary: () => summary(),
  findings: ({ kind, platform, status, offset, limit, reveal, group, sameAs }) => {
    const same = sameAs ? findings.find((f) => f.id === sameAs) : null;
    const all = findings.filter((f) => (!kind || f.kind === kind) && (!platform || f.msg.platform === platform) && matchesTab(f, status)
      && (!same || valueKey(f) === valueKey(same)))
      .sort((a, b) => KINDS[b.kind][1] - KINDS[a.kind][1] || b.msg.ts - a.msg.ts);
    const counts = new Map<string, number>();
    all.forEach((f) => counts.set(valueKey(f), (counts.get(valueKey(f)) ?? 0) + 1));
    let list = all;
    if (group) {
      const seen = new Set<string>();
      list = all.filter((f) => !seen.has(valueKey(f)) && !!seen.add(valueKey(f)))
        .sort((a, b) => KINDS[b.kind][1] - KINDS[a.kind][1] || counts.get(valueKey(b))! - counts.get(valueKey(a))! || b.msg.ts - a.msg.ts);
    }
    const p = page(list, offset, limit);
    return { ...p, items: p.items.map((f) => view(f, reveal, counts.get(valueKey(f)))) };
  },
  group_info: ({ id, status }) => {
    const me = findings.find((f) => f.id === id)!;
    const members = findings.filter((f) => valueKey(f) === valueKey(me) && matchesTab(f, status)).sort((a, b) => a.msg.ts - b.msg.ts);
    const platforms = new Map<string, number>();
    members.forEach((f) => platforms.set(f.msg.platform, (platforms.get(f.msg.platform) ?? 0) + 1));
    return {
      ids: members.map((f) => f.id), message_ids: members.map((f) => f.msg.id), member_platforms: members.map((f) => f.msg.platform),
      platforms: [...platforms].map(([platform, count]) => ({ platform, count })).sort((a, b) => b.count - a.count),
      first_ts: members[0]?.msg.ts ?? 0, last_ts: members[members.length - 1]?.msg.ts ?? 0,
    };
  },
  set_status_many: ({ ids, status }) => {
    const before = (ids as number[]).map((id) => [id, statuses.get(id) ?? null]);
    (ids as number[]).forEach((id) => (status ? statuses.set(id, status) : statuses.delete(id)));
    return [before, summary()];
  },
  restore_statuses: ({ previous }) => {
    (previous as [number, Status | null][]).forEach(([id, st]) => (st ? statuses.set(id, st) : statuses.delete(id)));
    return summary();
  },
  reveal_finding: ({ id }) => { const f = findings.find((x) => x.id === id); return f ? view(f, true) : null; },
  set_status: ({ id, status }) => { if (status) statuses.set(id, status); else statuses.delete(id); return summary(); },
  ignore_everywhere: ({ id }) => {
    const f = findings.find((x) => x.id === id)!;
    const value = f.msg.content.slice(f.start, f.end);
    ignored.add(f.kind + value);
    const before = findings.length;
    findings = findings.filter((x) => !(x.kind === f.kind && x.msg.content.slice(x.start, x.end) === value));
    return [before - findings.length, summary()];
  },
  clear_ignored: () => { const n = ignored.size; ignored.clear(); return n; },
  watch_terms: () => terms,
  export_flagged: () => new Set(findings.map((f) => f.msg.id)).size,
  wipe_data: () => reset(),
  data_location: () => "C:\\Users\\you\\AppData\\Local\\app.sompio.desktop\\sompio.db",
};

Object.assign(handlers, calendarHandlers, vaultHandlers);


export const mockBackend = {
  invoke: async <T,>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
    await wait(10);
    const h = handlers[cmd];
    if (!h) throw `mock: no handler for ${cmd}`;
    return (await h(args || {})) as T;
  },
  listen: async <T,>(event: string, cb: (p: T) => void) => {
    (listeners[event] ||= []).push(cb as (p: unknown) => void);
    return () => { listeners[event] = listeners[event].filter((x) => x !== cb); };
  },
};

// Fake backend for working on the UI in a plain browser. Never used inside the desktop app.
import type { ChannelView, FindingView, KindCount, KindKey, MessageView, Page, ScanSummary, Stats } from "./api";

const words = "anyone up for raid tonight the boss was rough need a healer brb dinner that patch broke my ui see you later nice drop gg honestly no idea what happened lol".split(" ");
let seed = 7;
const rand = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
const pick = <T,>(a: T[]) => a[Math.floor(rand() * a.length)];

const channelNames: [string, ChannelView["kind"], string | null][] = [
  ["general", "server", "HOOLIGANS"], ["raid-planning", "server", "HOOLIGANS"], ["loot", "server", "HOOLIGANS"],
  ["off-topic", "server", "Retro CS 1.6"], ["scrims", "server", "Retro CS 1.6"], ["Direct message with mara", "dm", null],
  ["Direct message with tobi", "dm", null], ["Weekend crew", "group_dm", null], ["help-desk", "server", "Tauri Builders"],
];
const leaks: [KindKey, string][] = [
  ["email", "mail me at skip.private@example.org"], ["phone", "bel me op 06-12345678"],
  ["address", "I live at Prinsengracht 263, 1016 GV"], ["iban", "IBAN NL91 ABNA 0417 1643 00"],
  ["credential", "password: hunter2-for-real"], ["ip_address", "server ip 81.204.12.7"],
];

const channels: ChannelView[] = [];
const messages: MessageView[] = [];
const findings: (FindingView & { raw: string })[] = [];
let id = 1;
const start = Date.UTC(2019, 0, 1);
channelNames.forEach(([name, kind, guild], ci) => {
  const n = 400 + Math.floor(rand() * 4000);
  const cid = String(1000 + ci);
  const label = guild ? `${name} in ${guild}` : name;
  let t = start + rand() * 3e10;
  for (let i = 0; i < n; i++) {
    t += rand() * 9e6;
    let content = Array.from({ length: 3 + Math.floor(rand() * 22) }, () => pick(words)).join(" ");
    if (rand() < 0.01) {
      const [k, leak] = pick(leaks);
      const before = content + " ";
      content = before + leak;
      findings.push({
        id: findings.length + 1, message_id: String(id), channel_id: cid, channel_name: label, ts: t, kind: k,
        label: k, severity: ["credential", "iban", "address"].includes(k) ? 3 : 2,
        before: "…" + before.slice(-40), matched: leak, after: "", masked: true, raw: leak,
      });
    }
    messages.push({ id: String(id++), channel_id: cid, channel_name: label, ts: t, content, attachments: rand() < 0.04 ? ["https://cdn.discordapp.com/attachments/1/2/screenshot.png"] : [] });
  }
  channels.push({ id: cid, name: label, kind, guild_name: guild, message_count: n, first_ts: null, last_ts: t });
});
channels.sort((a, b) => b.message_count - a.message_count);

const labels: Record<KindKey, [string, number]> = {
  credential: ["Passwords, tokens & keys", 3], card: ["Payment cards", 3], iban: ["Bank accounts (IBAN)", 3],
  watch_term: ["Your watch terms", 3], address: ["Street addresses", 3], phone: ["Phone numbers", 2],
  email: ["Email addresses", 2], ip_address: ["IP addresses", 2], postcode: ["Postcodes", 1], crypto_wallet: ["Crypto wallets", 1],
};
findings.forEach((f) => (f.label = labels[f.kind][0]));
const mask = (s: string) => s.replace(/(\p{L}|\d)([\p{L}\d]*)/gu, (_, a, b) => a + "•".repeat(b.length));

let loaded = false;
let scanned = false;
const page = <T,>(all: T[], offset: number, limit: number): Page<T> => ({ total: all.length, offset, items: all.slice(offset, offset + limit) });
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const listeners: Record<string, ((p: unknown) => void)[]> = {};
const emit = (e: string, p: unknown) => (listeners[e] || []).forEach((cb) => cb(p));

function summary(): ScanSummary {
  const by_kind: KindCount[] = (Object.keys(labels) as KindKey[]).map((k) => {
    const fs = scanned ? findings.filter((f) => f.kind === k) : [];
    return { kind: k, label: labels[k][0], severity: labels[k][1], count: fs.length, messages: fs.length };
  });
  return { scanned: scanned ? messages.length : 0, flagged_messages: scanned ? findings.length : 0, total_findings: scanned ? findings.length : 0, scanned_at_ms: scanned ? Date.now() : null, by_kind };
}

const handlers: Record<string, (a: any) => unknown> = {
  has_data: () => loaded,
  import_package: async () => {
    for (let i = 0; i <= channels.length; i++) { emit("import-progress", { phase: "Importing messages", done: i, total: channels.length }); await wait(120); }
    loaded = true;
    return { channels: channels.length, messages: messages.length, servers: 3, warnings: [] };
  },
  stats: (): Stats => {
    const years = new Map<number, number>();
    messages.forEach((m) => years.set(new Date(m.ts).getUTCFullYear(), (years.get(new Date(m.ts).getUTCFullYear()) || 0) + 1));
    return {
      messages: messages.length, channels: channels.length, servers: 3, dms: 2, group_dms: 1,
      with_attachments: messages.filter((m) => m.attachments.length).length,
      first_ts: Math.min(...messages.map((m) => m.ts)), last_ts: Math.max(...messages.map((m) => m.ts)),
      imported_at_ms: Date.now(), per_year: [...years].sort().map(([year, count]) => ({ year, count })), top_channels: channels.slice(0, 8),
    };
  },
  channels: ({ filter }) => channels.filter((c) => !filter || c.name.toLowerCase().includes(filter.toLowerCase())),
  messages: ({ channelId, offset, limit }) => page(messages.filter((m) => m.channel_id === channelId), offset, limit),
  message_index: ({ channelId, messageId }) => messages.filter((m) => m.channel_id === channelId).findIndex((m) => m.id === messageId),
  search: ({ text, channelId, offset, limit }) => {
    const q = text.toLowerCase();
    return page(messages.filter((m) => (!channelId || m.channel_id === channelId) && m.content.toLowerCase().includes(q)).reverse(), offset, limit);
  },
  run_scan: async () => {
    for (let i = 0; i <= 10; i++) { emit("scan-progress", [i * messages.length / 10, messages.length]); await wait(90); }
    scanned = true;
    return summary();
  },
  scan_summary: () => summary(),
  findings: ({ kind, offset, limit, reveal }) => {
    const all = (scanned ? findings : []).filter((f) => !kind || f.kind === kind).sort((a, b) => b.severity - a.severity || b.ts - a.ts);
    const p = page(all, offset, limit);
    return { ...p, items: p.items.map((f) => ({ ...f, matched: reveal ? f.raw : mask(f.raw), masked: !reveal })) };
  },
  watch_terms: () => [],
  export_flagged: () => findings.length,
  wipe_data: () => { loaded = false; scanned = false; },
  data_location: () => "C:\\Users\\you\\AppData\\Local\\app.veil.desktop\\veil.db",
};

export const mockBackend = {
  invoke: async <T,>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
    await wait(15);
    return (await handlers[cmd](args || {})) as T;
  },
  listen: async <T,>(event: string, cb: (p: T) => void) => {
    (listeners[event] ||= []).push(cb as (p: unknown) => void);
    return () => { listeners[event] = listeners[event].filter((x) => x !== cb); };
  },
};

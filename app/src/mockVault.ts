// Vault in the browser preview: made-up entries, enough to click through every screen.
import type { HealthReport, VaultEntry, VaultEntryDetail, VaultEntryInput, VaultGroup, VaultSettings } from "./api";

interface Item { d: VaultEntryDetail; password: string; otp: string | null; secrets: Record<string, string>; history: { password: string; modified: string }[] }

const ROOT = "root", BIN = "bin";
let groups: VaultGroup[] = [
  { id: ROOT, parent: null, name: "Vault", depth: 0, entries: 0, recycle_bin: false },
  { id: "g-work", parent: ROOT, name: "Work", depth: 1, entries: 0, recycle_bin: false },
  { id: "g-mail", parent: ROOT, name: "Email", depth: 1, entries: 0, recycle_bin: false },
  { id: "g-money", parent: ROOT, name: "Money", depth: 1, entries: 0, recycle_bin: false },
  { id: "g-social", parent: ROOT, name: "Social", depth: 1, entries: 0, recycle_bin: false },
  { id: BIN, parent: ROOT, name: "Recycle Bin", depth: 1, entries: 0, recycle_bin: true },
];
const now = new Date().toISOString().slice(0, 19) + "Z";
const ago = (days: number) => new Date(Date.now() - days * 86_400_000).toISOString().slice(0, 19) + "Z";
let seq = 1;
const make = (group: string, title: string, username: string, url: string, password: string, extra: Partial<VaultEntryDetail> = {}, otp: string | null = null, changedDaysAgo = 30): Item => ({
  d: {
    id: `e${seq++}`, group_id: group, group_path: "", title, username, url, tags: [], has_password: !!password, has_totp: !!otp,
    expires: null, expired: false, modified: ago(changedDaysAgo), in_recycle_bin: false, icon: null, notes: "", custom: [],
    autotype: { enabled: true, sequence: "", windows: [] }, created: ago(400), password_modified: ago(changedDaysAgo), history: [], attachments: [], ...extra,
  },
  password, otp, secrets: {}, history: [],
});
let items: Item[] = [
  make("g-work", "Zoom", "alex.rivers@example.org", "https://zoom.us", "Tundra-Velvet-Crane-42", { tags: ["work"], autotype: { enabled: true, sequence: "{PASSWORD}{ENTER}", windows: ["Zoom*"] } }, "JBSWY3DPEHPK3PXP"),
  make("g-work", "Slack", "alex.rivers@example.org", "https://slack.com", "hunter22", {}, null, 900),
  make("g-mail", "Proton Mail", "sam@proton.me", "https://account.proton.me", "quarry-lantern-fjord-pelican-amber", { notes: "Recovery phrase is in the safe." }, "JBSWY3DPEHPK3PXP"),
  make("g-mail", "Gmail (old)", "sam.old@gmail.com", "https://mail.google.com", "hunter22", {}, null, 1400),
  make("g-money", "ING", "sam", "https://mijn.ing.nl", "Plover!Mosaic-Ridge-9", { custom: [{ key: "PIN", value: null, protected: true }] }),
  make("g-social", "Discord", "sam", "https://discord.com", "BrightMarble77", { tags: ["community"] }, "JBSWY3DPEHPK3PXP", 200),
  make("g-social", "Reddit", "sam_ams", "https://www.reddit.com", "password1"),
  make(ROOT, "Home Wi-Fi", "", "", "orbit-candle-meadow-tusk", { notes: "Guest network: Sompio-Guest" }),
];
items[4].secrets.PIN = "4321";
items[1].history.push({ password: "zoom-2023", modified: ago(1200) });
let settings: VaultSettings = { shortcut: "Ctrl+Alt+A", clear_seconds: 20, lock_after_type: false, auto_icons: true, browser: false };
let browsers = [{ id: "a1b2", browser: "Chrome", added: Date.now() - 86400000 * 3, last_used: Date.now() - 3600000 }];
// In the preview, "downloaded" icons are drawn here: a coloured tile with the site's first letter
const mockIcons: Record<string, string> = {};
const fakeIcon = (site: string) => {
  const hue = [...site].reduce((h, c) => (h * 31 + c.charCodeAt(0)) % 360, 7);
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64'><rect width='64' height='64' rx='14' fill='hsl(${hue},60%,45%)'/><text x='32' y='44' font-size='34' font-family='Arial' font-weight='bold' fill='white' text-anchor='middle'>${site[0]?.toUpperCase() ?? "?"}</text></svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
};
const setFake = (i: Item, site: string) => { const id = `icon-${site}`; mockIcons[id] = fakeIcon(site); i.d.icon = id; return id; };
const siteOf = (u: string) => u.replace(/^https?:\/\//, "").replace(/^www\./, "").split("/")[0];

const path = (g: string): string => {
  const out: string[] = [];
  let cur = groups.find((x) => x.id === g);
  while (cur && cur.parent) { out.unshift(cur.name); cur = groups.find((x) => x.id === cur!.parent); }
  return out.join(" / ");
};
const inBin = (g: string): boolean => { let cur = groups.find((x) => x.id === g); while (cur) { if (cur.id === BIN) return true; cur = groups.find((x) => x.id === cur!.parent); } return false; };
const within = (g: string, scope: string): boolean => { let cur = groups.find((x) => x.id === g); while (cur) { if (cur.id === scope) return true; cur = groups.find((x) => x.id === cur!.parent); } return false; };
const summary = (i: Item): VaultEntry => {
  const { notes: _n, custom: _c, autotype: _a, created: _cr, password_modified: _p, history: _h, ...s } = i.d;
  return { ...s, group_path: path(i.d.group_id), in_recycle_bin: inBin(i.d.group_id), has_password: !!i.password, has_totp: !!i.otp };
};
const detail = (i: Item): VaultEntryDetail => ({
  ...i.d, ...summary(i),
  attachments: (i as Item & { files?: { name: string; size: number }[] }).files ?? [],
  history: i.history.map((h, n) => ({ index: n, modified: h.modified, title: i.d.title, username: i.d.username, password_changed: true })),
});
const find = (id: string) => { const i = items.find((x) => x.d.id === id); if (!i) throw "That entry no longer exists."; return i; };
const code = () => String(Math.floor(Math.random() * 1e6)).padStart(6, "0");

function apply(i: Item, e: VaultEntryInput) {
  if (!e.title.trim()) throw "Give the entry a title.";
  if (e.password != null && e.password !== i.password && i.password) i.history.unshift({ password: i.password, modified: now });
  Object.assign(i.d, { title: e.title.trim(), username: e.username, url: e.url, notes: e.notes, tags: e.tags, expires: e.expires ? `${e.expires}T00:00:00Z` : null, modified: now });
  i.d.expired = !!e.expires && e.expires <= new Date().toISOString().slice(0, 10);
  if (e.group_id) i.d.group_id = e.group_id;
  if (e.password != null) i.password = e.password;
  if (e.otp != null) i.otp = e.otp || null;
  if (e.autotype) i.d.autotype = e.autotype;
  i.d.custom = e.custom.filter((c) => c.key.trim()).map((c) => {
    if (c.value != null) i.secrets[c.key] = c.value;
    return { key: c.key, value: c.protected ? null : (c.value ?? i.secrets[c.key] ?? ""), protected: c.protected };
  });
}

const words = "tundra velvet crane oxygen mallet quarry lantern fjord pelican amber orbit candle meadow tusk plover mosaic ridge harbor".split(" ");
const strength = (pw: string) => {
  const score = pw.length >= 20 ? 4 : pw.length >= 14 ? 3 : pw.length >= 10 ? 2 : pw.length >= 7 ? 1 : 0;
  return { score, label: ["Very weak", "Weak", "Fair", "Strong", "Very strong"][score], bits: pw.length * 4.7, crack_time: ["instantly", "minutes", "days", "centuries", "centuries"][score], warning: score < 2 ? "This is a very common password." : null, suggestions: score < 3 ? ["Add another word or two. Uncommon words are better."] : [] };
};

function health(): HealthReport {
  const live = items.filter((i) => !inBin(i.d.group_id) && i.password);
  const counts = new Map<string, number>();
  live.forEach((i) => counts.set(i.password, (counts.get(i.password) ?? 0) + 1));
  const out: HealthReport["items"] = [];
  let weak = 0, reused = 0, old = 0;
  for (const i of live) {
    const issues: HealthReport["items"][number]["issues"] = [];
    let severity = 0;
    const s = strength(i.password);
    if (s.score <= 2) { weak++; severity = 4 - Math.min(2, s.score); issues.push({ kind: "weak", score: s.score, label: s.label, hint: s.warning }); }
    const n = counts.get(i.password)!;
    if (n > 1) { reused++; severity = Math.max(severity, 3); issues.push({ kind: "reused", count: n, others: live.filter((x) => x !== i && x.password === i.password).map((x) => x.d.title) }); }
    const days = Math.round((Date.now() - Date.parse(i.d.password_modified!)) / 86_400_000);
    if (days >= 365) { old++; severity = Math.max(severity, 1); issues.push({ kind: "old", days }); }
    if (issues.length) out.push({ entry: summary(i), issues, severity });
  }
  out.sort((a, b) => b.severity - a.severity);
  const bad = out.filter((x) => x.severity >= 2).length;
  return { checked: live.length, weak, reused, expired: 0, old, score: Math.round((100 * (live.length - bad)) / live.length), items: out };
}

export const vaultHandlers: Record<string, (a: any) => unknown> = {
  vault_groups: () => groups.map((g) => ({ ...g, entries: items.filter((i) => i.d.group_id === g.id).length })),
  vault_entries: ({ group, search }) => {
    const terms = String(search).toLowerCase().split(/\s+/).filter(Boolean);
    return items
      .filter((i) => (group ? within(i.d.group_id, group) && (!inBin(i.d.group_id) || inBin(group)) : !inBin(i.d.group_id)))
      .filter((i) => terms.every((t) => `${i.d.title} ${i.d.username} ${i.d.url} ${i.d.notes} ${i.d.tags.join(" ")} ${path(i.d.group_id)}`.toLowerCase().includes(t)))
      .map(summary)
      .sort((a, b) => a.title.localeCompare(b.title));
  },
  vault_entry: ({ id }) => detail(find(id)),
  vault_reveal: ({ id, field, history }) => {
    const i = find(id);
    if (history != null) return i.history[history]?.password ?? "";
    return field === "Password" ? i.password : field === "UserName" ? i.d.username : i.secrets[field] ?? "";
  },
  vault_totp: ({ id }) => (find(id).otp ? { code: code(), remaining: 30 - (Math.floor(Date.now() / 1000) % 30), period: 30 } : null),
  vault_copy: () => settings.clear_seconds,
  vault_copy_text: () => settings.clear_seconds,
  vault_add_entry: ({ entry }) => { const i = make(entry.group_id ?? ROOT, "", "", "", ""); apply(i, entry); items.push(i); return i.d.id; },
  vault_icons: () => ({ ...mockIcons }),
  vault_fetch_icon: async ({ id, address }) => {
    await new Promise((r) => setTimeout(r, 500));
    const i = find(id);
    const site = siteOf(address || i.d.url);
    if (!site || !site.includes(".")) throw "This entry has no website. Add one, or enter the address to get the icon from.";
    return setFake(i, site);
  },
  vault_icon_from_file: ({ id }) => setFake(find(id), "picture"),
  vault_remove_icon: ({ id }) => { find(id).d.icon = null; },
  vault_fetch_all_icons: async () => {
    const todo = items.filter((i) => !i.d.icon && i.d.url && !inBin(i.d.group_id));
    for (const i of todo) setFake(i, siteOf(i.d.url));
    return [todo.length, todo.length];
  },
  vault_clone_entry: ({ id }) => {
    const o = find(id);
    const c = make(o.d.group_id, `${o.d.title} - Clone`, o.d.username, o.d.url, o.password, { tags: [...o.d.tags], notes: o.d.notes }, o.otp);
    items.push(c);
    return c.d.id;
  },
  vault_update_entry: ({ id, entry }) => apply(find(id), entry),
  vault_delete_entry: ({ id }) => {
    const i = find(id);
    if (inBin(i.d.group_id)) { items = items.filter((x) => x !== i); return true; }
    (i as Item & { prev?: string }).prev = i.d.group_id;
    i.d.group_id = BIN;
    return false;
  },
  vault_restore_entry: ({ id }) => { const i = find(id) as Item & { prev?: string }; i.d.group_id = i.prev ?? ROOT; },
  vault_restore_version: ({ id, index }) => { const i = find(id); const old = i.history[index]; i.history.unshift({ password: i.password, modified: now }); i.password = old.password; },
  vault_move_entry: ({ id, group }) => { find(id).d.group_id = group; },
  vault_empty_bin: () => { const n = items.filter((i) => inBin(i.d.group_id)).length; items = items.filter((i) => !inBin(i.d.group_id)); return n; },
  vault_add_group: ({ parent, name }) => {
    if (!name.trim()) throw "Give the folder a name.";
    const p = groups.find((g) => g.id === (parent ?? ROOT))!;
    const g = { id: `g${seq++}`, parent: p.id, name: name.trim(), depth: p.depth + 1, entries: 0, recycle_bin: false };
    groups.splice(groups.indexOf(p) + 1, 0, g);
    return g.id;
  },
  vault_rename_group: ({ id, name }) => { groups.find((g) => g.id === id)!.name = name; },
  vault_move_group: () => undefined,
  vault_delete_group: ({ id }) => { const g = groups.find((x) => x.id === id)!; g.parent = BIN; g.depth = 2; groups = [...groups.filter((x) => x !== g), g]; },
  vault_generate: ({ kind, password, passphrase }) => {
    let value: string;
    if (kind === "passphrase") {
      const r = passphrase ?? { words: 6, separator: "-", capitalize: false, number: false };
      value = Array.from({ length: r.words }, () => words[Math.floor(Math.random() * words.length)]).map((w) => (r.capitalize ? w[0].toUpperCase() + w.slice(1) : w)).join(r.separator);
      if (r.number) value += Math.floor(Math.random() * 10);
    } else {
      const r = password ?? { length: 20 };
      const chars = "abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789!#$%&*+-=?@";
      value = Array.from({ length: r.length }, () => chars[Math.floor(Math.random() * chars.length)]).join("");
    }
    return { value, strength: strength(value) };
  },
  vault_strength: ({ password }) => strength(password),
  vault_health: () => health(),
  vault_import_kdbx: ({ password }) => { if (!password) throw "Enter the file's password, or choose its key file."; return { entries: 12, groups: 3, skipped_attachments: 0, group_id: ROOT, warnings: [] }; },
  vault_import_csv: () => ({ entries: 48, groups: 4, skipped_attachments: 0, group_id: ROOT, warnings: [] }),
  vault_export_kdbx: ({ password }) => { if (password.length < 8) throw "Use a password of at least 8 characters for the exported file."; },
  vault_export_csv: () => undefined,
  vault_settings: () => settings,
  vault_set_settings: ({ settings: s }) => { settings = s; },
  browser_clients: () => (settings.browser ? browsers : []),
  browser_forget: ({ id }) => { browsers = browsers.filter((b) => b.id !== id); },
  browser_answer: () => undefined,
  browser_extension_folder: () => "C:\\Users\\you\\AppData\\Local\\Sompio\\extension",
  browser_open_extension_folder: () => undefined,
  browser_installed: () => ["chrome", "edge", "firefox"],
  browser_open_extensions_page: () => undefined,
  vault_autotype_matches: () => ({ window: "Zoom - Sign In", matches: [{ entry: summary(items[0]), sequence: "{PASSWORD}{ENTER}", because: "window rule" }, { entry: summary(items[1]), sequence: "{USERNAME}{TAB}{PASSWORD}{ENTER}", because: "title" }] }),
  vault_autotype_choose: () => undefined,
  vault_autotype_now: () => undefined,
  vault_add_attachment: ({ id, path }) => {
    const i = find(id) as Item & { files?: { name: string; size: number }[] };
    const name = String(path).split(/[\\/]/).pop()!;
    i.files = [...(i.files ?? []).filter((f) => f.name !== name), { name, size: 48_213 }];
    return name;
  },
  vault_save_attachment: () => undefined,
  vault_remove_attachment: ({ id, name }) => { const i = find(id) as Item & { files?: { name: string }[] }; i.files = (i.files ?? []).filter((f) => f.name !== name); },
  vault_merge_kdbx: ({ password }) => { if (!password) throw "Enter the file's password, or choose its key file."; return { added: 1, updated: 2, moved: 0, deleted: 0, warnings: [] }; },
  vault_in_trace: () => [],
  vault_check_leaks: () => items.filter((i) => ["hunter22", "password1"].includes(i.password)).map((i) => ({ entry: summary(i), seen: i.password === "password1" ? 2_418_984 : 1_337 })),
  vault_add_samples: () => {
    const g = { id: `g${seq++}`, parent: ROOT, name: "Samples", depth: 1, entries: 0, recycle_bin: false };
    groups.splice(1, 0, g);
    items.push(make(g.id, "Sompio test login", "tester@example.org", "", "Glacier-Lantern-Fjord-77", { tags: ["try auto-type"] }));
    items.push(make(g.id, "Notepad (auto-type test)", "alex.rivers", "", "typed-by-sompio", { autotype: { enabled: true, sequence: "Hello from Sompio{ENTER}", windows: ["*Notepad*"] } }));
    return g.id;
  },
  samples_write: ({ which }) => ({
    folder: `C:\\Users\\you\\Documents\\Sompio samples\\${which[0].toUpperCase()}${which.slice(1)}`,
    files: which === "vault"
      ? [["Sample KeePass.kdbx", "A KeePass file. Its password is: sample-password"], ["Sample Bitwarden export.csv", "A Bitwarden export with a folder and a 2FA secret."], ["Test login page.html", "Open it in your browser to try auto-type."]]
      : which === "calendar"
        ? [["Northwind team (sample).csv", "A shared team schedule with groups."], ["Sample Desk Reminders folder", "A Desk Reminders folder to bring in."]]
        : [["Discord package.zip", "Discord: a server with two channels and two DMs."], ["Telegram export.zip", "Telegram: a private chat and a group."], ["Sample PC folder", "This PC: a folder for Scan a folder."]],
  }),
};

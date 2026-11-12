// Trace's request centre: where to get your data, the privacy letters, and the company directory.
// The directory and the data protection authorities come from the Datenanfragen.de database
// (public domain, CC0), bundled in public/trace so nothing is looked up online.

import type { Platform } from "./api";

export interface Company {
  s: string; // key
  n: string; // name
  e?: string; // privacy email
  f?: string; // privacy web form
  w?: string; // website
  a?: string; // postal address
  c?: string[]; // countries it's relevant for (absent = everywhere)
  k?: string[]; // categories
  id?: 1; // asks for an ID document
  m?: string; // note
  v?: 1; // checked against the company's privacy policy
}

export interface Authority {
  slug: string;
  name: string;
  "relevant-countries": string[];
  email?: string;
  webform?: string;
  web?: string;
  address?: string;
  "complaint-language"?: string;
}

let companies: Promise<Company[]> | null = null;
let authorities: Promise<Authority[]> | null = null;
export const loadCompanies = () => (companies ??= fetch("/trace/companies.json").then((r) => r.json()));
export const loadAuthorities = () => (authorities ??= fetch("/trace/authorities.json").then((r) => r.json()));

/** Ranks by how well the name or website matches, verified entries first. */
export function searchCompanies(all: Company[], q: string, limit = 12): Company[] {
  const t = q.trim().toLowerCase();
  if (t.length < 2) return [];
  const scored: [number, Company][] = [];
  for (const c of all) {
    const name = c.n.toLowerCase();
    const web = (c.w ?? "").toLowerCase().replace(/^https?:\/\/(www\.)?/, "");
    let score = 0;
    if (name === t || c.s === t) score = 100;
    else if (name.startsWith(t) || web.startsWith(t)) score = 60;
    else if (name.includes(t) || web.includes(t)) score = 30;
    if (!score) continue;
    if (c.v) score += 5;
    scored.push([score - name.length / 100, c]);
  }
  return scored.sort((a, b) => b[0] - a[0]).slice(0, limit).map(([, c]) => c);
}

// ---------- Getting your data ----------

export interface ExportInfo {
  key: string;
  platform: Platform;
  name: string;
  /** Where to ask, step by step (some are only in the app) */
  how: string;
  url?: string;
  /** Usual waiting time, and the day count used for the "check back" reminder */
  wait: string;
  days: number;
  /** Can be connected for results right away (Sources) */
  live?: boolean;
}

export const EXPORTS: ExportInfo[] = [
  { key: "discord", platform: "discord", name: "Discord", how: "In the Discord app: User Settings → Data & Privacy → Request all of my data.", wait: "Up to 30 days, often sooner", days: 30 },
  { key: "telegram", platform: "telegram", name: "Telegram", how: "In Telegram Desktop: Settings → Advanced → Export Telegram data (JSON). Or connect Telegram under Sources for results right away.", wait: "Right away", days: 0, live: true },
  { key: "gmail", platform: "email", name: "Gmail and other email", how: "Connect your mailbox under Sources for results right away, or download your mail from Google Takeout.", url: "https://takeout.google.com", wait: "Right away when connected; Takeout takes hours to days", days: 2, live: true },
  { key: "reddit", platform: "reddit", name: "Reddit", how: "Ask for your data on Reddit's data request page.", url: "https://www.reddit.com/settings/data-request", wait: "Up to 30 days", days: 30 },
  { key: "x", platform: "x", name: "X (Twitter)", how: "Settings → Your account → Download an archive of your data.", url: "https://x.com/settings/download_your_data", wait: "24 hours or longer", days: 2 },
  { key: "facebook", platform: "facebook", name: "Facebook", how: "Accounts Center → Your information and permissions → Download your information. Choose JSON.", url: "https://accountscenter.facebook.com/info_and_permissions/dyi", wait: "Minutes to a few days", days: 3 },
  { key: "instagram", platform: "instagram", name: "Instagram", how: "Accounts Center → Your information and permissions → Download your information. Choose JSON.", url: "https://accountscenter.instagram.com/info_and_permissions/dyi", wait: "Minutes to a few days", days: 3 },
  { key: "tiktok", platform: "tiktok", name: "TikTok", how: "In the TikTok app: Settings and privacy → Account → Download your data. Choose JSON.", wait: "A few days", days: 4 },
  { key: "snapchat", platform: "snapchat", name: "Snapchat", how: "On accounts.snapchat.com, sign in and choose My Data.", url: "https://accounts.snapchat.com", wait: "About a day", days: 2 },
  { key: "linkedin", platform: "linkedin", name: "LinkedIn", how: "Settings → Data privacy → Get a copy of your data.", url: "https://www.linkedin.com/mypreferences/d/download-my-data", wait: "10 minutes to 24 hours", days: 1 },
  { key: "whatsapp", platform: "whatsapp", name: "WhatsApp", how: "Open a chat → More → Export chat (one chat at a time, without media).", wait: "Right away", days: 0 },
];

// ---------- Letters ----------

export type RequestKind = "erasure" | "access" | "objection";

export const KIND_LABEL: Record<RequestKind, string> = {
  erasure: "Delete my data",
  access: "Send me a copy of my data",
  objection: "Stop using my data",
};

export const KIND_SHORT: Record<RequestKind, string> = {
  erasure: "Deletion",
  access: "Copy of data",
  objection: "Objection",
};

export interface LetterInput {
  kind: RequestKind;
  company: string;
  name: string;
  account: string;
  date: string; // as written
}

export function letter(i: LetterInput): { subject: string; body: string } {
  const who = [i.name && `Name: ${i.name}`, i.account && `Account or email address: ${i.account}`].filter(Boolean).join("\n");
  const ident = `So you can find my data:\n${who || "[add the email address or username of your account]"}`;
  const close = `You have one month from receiving this request to respond (Article 12(3) GDPR). Please use the details above only to deal with this request.\n\nKind regards,\n${i.name || "[your name]"}\n${i.date}`;
  if (i.kind === "erasure") {
    return {
      subject: "Request to erase my personal data (Article 17 GDPR)",
      body: `Dear ${i.company} privacy team,\n\nUnder Article 17 of the General Data Protection Regulation (GDPR), I ask you to erase all personal data you hold about me.\n\n${ident}\n\nPlease:\n1. Erase all personal data concerning me, including my messages and posts, and remove it from backups as soon as possible.\n2. Tell anyone you have shared my data with about this request (Article 19 GDPR).\n3. Confirm in writing when the erasure is complete.\n\nIf you believe you must keep some of my data, please tell me which data, why, and on which legal ground.\n\n${close}`,
    };
  }
  if (i.kind === "access") {
    return {
      subject: "Request for access to my personal data (Article 15 GDPR)",
      body: `Dear ${i.company} privacy team,\n\nUnder Article 15 of the General Data Protection Regulation (GDPR), I ask for a copy of all personal data you hold about me.\n\n${ident}\n\nPlease send me, in a common electronic format:\n1. A copy of all my personal data, including messages, posts and any data you have inferred about me.\n2. Why you process it, and on which legal ground.\n3. Who you have shared it with or sold it to.\n4. How long you will keep it.\n5. Where you got it, if not from me.\n6. Whether you use it for automated decisions or profiling, and how.\n\n${close}`,
    };
  }
  return {
    subject: "Objection to processing of my personal data (Article 21 GDPR)",
    body: `Dear ${i.company} privacy team,\n\nUnder Article 21 of the General Data Protection Regulation (GDPR), I object to your processing of my personal data. This includes direct marketing, profiling, and any processing based on your legitimate interests. Where you rely on my consent, I withdraw it (Article 7(3) GDPR).\n\n${ident}\n\nPlease stop processing my data for these purposes, stop sharing it with third parties, and confirm in writing that you have done so.\n\n${close}`,
  };
}

export function reminderLetter(i: LetterInput & { sentOn: string }): { subject: string; body: string } {
  const what = { erasure: "erase my personal data (Article 17 GDPR)", access: "give me access to my personal data (Article 15 GDPR)", objection: "stop processing my personal data (Article 21 GDPR)" }[i.kind];
  return {
    subject: `Reminder: my GDPR request of ${i.sentOn}`,
    body: `Dear ${i.company} privacy team,\n\nOn ${i.sentOn} I asked you to ${what}. The one-month deadline in Article 12(3) GDPR has passed, and I have not received a full answer.\n\nAccount or email address: ${i.account || "[add it]"}\n\nPlease answer within 14 days. Otherwise I will file a complaint with the data protection authority.\n\nKind regards,\n${i.name || "[your name]"}\n${i.date}`,
  };
}

export function complaintText(i: LetterInput & { sentOn: string; reminders: string[] }): { subject: string; body: string } {
  const what = { erasure: "erasure of my personal data (Article 17 GDPR)", access: "access to my personal data (Article 15 GDPR)", objection: "an objection to processing of my personal data (Article 21 GDPR)" }[i.kind];
  const rem = i.reminders.length ? ` I sent a reminder on ${i.reminders.join(" and ")}.` : "";
  return {
    subject: `Complaint about ${i.company}: no answer to my GDPR request`,
    body: `Dear Sir or Madam,\n\nI would like to file a complaint against ${i.company}.\n\nOn ${i.sentOn} I sent them a request for ${what}.${rem} The one-month deadline in Article 12(3) GDPR has passed, and they have not answered in full.\n\nMy account or email address with them: ${i.account || "[add it]"}\n\nI have attached a copy of my request${i.reminders.length ? " and my reminder" : ""}. Please investigate and make sure ${i.company} complies.\n\nKind regards,\n${i.name || "[your name]"}\n${i.date}`,
  };
}

export const mailto = (to: string, l: { subject: string; body: string }) =>
  `mailto:${encodeURIComponent(to)}?subject=${encodeURIComponent(l.subject)}&body=${encodeURIComponent(l.body.replace(/\n/g, "\r\n"))}`;

// ---------- Dates ----------

export const today = () => {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};
export function addDays(iso: string, days: number) {
  const d = new Date(`${iso}T12:00:00`);
  d.setDate(d.getDate() + days);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}
/** One month later, as the GDPR counts it (31 Jan → 28/29 Feb) */
export function addMonth(iso: string) {
  const [y, m, d] = iso.split("-").map(Number);
  const last = new Date(y, m + 1, 0).getDate();
  const ny = m === 12 ? y + 1 : y;
  const nm = m === 12 ? 1 : m + 1;
  return `${ny}-${String(nm).padStart(2, "0")}-${String(Math.min(d, last)).padStart(2, "0")}`;
}
export const niceDate = (iso: string) =>
  new Date(`${iso}T12:00:00`).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
export const daysUntil = (iso: string) => Math.round((new Date(`${iso}T12:00:00`).getTime() - new Date(`${today()}T12:00:00`).getTime()) / 86400000);

// ---------- Find it now ----------

/** Ready-made searches on the platforms themselves, for results before the export arrives. */
export function searches(terms: string[], handles: { discord?: string; reddit?: string; x?: string }) {
  const q = (t: string) => (/\s/.test(t) ? `"${t}"` : t);
  return terms.map((t) => ({
    term: t,
    discord: `${handles.discord ? `from: ${handles.discord} ` : ""}${q(t)}`,
    x: `https://x.com/search?f=live&q=${encodeURIComponent(`${handles.x ? `from:${handles.x.replace(/^@/, "")} ` : ""}${q(t)}`)}`,
    reddit: `https://www.reddit.com/search/?q=${encodeURIComponent(`${handles.reddit ? `author:${handles.reddit.replace(/^u\//, "")} ` : ""}${q(t)}`)}`,
    web: `https://duckduckgo.com/?q=${encodeURIComponent(q(t))}`,
  }));
}

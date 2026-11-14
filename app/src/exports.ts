// Getting your data from each platform: how to ask, and roughly how long it takes.

export interface ExportHow {
  name: string;
  /** How to ask for it, in that platform's own words */
  how: string;
  /** Plain words for the usual wait */
  wait: string;
  /** After this many days, remind the person to check their email */
  remindAfter: number;
  /** Ready straight away (no waiting for an email) */
  instant?: boolean;
}

export const EXPORTS: ExportHow[] = [
  { name: "Discord", how: "User Settings, then Data & Privacy, then Request all of my data. Discord emails you a link.", wait: "usually a few days, at most 30", remindAfter: 3 },
  { name: "Reddit", how: "Settings, then Privacy, then Request your data. Reddit emails you when it's ready.", wait: "a few days, at most 30", remindAfter: 4 },
  { name: "Instagram", how: "Settings, Accounts Center, Your information and permissions, Download your information. Choose All available information and format JSON.", wait: "a few hours to a few days", remindAfter: 1 },
  { name: "Facebook", how: "Settings, Accounts Center, Your information and permissions, Download your information. Choose format JSON. Covers Messenger too.", wait: "a few hours to a few days", remindAfter: 1 },
  { name: "X", how: "Settings, then Your account, then Download an archive of your data. X emails you when it's ready.", wait: "a day or more", remindAfter: 2 },
  { name: "TikTok", how: "Settings and privacy, Account, Download your data. Choose JSON, then download it from the same page when it's ready.", wait: "a few days", remindAfter: 3 },
  { name: "Snapchat", how: "Settings, My Data. Tick Export JSON files, submit, and download the zip from the email Snapchat sends.", wait: "about a day", remindAfter: 1 },
  { name: "LinkedIn", how: "Settings, Data privacy, Get a copy of your data. Choose the larger archive so messages are included.", wait: "up to a day", remindAfter: 1 },
  { name: "Gmail", how: "takeout.google.com, deselect all, tick Mail. Add the .zip or .mbox here. Only mail you sent is read. Faster: connect your mailbox instead.", wait: "a few hours to a few days", remindAfter: 1 },
  { name: "Telegram", how: "In Telegram Desktop: Settings, Advanced, Export Telegram data. Tick Personal information and your chats, choose JSON. It saves straight to your computer.", wait: "ready straight away", remindAfter: 0, instant: true },
  { name: "WhatsApp", how: "On your phone, open a chat, tap the contact or group name, then Export chat, Without media. Send the .txt (or .zip) to this computer and add it here. One chat at a time.", wait: "ready straight away", remindAfter: 0, instant: true },
];

/** Whole days since a YYYY-MM-DD date. */
export function daysSince(day: string): number {
  const then = new Date(`${day}T00:00:00`).getTime();
  return Math.max(0, Math.floor((Date.now() - then) / 86_400_000));
}

export function today(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

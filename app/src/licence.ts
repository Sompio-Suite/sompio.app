import type { AppKey, AppStatus } from "./api";
import { date } from "./format";

const DAY = 86_400_000;

export type Tone = "ok" | "trial" | "ended" | "none";

/** "Trial, 12 days left", "Until 9 Dec 2026", "Ended 3 Nov 2026", "Not unlocked" */
export function accessText(a: AppStatus | undefined, now: number): { text: string; tone: Tone } {
  if (!a || a.ends_at == null) return { text: "Not unlocked", tone: "none" };
  if (!a.active) return { text: `Ended ${date(a.ends_at)}`, tone: "ended" };
  const left = Math.max(1, Math.ceil((a.ends_at - now) / DAY));
  if (a.trial_only) return { text: `Free trial, ${left} ${left === 1 ? "day" : "days"} left`, tone: "trial" };
  return { text: left <= 7 ? `${left} ${left === 1 ? "day" : "days"} left` : `Until ${date(a.ends_at)}`, tone: left <= 7 ? "trial" : "ok" };
}

export interface SuiteApp {
  key: AppKey;
  name: string;
  icon: string;
  line: string;
  /** Built and usable now */
  ready: boolean;
  when?: string;
  plans: string[];
}

export const SUITE: SuiteApp[] = [
  {
    key: "clean",
    name: "Clean",
    icon: "review",
    line: "Find personal details in your chats, posts, sent mail and files, and clean them up.",
    ready: true,
    plans: [],
  },
  {
    key: "calendar",
    name: "Calendar",
    icon: "calendar",
    line: "Your schedule and reminders, with regular privacy check-ups. Opens your Desk Reminders schedule as it is.",
    ready: true,
    plans: [],
  },
  {
    key: "vault",
    name: "Vault",
    icon: "key",
    line: "Your passwords, in the KeePass format, typed in for you where you need them.",
    ready: false,
    when: "Coming later",
    plans: [
      "Opens and saves KeePass files, so you're never locked in",
      "Auto-type with a shortcut, matched to the website you're on",
      "Clean finds a password you once pasted in a chat and offers to save and change it",
      "A monthly check for weak, reused and leaked passwords",
    ],
  },
  {
    key: "voice",
    name: "Voice",
    icon: "mic",
    line: "Voice rooms for your group, end-to-end encrypted. No ID, no phone number.",
    ready: false,
    when: "Coming later",
    plans: [
      "TeamSpeak-style rooms with low delay, built for gaming groups",
      "End-to-end encrypted: the server passes voice along without hearing it",
      "Run your own server, or use a room hosted by us",
      "No ID, face scan or phone number to join",
    ],
  },
];

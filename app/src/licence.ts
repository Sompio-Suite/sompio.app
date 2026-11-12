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
    key: "calendar",
    name: "Calendar",
    icon: "calendar",
    line: "Your schedule and reminders, with regular privacy check-ups. Opens your Desk Reminders schedule as it is.",
    ready: true,
    plans: [],
  },
  {
    key: "clean",
    name: "Trace",
    icon: "review",
    line: "Find personal details in your chats, posts, sent mail and files, and clean them up.",
    ready: true,
    plans: [],
  },
  {
    key: "vault",
    name: "Vault",
    icon: "key",
    line: "Your passwords in the KeePass format, typed in for you with one shortcut. Opens and saves KeePassXC files.",
    ready: true,
    plans: [],
  },

];

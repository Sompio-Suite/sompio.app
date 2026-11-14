import type { AppKey, AppStatus, LicenceStatus } from "./api";
import { date } from "./format";

const DAY = 86_400_000;
/** Where keys are bought. */
export const BUY_URL = "https://sompio.org/pricing.php";
/** A forever key holds this many days. */
export const FOREVER_DAYS = 65535;
/** Access that ends more than 100 years from now came from a forever key. */
const isForever = (endsAt: number, now: number) => endsAt - now > 36_500 * DAY;
/** The reminder before a paid pass runs out shows from this many days before. */
export const REMIND_DAYS = 7;

/** "Forever", "31 days" */
export const lengthText = (days: number) => (days >= FOREVER_DAYS ? "Forever" : `${days} ${days === 1 ? "day" : "days"}`);

/**
 * A paid pass that runs out within REMIND_DAYS: the earliest end among the unlocked apps (the
 * free trial has its own notice). Null when nothing is about to end.
 */
export function passEnding(l: LicenceStatus | null): { endsAt: number; daysLeft: number } | null {
  if (!l) return null;
  const ends = l.apps.filter((a) => a.active && !a.trial_only && a.ends_at != null).map((a) => a.ends_at as number);
  if (ends.length === 0) return null;
  const endsAt = Math.min(...ends);
  const daysLeft = Math.max(1, Math.ceil((endsAt - l.now) / DAY));
  return daysLeft <= REMIND_DAYS ? { endsAt, daysLeft } : null;
}

export type Tone = "ok" | "trial" | "ended" | "none";

/** "Trial, 12 days left", "Until 9 Dec 2026", "Ended 3 Nov 2026", "Not unlocked" */
export function accessText(a: AppStatus | undefined, now: number): { text: string; tone: Tone } {
  if (!a || a.ends_at == null) return { text: "Not unlocked", tone: "none" };
  if (!a.active) return { text: `Ended ${date(a.ends_at)}`, tone: "ended" };
  if (isForever(a.ends_at, now)) return { text: "Forever", tone: "ok" };
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
    key: "vault",
    name: "Vault",
    icon: "key",
    line: "Your passwords, filled in for you in your browser and apps. Warns you when one is weak, reused, in a data breach, or shared in your own chats.",
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
    key: "calendar",
    name: "Calendar",
    icon: "calendar",
    line: "Your reminders, typed in one line, popping up on time. Shares schedules with your team or guild in each person's own time zone.",
    ready: true,
    plans: [],
  },
];

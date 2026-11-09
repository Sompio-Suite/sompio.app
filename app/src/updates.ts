import { inDesktopApp } from "./api";

/** A newer, signed Sompio that can be installed. */
export interface UpdateInfo {
  version: string;
  notes: string;
  install: (progress: (percent: number | null) => void) => Promise<void>;
}

const LAST_CHECK = "sompio.updates.lastCheck";
const AUTO = "sompio.updates.auto";

export function autoCheckEnabled(): boolean {
  try {
    return localStorage.getItem(AUTO) !== "0";
  } catch {
    return true;
  }
}

export function setAutoCheck(on: boolean) {
  try {
    localStorage.setItem(AUTO, on ? "1" : "0");
  } catch {
    /* not remembered */
  }
}

export async function currentVersion(): Promise<string> {
  if (!inDesktopApp) return "0.1.0";
  const { getVersion } = await import("@tauri-apps/api/app");
  return getVersion();
}

/**
 * Asks GitHub whether there's a newer version. Only a small version file is fetched; nothing about you
 * is sent beyond what any download sends (your IP address). Returns null when up to date.
 */
export async function checkForUpdate(): Promise<UpdateInfo | null> {
  try {
    localStorage.setItem(LAST_CHECK, String(Date.now()));
  } catch {
    /* fine */
  }
  if (!inDesktopApp) {
    // Browser preview: pretend a new version exists so the screens can be seen.
    return {
      version: "0.2.0",
      notes: "Calendar preview, faster scans.",
      install: async (progress) => {
        for (let p = 0; p <= 100; p += 20) {
          progress(p);
          await new Promise((r) => setTimeout(r, 150));
        }
      },
    };
  }
  const { check } = await import("@tauri-apps/plugin-updater");
  const update = await check();
  if (!update) return null;
  return {
    version: update.version,
    notes: update.body ?? "",
    install: async (progress) => {
      let total = 0;
      let done = 0;
      await update.downloadAndInstall((e) => {
        if (e.event === "Started") total = e.data.contentLength ?? 0;
        else if (e.event === "Progress") {
          done += e.data.chunkLength;
          progress(total ? Math.round((done / total) * 100) : null);
        } else if (e.event === "Finished") progress(100);
      });
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    },
  };
}

/** True if the last automatic check was long enough ago. */
export function dueForCheck(hours = 6): boolean {
  try {
    const last = Number(localStorage.getItem(LAST_CHECK) || 0);
    return Date.now() - last > hours * 3_600_000;
  } catch {
    return true;
  }
}

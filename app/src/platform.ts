// Which system Sompio runs on. A few features are Windows-only for now (auto-type, reading text
// in pictures); on a Mac or Linux they're hidden or explained instead.
import { inDesktopApp } from "./api";

export type OS = "windows" | "macos" | "linux";

let os: OS = "windows";
if (inDesktopApp) {
  import("@tauri-apps/api/core")
    .then(({ invoke }) => invoke<string>("app_platform"))
    .then((p) => { if (p === "macos" || p === "linux") os = p; })
    .catch(() => {});
} else if (typeof navigator !== "undefined") {
  // The preview in a browser: follow the computer it's opened on
  const ua = navigator.userAgent;
  if (/Mac OS X|Macintosh/.test(ua)) os = "macos";
  else if (/Linux/.test(ua) && !/Android/.test(ua)) os = "linux";
}

export const platform = () => os;
export const isWindows = () => os === "windows";
/** "PC" on Windows and Linux, "Mac" on a Mac. */
export const computer = () => (os === "macos" ? "Mac" : "PC");

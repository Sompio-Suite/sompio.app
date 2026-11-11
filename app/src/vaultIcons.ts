import { useSyncExternalStore } from "react";
import { api } from "./api";

/** Entry icons (downloaded favicons), shared by every list that shows entries. */
let icons: Record<string, string> = {};
const listeners = new Set<() => void>();

export async function reloadIcons() {
  try {
    icons = await api.vaultIcons();
  } catch {
    icons = {};
  }
  listeners.forEach((l) => l());
}

export function useIcon(id: string | null | undefined): string | null {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => (id ? icons[id] ?? null : null),
  );
}

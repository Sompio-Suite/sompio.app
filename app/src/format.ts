const nf = new Intl.NumberFormat();
const df = new Intl.DateTimeFormat(undefined, { year: "numeric", month: "short", day: "numeric" });
const dtf = new Intl.DateTimeFormat(undefined, { year: "numeric", month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
const mf = new Intl.DateTimeFormat(undefined, { year: "numeric", month: "long" });

export const n = (x: number) => nf.format(x);
export const date = (ms: number | null) => (ms == null ? "" : df.format(ms));
export const dateTime = (ms: number | null) => (ms == null ? "" : dtf.format(ms));
export const month = (ms: number | null) => (ms == null ? "" : mf.format(ms));
export const plural = (count: number, one: string, many: string) => `${n(count)} ${count === 1 ? one : many}`;

export const channelKind: Record<string, string> = {
  dm: "Direct message",
  group_dm: "Group chat",
  server: "Server",
  thread: "Thread",
  community: "Community",
  posts: "Your posts",
  channel: "Channel",
  saved: "Saved messages",
  folder: "Folder",
  unknown: "Other",
};

/** The file name at the end of a Discord attachment URL. */
export const fileName = (url: string) => decodeURIComponent(url.split("?")[0].split("/").pop() || url);

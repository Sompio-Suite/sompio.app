// Sompio Vault for the browser: the background part. It is the only part that talks to Sompio,
// through the browser's native messaging (the browser starts Sompio's small helper, which passes
// messages to the Sompio app on this computer). Nothing here goes to the internet.
//
// After pairing, every request is encrypted with a key only this browser and your vault know
// (AES-256-GCM). Pages never see that key, and only ever get the one login you click, for their
// own website.

const ext = globalThis.browser ?? globalThis.chrome;
const HOST = "app.sompio.browser";

// ---------- Talking to Sompio ----------

let port = null;
let nextRid = 1;
const waiting = new Map();

function connect() {
  port = ext.runtime.connectNative(HOST);
  port.onMessage.addListener((m) => {
    const done = waiting.get(m.rid);
    if (done) {
      waiting.delete(m.rid);
      done(m);
    }
  });
  port.onDisconnect.addListener((p) => {
    const why = (p && p.error && p.error.message) || (ext.runtime.lastError && ext.runtime.lastError.message) || "";
    port = null;
    const code = /not found|no such native/i.test(why) ? "no_host" : "not_running";
    for (const done of waiting.values()) done({ type: "error", code, error: why });
    waiting.clear();
  });
}

function send(msg, timeoutMs = 15000) {
  return new Promise((resolve) => {
    try {
      if (!port) connect();
    } catch (e) {
      return resolve({ type: "error", code: "no_host", error: String(e) });
    }
    const rid = nextRid++;
    const timer = setTimeout(() => {
      waiting.delete(rid);
      resolve({ type: "error", code: "timeout", error: "Sompio didn't answer." });
    }, timeoutMs);
    waiting.set(rid, (m) => {
      clearTimeout(timer);
      resolve(m);
    });
    try {
      port.postMessage({ ...msg, rid });
    } catch (e) {
      waiting.delete(rid);
      clearTimeout(timer);
      port = null;
      resolve({ type: "error", code: "not_running", error: String(e) });
    }
  });
}

// ---------- Encryption ----------

const enc = new TextEncoder();
const dec = new TextDecoder();
const b64 = (bytes) => btoa(String.fromCharCode(...new Uint8Array(bytes)));
const unb64 = (s) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
const hex = (bytes) => [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");

async function sealFor(keyB64, aad, text) {
  const key = await crypto.subtle.importKey("raw", unb64(keyB64), "AES-GCM", false, ["encrypt"]);
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const data = await crypto.subtle.encrypt({ name: "AES-GCM", iv, additionalData: enc.encode(aad) }, key, enc.encode(text));
  return { iv: b64(iv), data: b64(data) };
}

async function openFrom(keyB64, aad, iv, data) {
  const key = await crypto.subtle.importKey("raw", unb64(keyB64), "AES-GCM", false, ["decrypt"]);
  const plain = await crypto.subtle.decrypt({ name: "AES-GCM", iv: unb64(iv), additionalData: enc.encode(aad) }, key, unb64(data));
  return dec.decode(plain);
}

/** The short code Sompio shows too, so you can check it's the same request. */
async function pairCode(keyB64) {
  const ALPHABET = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
  const d = new Uint8Array(await crypto.subtle.digest("SHA-256", enc.encode(keyB64)));
  const s = [...d.slice(0, 6)].map((b) => ALPHABET[b % ALPHABET.length]).join("");
  return `${s.slice(0, 3)}-${s.slice(3)}`;
}

// ---------- Pairing ----------

const getPairing = async () => (await ext.storage.local.get("pairing")).pairing ?? null;

function browserName() {
  const ua = navigator.userAgent;
  if (/Edg\//.test(ua)) return "Edge";
  if (/Firefox\//.test(ua)) return "Firefox";
  if (/OPR\//.test(ua)) return "Opera";
  if (/Vivaldi/.test(ua)) return "Vivaldi";
  if (navigator.brave) return "Brave";
  return "Chrome";
}

let pairing = null; // { code, promise } while waiting for Sompio

async function beginPairing() {
  if (pairing) return { code: pairing.code };
  const id = hex(crypto.getRandomValues(new Uint8Array(16)));
  const key = b64(crypto.getRandomValues(new Uint8Array(32)));
  const code = await pairCode(key);
  const promise = send({ type: "pair", id, key, browser: browserName() }, 130000).then(async (r) => {
    pairing = null;
    if (r.type === "paired" && r.ok) {
      await ext.storage.local.set({ pairing: { id, key } });
      return { ok: true };
    }
    return { ok: false, code: r.code, error: r.error || "Pairing didn't work." };
  });
  pairing = { code, promise };
  promise.then((result) => ext.runtime.sendMessage({ type: "pairDone", ...result }).catch(() => {}));
  return { code };
}

/** An encrypted request to the vault. Returns { ok, r } or { ok: false, code, error }. */
async function call(method, args = {}) {
  const p = await getPairing();
  if (!p) return { ok: false, code: "not_paired", error: "Connect this browser to Sompio first." };
  const n = hex(crypto.getRandomValues(new Uint8Array(16)));
  const sealed = await sealFor(p.key, `sompio-req:${p.id}`, JSON.stringify({ n, t: Date.now(), m: method, a: args }));
  const r = await send({ type: "call", id: p.id, ...sealed });
  if (r.type === "error") {
    if (r.code === "not_paired") await ext.storage.local.remove("pairing");
    return { ok: false, code: r.code, error: r.error };
  }
  if (r.type !== "reply") return { ok: false, code: "bad", error: "Unexpected answer from Sompio." };
  let body;
  try {
    body = JSON.parse(await openFrom(p.key, `sompio-res:${p.id}`, r.iv, r.data));
  } catch {
    return { ok: false, code: "bad", error: "Sompio's answer couldn't be checked." };
  }
  if (body.n !== n) return { ok: false, code: "bad", error: "Sompio's answer didn't match the question." };
  return body.ok ? { ok: true, r: body.r } : { ok: false, code: "failed", error: body.error };
}

async function state() {
  const s = await send({ type: "status" }, 5000);
  const paired = !!(await getPairing());
  if (s.type === "error") return { running: false, host: s.code !== "no_host", paired, waiting: pairing?.code ?? null };
  return { running: !!s.running, host: true, enabled: !!s.enabled, locked: !!s.locked, paired, version: s.version, waiting: pairing?.code ?? null };
}

// ---------- Logins seen being submitted, to offer saving ----------

const hostOf = (url) => {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return "";
  }
};
const siteOf = (host) => host.split(".").slice(-2).join(".");

async function pendingAll() {
  return (await ext.storage.session?.get?.("pending"))?.pending ?? memoryPending;
}
let memoryPending = {};
async function setPending(all) {
  memoryPending = all;
  try {
    await ext.storage.session.set({ pending: all });
  } catch {
    /* older browsers: memory only */
  }
}

async function captured(sender, { username, password, title }) {
  const url = sender.url || sender.tab?.url || "";
  if (!password || !/^https?:/.test(url)) return { ok: true };
  const prefs = (await ext.storage.local.get("prefs")).prefs ?? {};
  if (prefs.offerSave === false || (prefs.never ?? []).includes(hostOf(url))) return { ok: true };
  const logins = await call("logins", { url });
  if (!logins.ok) return { ok: true };
  const same = logins.r.filter((l) => l.username === username);
  let mode = "new";
  let id = null;
  for (const l of same.length ? same : logins.r.filter((l) => !l.username && !username)) {
    const f = await call("fill", { id: l.id, url });
    if (f.ok && f.r.password === password) return { ok: true }; // already saved
    mode = "update";
    id = l.id;
  }
  const all = await pendingAll();
  all[sender.tab?.id ?? "x"] = { url, host: hostOf(url), username, password, title, mode, id, at: Date.now() };
  await setPending(all);
  return { ok: true };
}

// ---------- Messages from pages and the popup ----------

async function handle(msg, sender) {
  const fromPage = !!sender.tab && sender.id === ext.runtime.id && !sender.url?.startsWith(ext.runtime.getURL(""));
  const pageUrl = sender.url || sender.tab?.url || "";
  switch (msg.type) {
    case "state":
      return state();
    case "show":
      return send({ type: "show" });
    case "pairBegin":
      return beginPairing();
    case "forget":
      await ext.storage.local.remove("pairing");
      return { ok: true };

    // From a page: only ever about that page's own website
    case "logins":
      return call("logins", { url: pageUrl });
    case "fill":
      return call("fill", { id: msg.id, url: pageUrl });
    case "totp":
      return call("totp", { id: msg.id, url: pageUrl });
    case "generate":
      return call("generate");
    case "captured":
      return captured(sender, msg);
    case "pendingSave": {
      const all = await pendingAll();
      const p = all[sender.tab?.id ?? "x"];
      if (!p || Date.now() - p.at > 180000 || siteOf(p.host) !== siteOf(hostOf(pageUrl))) return null;
      return { host: p.host, username: p.username, mode: p.mode };
    }
    case "answerSave": {
      const all = await pendingAll();
      const key = sender.tab?.id ?? "x";
      const p = all[key];
      delete all[key];
      await setPending(all);
      if (!p) return { ok: false };
      if (msg.answer === "never") {
        const prefs = (await ext.storage.local.get("prefs")).prefs ?? {};
        prefs.never = [...new Set([...(prefs.never ?? []), p.host])];
        await ext.storage.local.set({ prefs });
        return { ok: true };
      }
      if (msg.answer !== "save" && msg.answer !== "update") return { ok: true };
      return call("save", { url: p.url, title: p.title || p.host, username: p.username, password: p.password, id: msg.answer === "update" ? p.id : "" });
    }

    // From the popup (an extension page, not a website)
    case "popupLogins":
      if (fromPage) return null;
      return call("logins", { url: msg.url });
    case "popupSearch":
      if (fromPage) return null;
      return call("search", { q: msg.q });
    case "popupFill": {
      if (fromPage) return null;
      const f = await call("fill", { id: msg.id, url: msg.url });
      if (!f.ok) return f;
      try {
        await ext.tabs.sendMessage(msg.tabId, { type: "doFill", creds: f.r }, { frameId: 0 });
      } catch {
        return { ok: false, error: "This page can't be filled. Reload it and try again." };
      }
      return { ok: true };
    }
    case "popupSecret":
      if (fromPage) return null;
      return call("fill", { id: msg.id, url: "", anySite: true });
    default:
      return null;
  }
}

ext.runtime.onMessage.addListener((msg, sender, sendResponse) => {
  if (sender.id !== ext.runtime.id) return false;
  handle(msg, sender).then(sendResponse, (e) => sendResponse({ ok: false, error: String(e) }));
  return true;
});

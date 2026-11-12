// The toolbar popup: connection state, logins for this website, search, and a password generator.

const ext = globalThis.browser ?? globalThis.chrome;
const ask = (msg) => ext.runtime.sendMessage(msg).catch((e) => ({ ok: false, error: String(e) }));
const main = document.getElementById("main");
const status = document.getElementById("status");
let tab = null;
let clearTimer = null;

const h = (tag, attrs = {}, ...kids) => {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k.startsWith("on")) e.addEventListener(k.slice(2), v);
    else if (k === "class") e.className = v;
    else e.setAttribute(k, v);
  }
  for (const k of kids.flat()) if (k != null) e.append(k);
  return e;
};

function toast(t) {
  document.querySelector(".toast")?.remove();
  const el = h("div", { class: "toast", role: "status" }, t);
  document.body.append(el);
  setTimeout(() => el.remove(), 2600);
}

async function copy(textValue, what, secret) {
  await navigator.clipboard.writeText(textValue);
  toast(secret ? `${what} copied. It clears in 20 seconds while this stays open.` : `${what} copied.`);
  if (secret) {
    clearTimeout(clearTimer);
    clearTimer = setTimeout(() => navigator.clipboard.writeText("").catch(() => {}), 20000);
  }
}

function setStatus(text, kind) {
  status.textContent = text;
  status.className = `pill ${kind ?? ""}`;
}

function avatar(l) {
  const a = h("span", { class: "avatar" });
  if (typeof l.icon === "string" && /^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(l.icon)) a.append(h("img", { src: l.icon, alt: "" }));
  else a.textContent = (l.title || "?").trim().charAt(0).toUpperCase();
  return a;
}

function loginRow(l, onSite) {
  const actions = [];
  if (onSite) {
    actions.push(h("button", { class: "plain", onclick: async () => {
      const r = await ask({ type: "popupFill", id: l.id, url: tab.url, tabId: tab.id });
      if (r?.ok) window.close();
      else toast(r?.error ?? "Couldn't fill this page.");
    } }, "Fill"));
  }
  actions.push(h("button", { class: "icon-btn", title: "Copy username", onclick: () => l.username && copy(l.username, "Username") }, "User"));
  actions.push(h("button", { class: "icon-btn", title: "Copy password", onclick: async () => {
    const r = await ask({ type: "popupSecret", id: l.id });
    if (r?.ok) copy(r.r.password, "Password", true);
    else toast(r?.error ?? "Couldn't get the password.");
  } }, "Pass"));
  return h("div", { class: "login" }, avatar(l), h("span", { class: "who" }, h("b", {}, l.title || l.url), h("span", {}, l.username || "No username")), ...actions);
}

function screen(...kids) {
  main.replaceChildren(...kids);
}

async function render() {
  [tab] = await ext.tabs.query({ active: true, currentWindow: true });
  const s = await ask({ type: "state" });
  if (!s || !s.host) {
    setStatus("Not set up", "warn");
    return screen(
      h("p", {}, h("b", {}, "Sompio isn't set up for this browser yet.")),
      h("ol", { class: "steps" },
        h("li", {}, "Open Sompio and unlock it."),
        h("li", {}, "Go to Vault, Settings, and turn on ", h("b", {}, "Browser extension"), "."),
        h("li", {}, "Restart this browser, then click this button again.")),
    );
  }
  if (!s.running) {
    setStatus("Not running", "warn");
    return screen(h("p", {}, h("b", {}, "Sompio isn't running.")), h("p", { class: "muted" }, "Start Sompio from the Start menu or the tray, then try again."));
  }
  if (s.enabled === false) {
    setStatus("Turned off", "warn");
    return screen(h("p", {}, "The browser extension is turned off in Sompio. Turn it on in ", h("b", {}, "Vault, Settings"), "."));
  }
  if (s.locked) {
    setStatus("Locked", "warn");
    return screen(
      h("p", {}, h("b", {}, "Sompio is locked.")),
      h("p", { class: "muted" }, "Unlock it to fill your logins."),
      h("button", { class: "primary", onclick: () => ask({ type: "show" }).then(() => window.close()) }, "Unlock Sompio"),
    );
  }
  if (!s.paired) return pairScreen(s.waiting);

  setStatus("Connected", "ok");
  const onWeb = tab && /^https?:/.test(tab.url ?? "");
  const host = onWeb ? new URL(tab.url).hostname.replace(/^www\./, "") : "";
  const here = h("div", { class: "list" }, h("p", { class: "muted" }, "Loading…"));
  const results = h("div", { class: "list" });
  const search = h("input", { type: "search", placeholder: "Search your vault", "aria-label": "Search your vault" });
  let t = null;
  search.addEventListener("input", () => {
    clearTimeout(t);
    t = setTimeout(async () => {
      const q = search.value.trim();
      if (q.length < 2) return results.replaceChildren();
      const r = await ask({ type: "popupSearch", q });
      results.replaceChildren(...(r?.ok ? (r.r.length ? r.r.map((l) => loginRow(l, false)) : [h("p", { class: "muted" }, "Nothing found.")]) : [h("p", { class: "err" }, r?.error ?? "Search didn't work.")]));
    }, 200);
  });
  const genOut = h("code", {}, "");
  const genBox = h("div", { class: "gen" }, genOut, h("button", { class: "icon-btn", onclick: () => genOut.textContent && copy(genOut.textContent, "Password", true) }, "Copy"));
  genBox.hidden = true;
  screen(
    h("h2", {}, onWeb ? `Logins for ${host}` : "This page"),
    here,
    search,
    results,
    h("div", { class: "box" },
      h("button", { class: "plain", onclick: async () => {
        const r = await ask({ type: "generate" });
        if (r?.ok) {
          genOut.textContent = r.r.password;
          genBox.hidden = false;
        }
      } }, "Generate a strong password"),
      genBox),
  );
  if (!onWeb) return here.replaceChildren(h("p", { class: "muted" }, "Open a website to see its logins."));
  const r = await ask({ type: "popupLogins", url: tab.url });
  if (!r?.ok) return here.replaceChildren(h("p", { class: "err" }, r?.error ?? "Couldn't load logins."));
  here.replaceChildren(...(r.r.length ? r.r.map((l) => loginRow(l, true)) : [h("p", { class: "muted" }, "No logins saved for this website. Sign in, and Sompio offers to save it.")]));
}

function pairScreen(waitingCode) {
  setStatus("Not connected", "warn");
  const go = h("button", { class: "primary" }, "Connect to Sompio");
  const box = h("div", { class: "box" },
    h("p", {}, h("b", {}, "Connect this browser to your Vault")),
    h("p", { class: "muted" }, "Sompio asks you to allow it. You only do this once per browser."),
    go);
  const waitFor = (code) => {
    box.replaceChildren(
      h("p", {}, "In Sompio, check the code matches, then click ", h("b", {}, "Allow"), "."),
      h("div", { class: "code", "aria-label": "Pairing code" }, code),
      h("p", { class: "muted" }, "Waiting for Sompio…"),
    );
  };
  go.addEventListener("click", async () => {
    go.disabled = true;
    const r = await ask({ type: "pairBegin" });
    if (r?.code) waitFor(r.code);
  });
  if (waitingCode) waitFor(waitingCode);
  screen(box);
}

ext.runtime.onMessage.addListener((msg) => {
  if (msg?.type !== "pairDone") return;
  if (msg.ok) render();
  else {
    pairScreen(null);
    main.prepend(h("p", { class: "err" }, msg.error ?? "Pairing didn't work."));
  }
});

document.getElementById("settings").addEventListener("click", async () => {
  const prefs = { icons: true, offerSave: true, never: [], ...((await ext.storage.local.get("prefs")).prefs ?? {}) };
  const save = () => ext.storage.local.set({ prefs });
  const check = (key, label) => {
    const box = h("input", { type: "checkbox" });
    box.checked = prefs[key] !== false;
    box.addEventListener("change", () => {
      prefs[key] = box.checked;
      save();
    });
    return h("label", { class: "check" }, box, h("span", {}, label));
  };
  const paired = (await ask({ type: "state" }))?.paired;
  screen(
    h("h2", {}, "Settings"),
    check("icons", "Show the Sompio button in login fields"),
    check("offerSave", "Offer to save logins after you sign in"),
    prefs.never.length
      ? h("div", {}, h("p", { class: "muted" }, `Never offer to save on: ${prefs.never.join(", ")}`),
          h("button", { class: "link", onclick: () => { prefs.never = []; save(); toast("Cleared."); } }, "Clear this list"))
      : null,
    paired ? h("button", { class: "plain", onclick: async () => { await ask({ type: "forget" }); toast("Disconnected. Remove it in Sompio too."); render(); } }, "Disconnect this browser") : null,
    h("button", { class: "link", onclick: render }, "Back"),
  );
});

render();

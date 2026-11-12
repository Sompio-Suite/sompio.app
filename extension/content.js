// Sompio Vault on web pages: a small Sompio button in login fields, a list of the logins saved
// for this website, and an offer to save a login after you sign in. The page itself can't see
// any of it (it lives in a closed shadow root), and gets a password only when you click a login.

(() => {
  if (window.__sompioVault) return;
  window.__sompioVault = true;
  const ext = globalThis.browser ?? globalThis.chrome;
  const ask = (msg) => ext.runtime.sendMessage(msg).catch(() => null);

  const GREEN = "#2e7d32";
  const CSS = `
    :host { all: initial; }
    * { box-sizing: border-box; font-family: "Segoe UI", system-ui, -apple-system, sans-serif; }
    .btn { position: fixed; width: 22px; height: 22px; border: 0; padding: 0; margin: 0; border-radius: 5px; background: transparent; cursor: pointer; z-index: 2147483646; display: grid; place-items: center; opacity: .9; }
    .btn:hover { opacity: 1; background: rgba(31,45,58,.08); }
    .btn svg { width: 18px; height: 18px; display: block; }
    .menu { position: fixed; z-index: 2147483647; width: 300px; max-height: 340px; overflow: auto; background: #fff; color: #1d2733; border: 1px solid #c9d1da; border-radius: 8px; box-shadow: 0 10px 30px rgba(20,30,40,.22); font-size: 13px; padding: 4px; }
    .head { display: flex; align-items: center; gap: 6px; padding: 6px 8px 5px; font-size: 11px; font-weight: 600; color: #5b6774; text-transform: uppercase; letter-spacing: .03em; }
    .head svg { width: 14px; height: 14px; }
    .item { display: flex; align-items: center; gap: 9px; width: 100%; text-align: left; border: 0; background: none; padding: 7px 8px; border-radius: 6px; cursor: pointer; color: inherit; font-size: 13px; }
    .item:hover, .item:focus { background: #eef5fc; outline: none; }
    .avatar { flex: none; width: 28px; height: 28px; border-radius: 6px; background: #e8edf2; display: grid; place-items: center; font-weight: 700; color: #1f2d3a; overflow: hidden; }
    .avatar img { width: 20px; height: 20px; }
    .text { min-width: 0; display: grid; }
    .text b { font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .text span { color: #5b6774; font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .note { padding: 8px 10px 10px; color: #3c4854; line-height: 1.45; }
    .note button { margin-top: 8px; }
    .primary { border: 0; background: ${GREEN}; color: #fff; border-radius: 6px; padding: 6px 12px; font-weight: 600; font-size: 13px; cursor: pointer; }
    .plain { border: 1px solid #c9d1da; background: #fff; color: #1d2733; border-radius: 6px; padding: 6px 12px; font-size: 13px; cursor: pointer; }
    .sep { height: 1px; background: #e3e8ed; margin: 4px 2px; }
    .gen b { color: ${GREEN}; }
    .banner { position: fixed; top: 16px; right: 16px; z-index: 2147483647; width: 340px; background: #fff; color: #1d2733; border: 1px solid #c9d1da; border-radius: 10px; box-shadow: 0 12px 34px rgba(20,30,40,.25); padding: 14px 16px; font-size: 13px; line-height: 1.45; }
    .banner .title { display: flex; align-items: center; gap: 8px; font-weight: 700; font-size: 14px; margin-bottom: 4px; }
    .banner .title svg { width: 20px; height: 20px; }
    .banner .row { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 12px; }
    .banner .x { margin-left: auto; border: 0; background: none; font-size: 18px; line-height: 1; cursor: pointer; color: #5b6774; }
    .muted { color: #5b6774; }
  `;
  const LOGO = `<svg viewBox="0 0 120 120" aria-hidden="true"><rect width="120" height="120" rx="26" fill="#1f2d3a"/><path d="M22 58 A30 30 0 0 1 82 58 Z" fill="#fff"/><path d="M38 64 A30 30 0 0 0 98 64 Z" fill="#4caf50"/></svg>`;

  const hostEl = document.createElement("sompio-vault");
  const root = hostEl.attachShadow({ mode: "closed" });
  const style = document.createElement("style");
  style.textContent = CSS;
  root.appendChild(style);
  const attach = () => !hostEl.isConnected && document.documentElement.appendChild(hostEl);

  const el = (tag, cls, html) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (html != null) e.innerHTML = html;
    return e;
  };
  const text = (tag, cls, t) => {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    e.textContent = t;
    return e;
  };

  // ---------- Finding the fields ----------

  const visible = (i) => {
    if (i.disabled || i.readOnly) return false;
    const r = i.getBoundingClientRect();
    if (r.width < 40 || r.height < 14) return false;
    const s = getComputedStyle(i);
    return s.visibility !== "hidden" && s.display !== "none" && Number(s.opacity) > 0.05;
  };
  const TEXTY = new Set(["text", "email", "tel", ""]);
  const isOtp = (i) =>
    i.autocomplete === "one-time-code" ||
    (/(otp|totp|2fa|mfa|one.?time|verification.?code|auth.?code|security.?code)/i.test(`${i.name} ${i.id} ${i.placeholder} ${i.getAttribute("aria-label") ?? ""}`) &&
      (!i.maxLength || i.maxLength <= 10 || i.maxLength === -1) && i.type !== "password");
  const isNew = (i) => i.autocomplete === "new-password" || /(new|confirm|repeat|create|signup|register)/i.test(`${i.name} ${i.id} ${i.placeholder}`);

  function inputsNear(pw) {
    const scope = pw.form ?? document;
    return [...scope.querySelectorAll("input")].filter((i) => visible(i));
  }
  function usernameFor(pw) {
    const all = inputsNear(pw);
    const idx = all.indexOf(pw);
    for (let k = idx - 1; k >= 0; k--) {
      const i = all[k];
      if (TEXTY.has(i.type) && !isOtp(i)) return i;
    }
    return null;
  }
  function passwordsNear(i) {
    return inputsNear(i).filter((x) => x.type === "password");
  }
  function findUsernameField() {
    const pw = [...document.querySelectorAll("input[type=password]")].find(visible);
    if (pw) return usernameFor(pw);
    // Two-step logins: the email page comes before the password page
    return [...document.querySelectorAll('input[autocomplete~="username"], input[type=email], input[name*="user" i], input[name*="email" i], input[name*="login" i]')].find(visible) ?? null;
  }

  // ---------- Filling ----------

  function setValue(input, value) {
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
    input.focus();
    setter.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));
  }
  function fill(creds, near) {
    const pw = near?.type === "password" ? near : passwordsNear(near ?? document.body).find((p) => !isNew(p)) ?? [...document.querySelectorAll("input[type=password]")].find(visible);
    const user = pw ? usernameFor(pw) : near && TEXTY.has(near.type) ? near : findUsernameField();
    if (user && creds.username) setValue(user, creds.username);
    if (pw && creds.password) setValue(pw, creds.password);
    (pw ?? user)?.focus();
  }

  // ---------- The Sompio button in fields ----------

  const buttons = new Map(); // input -> button
  function decorate(input, kind) {
    if (buttons.has(input)) return;
    attach();
    const b = el("button", "btn", LOGO);
    b.type = "button";
    b.title = "Sompio Vault";
    b.tabIndex = -1;
    b.addEventListener("mousedown", (e) => e.preventDefault());
    b.addEventListener("click", (e) => {
      e.preventDefault();
      e.stopPropagation();
      openMenu(input, kind);
    });
    root.appendChild(b);
    buttons.set(input, b);
    place(input, b);
  }
  function place(input, b) {
    if (!input.isConnected || !visible(input)) {
      b.style.display = "none";
      return;
    }
    const r = input.getBoundingClientRect();
    b.style.display = "grid";
    b.style.top = `${r.top + (r.height - 22) / 2}px`;
    b.style.left = `${r.right - 22 - Math.min(8, r.width * 0.05)}px`;
  }
  let prefs = { icons: true };
  ext.storage.local.get("prefs").then((p) => (prefs = { icons: true, ...(p.prefs ?? {}) }));

  function scan() {
    if (prefs.icons === false) return;
    for (const i of document.querySelectorAll("input")) {
      if (!visible(i)) continue;
      if (i.type === "password") {
        decorate(i, isNew(i) ? "new" : "login");
        const u = usernameFor(i);
        if (u) decorate(u, "login");
      } else if (isOtp(i)) decorate(i, "otp");
    }
    if (![...document.querySelectorAll("input[type=password]")].some(visible)) {
      const u = findUsernameField();
      if (u && (u.autocomplete?.includes("username") || u.type === "email")) decorate(u, "login");
    }
    for (const [i, b] of buttons) {
      if (!i.isConnected) {
        b.remove();
        buttons.delete(i);
      } else place(i, b);
    }
  }
  const reposition = () => {
    for (const [i, b] of buttons) place(i, b);
    if (menu) positionMenu();
  };
  addEventListener("scroll", reposition, true);
  addEventListener("resize", reposition);
  let timer = null;
  new MutationObserver(() => {
    clearTimeout(timer);
    timer = setTimeout(scan, 300);
  }).observe(document.documentElement, { childList: true, subtree: true, attributes: true, attributeFilter: ["type", "style", "class", "hidden"] });
  setTimeout(scan, 200);

  // ---------- The list of logins ----------

  let menu = null;
  let menuFor = null;
  function closeMenu() {
    menu?.remove();
    menu = null;
    menuFor = null;
  }
  function positionMenu() {
    if (!menu || !menuFor) return;
    const r = menuFor.getBoundingClientRect();
    const below = innerHeight - r.bottom > 220 || r.top < 220;
    menu.style.left = `${Math.max(8, Math.min(r.left, innerWidth - 308))}px`;
    menu.style.top = below ? `${r.bottom + 4}px` : "";
    menu.style.bottom = below ? "" : `${innerHeight - r.top + 4}px`;
  }
  addEventListener("mousedown", (e) => {
    if (menu && !e.composedPath().includes(hostEl)) closeMenu();
  }, true);
  addEventListener("keydown", (e) => e.key === "Escape" && closeMenu(), true);

  function stateNote(s, error) {
    if (!s.host) return ["Sompio isn't set up for this browser yet.", "In Sompio, open Vault, Settings, and turn on the browser extension."];
    if (!s.running) return ["Sompio isn't running.", "Start Sompio to fill your logins."];
    if (s.enabled === false) return ["The browser extension is turned off in Sompio.", "Turn it on in Vault, Settings."];
    if (s.locked) return ["Sompio is locked.", null, "Unlock Sompio"];
    if (!s.paired) return ["This browser isn't connected to Sompio yet.", "Click the Sompio button in your browser's toolbar to connect it."];
    return [error || "Something went wrong.", null];
  }

  async function openMenu(input, kind) {
    closeMenu();
    menuFor = input;
    menu = el("div", "menu");
    menu.setAttribute("role", "listbox");
    const head = el("div", "head", `${LOGO}<span>Sompio Vault</span>`);
    menu.appendChild(head);
    const loading = text("div", "note", "Loading…");
    menu.appendChild(loading);
    root.appendChild(menu);
    positionMenu();

    const res = await ask({ type: "logins" });
    if (menuFor !== input) return;
    loading.remove();
    if (!res || !res.ok) {
      const s = (await ask({ type: "state" })) ?? { host: false };
      const [line, more, action] = stateNote(s, res?.error);
      const note = el("div", "note");
      note.appendChild(text("div", "", line));
      if (more) note.appendChild(text("div", "muted", more));
      if (action) {
        const b = text("button", "primary", action);
        b.addEventListener("click", () => {
          ask({ type: "show" });
          closeMenu();
        });
        note.appendChild(b);
      }
      menu.appendChild(note);
      return;
    }
    if (kind === "new") {
      const g = el("button", "item gen", `<span class="avatar">✱</span><span class="text"><b>Use a strong password</b><span>Made by Sompio</span></span>`);
      g.addEventListener("click", async () => {
        const r = await ask({ type: "generate" });
        if (!r?.ok) return;
        for (const p of passwordsNear(input)) setValue(p, r.r.password);
        closeMenu();
      });
      menu.appendChild(g);
      if (res.r.length) menu.appendChild(el("div", "sep"));
    }
    const list = kind === "otp" ? res.r.filter((l) => l.totp) : res.r;
    if (!list.length && kind !== "new") {
      menu.appendChild(text("div", "note", kind === "otp" ? "No 2FA codes saved for this website." : "No logins saved for this website yet. Sign in, and Sompio offers to save it."));
    }
    for (const l of list) {
      const item = el("button", "item");
      item.setAttribute("role", "option");
      const av = el("span", "avatar");
      // Entry icons come from Sompio as small PNG data; anything else shows the first letter
      if (typeof l.icon === "string" && /^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(l.icon)) {
        const img = document.createElement("img");
        img.src = l.icon;
        img.alt = "";
        av.appendChild(img);
      } else av.textContent = (l.title || "?").trim().charAt(0).toUpperCase();
      const t = el("span", "text");
      t.appendChild(text("b", "", l.title || l.url));
      t.appendChild(text("span", "", kind === "otp" ? "Fill the 2FA code" : l.username || "No username"));
      item.append(av, t);
      item.addEventListener("click", async () => {
        if (kind === "otp") {
          const r = await ask({ type: "totp", id: l.id });
          if (r?.ok && r.r) setValue(input, r.r.code);
        } else {
          const r = await ask({ type: "fill", id: l.id });
          if (r?.ok) fill(r.r, input);
        }
        closeMenu();
      });
      menu.appendChild(item);
    }
  }

  // ---------- Offering to save ----------

  let lastGrab = 0;
  function grab(form) {
    if (Date.now() - lastGrab < 1500) return;
    const pws = (form ? [...form.querySelectorAll("input[type=password]")] : [...document.querySelectorAll("input[type=password]")]).filter((p) => p.value);
    if (!pws.length) return;
    const pw = pws.find(isNew) ?? pws[pws.length - 1];
    const user = usernameFor(pws[0]);
    let username = user?.value ?? "";
    if (!username) {
      try {
        username = sessionStorage.getItem("__sompio_user") ?? "";
      } catch {
        /* not available */
      }
    }
    lastGrab = Date.now();
    ask({ type: "captured", username, password: pw.value, title: document.title });
  }
  addEventListener("submit", (e) => grab(e.target), true);
  addEventListener("click", (e) => {
    const b = e.target.closest?.('button, input[type=submit], [role=button]');
    if (b && (b.type === "submit" || /log ?in|sign ?in|continue|next|save|register|sign ?up|create/i.test(b.textContent || b.value || ""))) {
      const u = findUsernameField();
      if (u?.value) {
        try {
          sessionStorage.setItem("__sompio_user", u.value);
        } catch {
          /* not available */
        }
      }
      grab(b.form ?? null);
    }
  }, true);
  addEventListener("keydown", (e) => {
    if (e.key === "Enter" && e.target instanceof HTMLInputElement && (e.target.type === "password" || TEXTY.has(e.target.type))) {
      const u = findUsernameField();
      if (u?.value) {
        try {
          sessionStorage.setItem("__sompio_user", u.value);
        } catch {
          /* not available */
        }
      }
      if (e.target.type === "password") grab(e.target.form);
    }
  }, true);

  async function offerSave() {
    if (window !== window.top) return;
    const p = await ask({ type: "pendingSave" });
    if (!p) return;
    attach();
    const b = el("div", "banner");
    b.setAttribute("role", "dialog");
    const title = el("div", "title", LOGO);
    title.appendChild(text("span", "", p.mode === "update" ? "Update the saved password?" : "Save this login to Sompio?"));
    const x = text("button", "x", "×");
    x.title = "Not now";
    title.appendChild(x);
    b.appendChild(title);
    b.appendChild(text("div", "muted", `${p.host}${p.username ? ` · ${p.username}` : ""}`));
    const row = el("div", "row");
    const done = (answer) => {
      ask({ type: "answerSave", answer });
      b.remove();
    };
    const main = text("button", "primary", p.mode === "update" ? "Update" : "Save");
    main.addEventListener("click", () => done(p.mode === "update" ? "update" : "save"));
    row.appendChild(main);
    if (p.mode === "update") {
      const asNew = text("button", "plain", "Save as new");
      asNew.addEventListener("click", () => done("save"));
      row.appendChild(asNew);
    }
    const later = text("button", "plain", "Not now");
    later.addEventListener("click", () => done("later"));
    const never = text("button", "plain", "Never for this site");
    never.addEventListener("click", () => done("never"));
    row.append(later, never);
    x.addEventListener("click", () => done("later"));
    b.appendChild(row);
    root.appendChild(b);
  }
  setTimeout(offerSave, 600);
  // Pages that sign in without loading a new page
  addEventListener("submit", () => setTimeout(offerSave, 1500), true);

  // ---------- Filling from the toolbar popup ----------

  ext.runtime.onMessage.addListener((msg) => {
    if (msg?.type === "doFill" && window === window.top) fill(msg.creds, null);
  });
})();

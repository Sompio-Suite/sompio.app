//! Sompio Vault in the shell: commands for the UI, copying with auto-clear, and auto-type
//! (a global shortcut that types into the window in front). The vault logic is in sompio-vault.

use crate::{AppState, CmdResult};
use serde::{Deserialize, Serialize};
use sompio_licence::App;
use sompio_vault::autotype::{self, Key as AtKey, Mods, Step};
use sompio_vault::generator::{self, PassphraseRules, PasswordRules, Strength};
use sompio_vault::health::{self, HealthReport};
use sompio_vault::{EntryDetail, EntryInput, EntrySummary, GroupView, ImportSummary, Totp, Vault};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VaultSettings {
    /// The global auto-type shortcut, like "Ctrl+Alt+A". Empty turns it off.
    pub shortcut: String,
    /// Seconds before a copied password is cleared from the clipboard. 0 keeps it.
    pub clear_seconds: u64,
    /// Lock Sompio after auto-typing
    pub lock_after_type: bool,
}

impl Default for VaultSettings {
    fn default() -> Self {
        Self { shortcut: "Ctrl+Alt+A".into(), clear_seconds: 20, lock_after_type: false }
    }
}

pub struct VaultState {
    open: Mutex<Option<Vault>>,
    settings_path: PathBuf,
    settings: Mutex<VaultSettings>,
    /// Bumps with every copy, so an older auto-clear doesn't wipe a newer copy
    copy_serial: AtomicU64,
    /// The window auto-type was asked for, while the chooser is open
    target: Mutex<Option<Target>>,
}

#[derive(Clone)]
struct Target {
    title: String,
    #[cfg_attr(not(windows), allow(dead_code))]
    hwnd: isize,
}

impl VaultState {
    pub fn new(dir: &Path) -> Self {
        let settings_path = dir.join("vault-settings.json");
        let settings = std::fs::read_to_string(&settings_path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        Self { open: Mutex::new(None), settings_path, settings: Mutex::new(settings), copy_serial: AtomicU64::new(0), target: Mutex::new(None) }
    }

    /// Forgets the open vault (and the passwords in memory) when Sompio locks.
    pub fn close(&self) {
        if let Ok(mut g) = self.open.lock() {
            *g = None;
        }
    }
}

pub fn shortcut(s: &VaultState) -> String {
    s.settings.lock().map(|x| x.shortcut.clone()).unwrap_or_default()
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Runs `f` on the open vault, opening it first. Refuses while Sompio is locked.
async fn with_vault<T, F>(app: &AppHandle, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut Vault) -> anyhow::Result<T> + Send + 'static,
{
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let (path, key) = match state.vault.vault_db() {
            Ok(k) => k,
            Err(e) => {
                state.passwords.close();
                return Err(err(e));
            }
        };
        let mut guard = state.passwords.open.lock().map_err(err)?;
        if guard.is_none() {
            *guard = Some(Vault::open(&path, &key).map_err(err)?);
        }
        f(guard.as_mut().expect("opened above")).map_err(err)
    })
    .await
    .map_err(err)?
}

async fn change<T, F>(app: &AppHandle, state: &State<'_, AppState>, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut Vault) -> anyhow::Result<T> + Send + 'static,
{
    state.licences.require(App::Vault)?;
    with_vault(app, f).await
}

#[tauri::command]
pub async fn vault_groups(app: AppHandle) -> CmdResult<Vec<GroupView>> {
    with_vault(&app, |v| Ok(v.groups())).await
}

#[tauri::command]
pub async fn vault_entries(app: AppHandle, group: Option<String>, search: String) -> CmdResult<Vec<EntrySummary>> {
    with_vault(&app, move |v| v.entries(group.as_deref(), &search)).await
}

#[tauri::command]
pub async fn vault_entry(app: AppHandle, id: String) -> CmdResult<EntryDetail> {
    with_vault(&app, move |v| v.entry(&id)).await
}

/// Shows one field on screen (the eye button). `history` reads an older version.
#[tauri::command]
pub async fn vault_reveal(app: AppHandle, id: String, field: String, history: Option<usize>) -> CmdResult<String> {
    with_vault(&app, move |v| {
        let value = match history {
            Some(i) => v.reveal_history(&id, i, &field)?,
            None => v.reveal(&id, &field)?,
        };
        Ok(value.to_string())
    })
    .await
}

#[tauri::command]
pub async fn vault_totp(app: AppHandle, id: String) -> CmdResult<Option<Totp>> {
    with_vault(&app, move |v| v.totp(&id)).await
}

/// Puts text on the clipboard, out of Windows' clipboard history and cloud sync, and clears it
/// after a while if it's still there.
fn copy_secret(app: &AppHandle, text: Zeroizing<String>) -> CmdResult<u64> {
    let state = app.state::<AppState>();
    let seconds = state.passwords.settings.lock().map_err(err)?.clear_seconds;
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("The clipboard isn't available: {e}"))?;
    #[cfg(windows)]
    {
        use arboard::SetExtWindows;
        cb.set().exclude_from_history().exclude_from_cloud().exclude_from_monitoring().text(text.as_str()).map_err(err)?;
    }
    #[cfg(not(windows))]
    cb.set_text(text.as_str()).map_err(err)?;
    let serial = state.passwords.copy_serial.fetch_add(1, Ordering::SeqCst) + 1;
    if seconds > 0 {
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(seconds));
            let state = app.state::<AppState>();
            if state.passwords.copy_serial.load(Ordering::SeqCst) != serial {
                return;
            }
            if let Ok(mut cb) = arboard::Clipboard::new() {
                if cb.get_text().map(|t| t == *text).unwrap_or(false) {
                    let _ = cb.clear();
                }
            }
            let _ = app.emit("vault-clipboard-cleared", ());
        });
    }
    Ok(seconds)
}

/// Copies a field ("Password", "UserName", "URL", "TOTP" for the current code, or your own field).
#[tauri::command]
pub async fn vault_copy(app: AppHandle, id: String, field: String) -> CmdResult<u64> {
    let value = with_vault(&app, move |v| {
        if field == "TOTP" {
            return Ok(Zeroizing::new(v.totp(&id)?.map(|t| t.code).ok_or_else(|| anyhow::anyhow!("This entry has no 2FA set up."))?));
        }
        v.reveal(&id, &field)
    })
    .await?;
    copy_secret(&app, value)
}

/// Copies a generated password that isn't saved anywhere yet.
#[tauri::command]
pub fn vault_copy_text(app: AppHandle, text: String) -> CmdResult<u64> {
    copy_secret(&app, Zeroizing::new(text))
}

#[tauri::command]
pub async fn vault_add_entry(app: AppHandle, state: State<'_, AppState>, entry: EntryInput) -> CmdResult<String> {
    change(&app, &state, move |v| v.add_entry(&entry)).await
}

#[tauri::command]
pub async fn vault_update_entry(app: AppHandle, state: State<'_, AppState>, id: String, entry: EntryInput) -> CmdResult<()> {
    change(&app, &state, move |v| v.update_entry(&id, &entry)).await
}

/// Returns true when it was removed for good (it was already in the recycle bin).
#[tauri::command]
pub async fn vault_delete_entry(app: AppHandle, id: String) -> CmdResult<bool> {
    // Deleting is allowed without a licence: your data, your call.
    with_vault(&app, move |v| v.delete_entry(&id)).await
}

#[tauri::command]
pub async fn vault_restore_entry(app: AppHandle, id: String) -> CmdResult<()> {
    with_vault(&app, move |v| v.restore_entry(&id)).await
}

#[tauri::command]
pub async fn vault_restore_version(app: AppHandle, state: State<'_, AppState>, id: String, index: usize) -> CmdResult<()> {
    change(&app, &state, move |v| v.restore_version(&id, index)).await
}

#[tauri::command]
pub async fn vault_move_entry(app: AppHandle, state: State<'_, AppState>, id: String, group: String) -> CmdResult<()> {
    change(&app, &state, move |v| v.move_entry(&id, &group)).await
}

#[tauri::command]
pub async fn vault_empty_bin(app: AppHandle) -> CmdResult<usize> {
    with_vault(&app, |v| v.empty_recycle_bin()).await
}

#[tauri::command]
pub async fn vault_add_group(app: AppHandle, state: State<'_, AppState>, parent: Option<String>, name: String) -> CmdResult<String> {
    change(&app, &state, move |v| v.add_group(parent.as_deref(), &name)).await
}

#[tauri::command]
pub async fn vault_rename_group(app: AppHandle, state: State<'_, AppState>, id: String, name: String) -> CmdResult<()> {
    change(&app, &state, move |v| v.rename_group(&id, &name)).await
}

#[tauri::command]
pub async fn vault_move_group(app: AppHandle, state: State<'_, AppState>, id: String, parent: String) -> CmdResult<()> {
    change(&app, &state, move |v| v.move_group(&id, &parent)).await
}

#[tauri::command]
pub async fn vault_delete_group(app: AppHandle, id: String) -> CmdResult<()> {
    with_vault(&app, move |v| v.delete_group(&id)).await
}

#[derive(Serialize)]
pub struct Generated {
    value: String,
    strength: Strength,
}

/// kind: "password" or "passphrase"
#[tauri::command]
pub fn vault_generate(kind: String, password: Option<PasswordRules>, passphrase: Option<PassphraseRules>) -> CmdResult<Generated> {
    let value = if kind == "passphrase" {
        generator::passphrase(&passphrase.unwrap_or_default()).map_err(err)?
    } else {
        generator::password(&password.unwrap_or_default()).map_err(err)?
    };
    let strength = generator::strength(&value);
    Ok(Generated { value, strength })
}

#[tauri::command]
pub async fn vault_strength(password: String) -> CmdResult<Strength> {
    tauri::async_runtime::spawn_blocking(move || generator::strength(&password)).await.map_err(err)
}

#[tauri::command]
pub async fn vault_health(app: AppHandle) -> CmdResult<HealthReport> {
    with_vault(&app, |v| Ok(health::report(v))).await
}

#[tauri::command]
pub async fn vault_import_kdbx(app: AppHandle, state: State<'_, AppState>, path: String, password: String, keyfile: Option<String>) -> CmdResult<ImportSummary> {
    change(&app, &state, move |v| {
        let bytes = std::fs::read(&path)?;
        let key = keyfile.map(std::fs::read).transpose()?;
        let name = Path::new(&path).file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        v.import_kdbx(&bytes, &password, key.as_deref(), &name)
    })
    .await
}

#[tauri::command]
pub async fn vault_import_csv(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<ImportSummary> {
    change(&app, &state, move |v| {
        let bytes = std::fs::read(&path)?;
        let text = String::from_utf8_lossy(&bytes).to_string();
        let name = Path::new(&path).file_stem().map(|n| format!("Imported from {}", n.to_string_lossy())).unwrap_or_else(|| "Imported".into());
        sompio_vault::csvfile::import(v, &text, &name)
    })
    .await
}

/// Exports never need a licence: you can always take your passwords with you.
#[tauri::command]
pub async fn vault_export_kdbx(app: AppHandle, path: String, password: String) -> CmdResult<()> {
    with_vault(&app, move |v| v.export_kdbx(Path::new(&path), &password, None)).await
}

#[tauri::command]
pub async fn vault_export_csv(app: AppHandle, path: String) -> CmdResult<()> {
    with_vault(&app, move |v| Ok(std::fs::write(&path, sompio_vault::csvfile::export(v)?)?)).await
}

#[tauri::command]
pub fn vault_settings(state: State<'_, AppState>) -> CmdResult<VaultSettings> {
    Ok(state.passwords.settings.lock().map_err(err)?.clone())
}

#[tauri::command]
pub fn vault_set_settings(app: AppHandle, state: State<'_, AppState>, settings: VaultSettings) -> CmdResult<()> {
    let old = state.passwords.settings.lock().map_err(err)?.shortcut.clone();
    if old != settings.shortcut {
        register_shortcut(&app, &old, &settings.shortcut)?;
    }
    std::fs::write(&state.passwords.settings_path, serde_json::to_string_pretty(&settings).map_err(err)?).map_err(err)?;
    *state.passwords.settings.lock().map_err(err)? = settings;
    Ok(())
}

// Auto-type

/// Swaps the global shortcut. Errors when another program already uses it.
pub fn register_shortcut(app: &AppHandle, old: &str, new: &str) -> CmdResult<()> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let gs = app.global_shortcut();
    if !old.is_empty() {
        let _ = gs.unregister(old);
    }
    if new.is_empty() {
        return Ok(());
    }
    gs.register(new).map_err(|e| format!("{new} can't be used as the auto-type shortcut ({e}). Another program may already use it."))
}

/// Called when the global shortcut is pressed.
pub fn on_shortcut(app: &AppHandle) {
    let Ok(win) = active_win_pos_rs::get_active_window() else { return };
    // Pressing it inside Sompio does nothing
    if win.process_id == std::process::id() as u64 {
        return;
    }
    let target = Target { title: win.title.clone(), hwnd: foreground_hwnd() };
    let state = app.state::<AppState>();
    if let Ok(mut t) = state.passwords.target.lock() {
        *t = Some(target.clone());
    }
    let unlocked = state.vault.status().map(|s| s.unlocked).unwrap_or(false);
    if !unlocked {
        crate::calendar::show_main(app);
        let _ = app.emit("vault-autotype-locked", target.title);
        return;
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let title = target.title.clone();
        let found = with_vault(&app2, move |v| Ok(autotype::matches(v, &title))).await.unwrap_or_default();
        if found.len() == 1 {
            let m = &found[0];
            let _ = type_entry(&app2, m.entry.id.clone(), m.sequence.clone(), target).await;
        } else {
            show_chooser(&app2);
        }
    });
}

#[derive(Serialize)]
pub struct ChooserState {
    window: String,
    matches: Vec<autotype::Match>,
}

/// What the chooser shows: the matches for the window, or nothing (then it searches everything).
#[tauri::command]
pub async fn vault_autotype_matches(app: AppHandle) -> CmdResult<ChooserState> {
    let title = app.state::<AppState>().passwords.target.lock().map_err(err)?.as_ref().map(|t| t.title.clone()).unwrap_or_default();
    let t2 = title.clone();
    let matches = with_vault(&app, move |v| Ok(autotype::matches(v, &t2))).await?;
    Ok(ChooserState { window: title, matches })
}

/// Picked in the chooser: close it, go back to the window, and type.
#[tauri::command]
pub async fn vault_autotype_choose(app: AppHandle, id: String, sequence: Option<String>) -> CmdResult<()> {
    if let Some(w) = app.get_webview_window("autotype") {
        let _ = w.close();
    }
    let target = app.state::<AppState>().passwords.target.lock().map_err(err)?.clone().ok_or("The window to type into is gone. Press the shortcut again.")?;
    type_entry(&app, id, sequence.unwrap_or_else(|| sompio_vault::store::DEFAULT_SEQUENCE.into()), target).await
}

/// "Type it" from the main window: Sompio steps aside and types into the window that was
/// behind it.
#[tauri::command]
pub async fn vault_autotype_now(app: AppHandle, id: String) -> CmdResult<()> {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.minimize();
    }
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    let win = active_win_pos_rs::get_active_window().map_err(|_| "Couldn't tell which window is in front.".to_string())?;
    if win.process_id == std::process::id() as u64 {
        return Err("Click into the window you want to fill first, then use Type it.".into());
    }
    let target = Target { title: win.title, hwnd: foreground_hwnd() };
    let seq = with_vault(&app, {
        let id = id.clone();
        move |v| {
            let d = v.entry(&id)?;
            Ok(if d.autotype.sequence.is_empty() {
                if d.summary.username.is_empty() { "{PASSWORD}{ENTER}".to_string() } else { sompio_vault::store::DEFAULT_SEQUENCE.to_string() }
            } else {
                d.autotype.sequence
            })
        }
    })
    .await?;
    type_entry(&app, id, seq, target).await
}

async fn type_entry(app: &AppHandle, id: String, sequence: String, target: Target) -> CmdResult<()> {
    let typing = with_vault(app, move |v| autotype::resolve(v, &id, &sequence)).await?;
    let lock_after = app.state::<AppState>().passwords.settings.lock().map(|s| s.lock_after_type).unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || {
        focus(&target);
        std::thread::sleep(std::time::Duration::from_millis(250));
        send(&typing.0)
    })
    .await
    .map_err(err)?
    .map_err(|e| format!("Auto-type stopped: {e}"))?;
    if lock_after {
        let state = app.state::<AppState>();
        state.vault.lock();
        state.passwords.close();
        let _ = app.emit("locked", ());
    }
    Ok(())
}

fn show_chooser(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("autotype") {
        let _ = w.emit("vault-autotype", ());
        let _ = w.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "autotype", WebviewUrl::App("index.html#autotype".into()))
        .title("Sompio auto-type")
        .inner_size(460.0, 420.0)
        .center()
        .decorations(false)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .content_protected(true)
        .shadow(true)
        .focused(true)
        .build();
}

#[cfg(windows)]
fn foreground_hwnd() -> isize {
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() as isize }
}

#[cfg(not(windows))]
fn foreground_hwnd() -> isize {
    0
}

/// Puts the window auto-type was asked for back in front.
#[cfg(windows)]
fn focus(t: &Target) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE};
    if t.hwnd == 0 {
        return;
    }
    unsafe {
        let h = t.hwnd as windows_sys::Win32::Foundation::HWND;
        if IsIconic(h) != 0 {
            ShowWindow(h, SW_RESTORE);
        }
        SetForegroundWindow(h);
    }
}

#[cfg(not(windows))]
fn focus(_t: &Target) {}

fn key(k: AtKey) -> enigo::Key {
    use enigo::Key as K;
    match k {
        AtKey::Tab => K::Tab,
        AtKey::Enter => K::Return,
        AtKey::Space => K::Space,
        AtKey::Backspace => K::Backspace,
        AtKey::Delete => K::Delete,
        AtKey::Insert => K::Insert,
        AtKey::Up => K::UpArrow,
        AtKey::Down => K::DownArrow,
        AtKey::Left => K::LeftArrow,
        AtKey::Right => K::RightArrow,
        AtKey::Home => K::Home,
        AtKey::End => K::End,
        AtKey::PageUp => K::PageUp,
        AtKey::PageDown => K::PageDown,
        AtKey::Escape => K::Escape,
        AtKey::F(n) => [K::F1, K::F2, K::F3, K::F4, K::F5, K::F6, K::F7, K::F8, K::F9, K::F10, K::F11, K::F12, K::F13, K::F14, K::F15, K::F16][(n.clamp(1, 16) - 1) as usize],
    }
}

/// Sends the keystrokes. Typed text goes in as characters, so keyboard layouts don't matter.
fn send(steps: &[Step]) -> Result<(), String> {
    use enigo::{Direction::*, Enigo, Keyboard, Settings};
    let mut en = Enigo::new(&Settings::default()).map_err(err)?;
    // The shortcut's own Ctrl/Alt may still be held down
    for m in [enigo::Key::Control, enigo::Key::Alt, enigo::Key::Shift, enigo::Key::Meta] {
        let _ = en.key(m, Release);
    }
    let mut pause = std::time::Duration::from_millis(8);
    let hold = |en: &mut Enigo, m: Mods, dir: enigo::Direction| -> Result<(), String> {
        if m.ctrl { en.key(enigo::Key::Control, dir).map_err(err)?; }
        if m.alt { en.key(enigo::Key::Alt, dir).map_err(err)?; }
        if m.shift { en.key(enigo::Key::Shift, dir).map_err(err)?; }
        Ok(())
    };
    for s in steps {
        match s {
            Step::Text(t) => {
                for ch in t.chars() {
                    en.text(&ch.to_string()).map_err(err)?;
                    std::thread::sleep(pause);
                }
            }
            Step::Key { key: k, mods, times } => {
                hold(&mut en, *mods, Press)?;
                for _ in 0..*times {
                    en.key(key(*k), Click).map_err(err)?;
                    std::thread::sleep(pause);
                }
                hold(&mut en, *mods, Release)?;
            }
            Step::Combo { ch, mods } => {
                hold(&mut en, *mods, Press)?;
                en.key(enigo::Key::Unicode(ch.to_ascii_lowercase()), Click).map_err(err)?;
                hold(&mut en, *mods, Release)?;
                std::thread::sleep(pause);
            }
            Step::ClearField => {
                en.key(enigo::Key::Control, Press).map_err(err)?;
                en.key(enigo::Key::Unicode('a'), Click).map_err(err)?;
                en.key(enigo::Key::Control, Release).map_err(err)?;
                en.key(enigo::Key::Delete, Click).map_err(err)?;
            }
            Step::Delay(ms) => std::thread::sleep(std::time::Duration::from_millis(*ms)),
            Step::KeyDelay(ms) => pause = std::time::Duration::from_millis(*ms),
            Step::Field(_) => {}
        }
    }
    Ok(())
}

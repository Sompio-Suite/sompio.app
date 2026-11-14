//! The desktop shell. It only passes requests from the UI to sompio-clean and back.
//! Every command runs off the UI thread so the window never freezes.

mod browser;
mod calendar;
mod vault;
mod samples;
mod problems;
mod ocr;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use sompio_clean::{
    ChannelView, FindingView, FindingsPage, ImportSummary, MessageView, MessagesPage, ScanOptions,
    ScanSummary, SearchPage, SourceView, Stats, StatusFilter, Store,
};

struct AppState {
    /// The Sompio Lock and the key to each app's encrypted data. Nothing opens while it's locked.
    vault: Arc<sompio_suite::Vault>,
    /// The licence keys entered on this PC, and the trial
    licences: Licences,
    /// The Telegram connection. Only does anything after the person connects an account.
    telegram: tokio::sync::Mutex<sompio_connect::telegram::Telegram>,
    /// The email connection (IMAP). Blocking, so it always runs on a background thread.
    email: std::sync::Arc<sompio_connect::email::Email>,
    /// Reminders: what's coming up and what's on screen. Kept in memory so they run while locked.
    calendar: calendar::Calendar,
    /// The password vault while it's open, and auto-type
    passwords: vault::VaultState,
    /// The browser extension's pairing requests
    browser: browser::Bridge,
}

type CmdResult<T> = Result<T, String>;

/// Whether this build can update itself (only published releases can).
struct UpdatesOn(bool);

#[tauri::command]
fn updates_on(state: State<'_, UpdatesOn>) -> bool {
    state.0
}

/// The code published builds send when asking sompio.org for updates, so only Sompio apps get the
/// beta installer. Set by the cloud build (SOMPIO_UPDATE_TOKEN); empty in local builds.
#[tauri::command]
fn update_token() -> &'static str {
    option_env!("SOMPIO_UPDATE_TOKEN").unwrap_or("")
}

// The Sompio Lock. Argon2 takes about half a second on purpose, so these run off the UI thread.

async fn with_vault<T, F>(state: &State<'_, AppState>, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&sompio_suite::Vault) -> anyhow::Result<T> + Send + 'static,
{
    let vault = state.vault.clone();
    tauri::async_runtime::spawn_blocking(move || f(&vault).map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn lock_status(state: State<'_, AppState>) -> CmdResult<sompio_suite::LockStatus> {
    state.vault.status().map_err(|e| e.to_string())
}

#[tauri::command]
async fn lock_setup(state: State<'_, AppState>, password: String, picture_path: String, remember: bool) -> CmdResult<String> {
    with_vault(&state, move |v| Ok(v.setup(&password, Path::new(&picture_path), remember)?.to_string())).await
}

#[tauri::command]
async fn lock_new_picture(state: State<'_, AppState>, picture_path: String) -> CmdResult<()> {
    with_vault(&state, move |v| v.new_picture(Path::new(&picture_path))).await
}

#[tauri::command]
async fn lock_setup_with_picture(state: State<'_, AppState>, password: String, remember: bool) -> CmdResult<String> {
    with_vault(&state, move |v| Ok(v.setup_with_picture(&password, remember)?.to_string())).await
}

#[tauri::command]
async fn lock_unlock(state: State<'_, AppState>, password: String, picture_path: Option<String>, remember: bool) -> CmdResult<()> {
    with_vault(&state, move |v| v.unlock(&password, picture_path.as_deref().map(Path::new), remember)).await?;
    state.calendar.mark_dirty();
    Ok(())
}

#[tauri::command]
async fn lock_recover(
    state: State<'_, AppState>,
    code: String,
    new_password: String,
    picture_path: String,
    remember: bool,
) -> CmdResult<String> {
    with_vault(&state, move |v| Ok(v.recover(&code, &new_password, Path::new(&picture_path), remember)?.to_string())).await
}

#[tauri::command]
async fn lock_change_password(state: State<'_, AppState>, old: String, new: String, picture_path: Option<String>) -> CmdResult<()> {
    with_vault(&state, move |v| v.change_password(&old, &new, picture_path.as_deref().map(Path::new))).await
}

// YubiKey: stands in for the key picture. The device is asked with HID feature reports.

fn touch_notice(app: &tauri::AppHandle) -> impl FnMut() + '_ {
    move || {
        let _ = app.emit("yubikey-touch", ());
    }
}

fn ask_key(app: &tauri::AppHandle, k: &sompio_suite::HardwareKeyInfo) -> anyhow::Result<[u8; 20]> {
    let mut touch = touch_notice(app);
    Ok(sompio_hwkey::challenge(k.slot, &k.challenge, k.serial, &mut touch)?.0)
}

#[derive(serde::Serialize)]
struct YubiKeyFound {
    name: String,
    serial: Option<u32>,
}

#[tauri::command]
async fn lock_yubikeys_present() -> CmdResult<Vec<YubiKeyFound>> {
    tauri::async_runtime::spawn_blocking(|| sompio_hwkey::list().map(|l| l.into_iter().map(|f| YubiKeyFound { name: f.name, serial: f.serial }).collect()))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn lock_unlock_yubikey(app: tauri::AppHandle, state: State<'_, AppState>, password: String) -> CmdResult<()> {
    with_vault(&state, move |v| v.unlock_with_hardware(&password, &mut |k| ask_key(&app, k))).await?;
    state.calendar.mark_dirty();
    Ok(())
}

#[tauri::command]
async fn lock_add_yubikey(app: tauri::AppHandle, state: State<'_, AppState>, picture_path: Option<String>, slot: u8, label: String) -> CmdResult<()> {
    with_vault(&state, move |v| {
        v.add_hardware_key(picture_path.as_deref().map(Path::new), slot, &label, &mut |slot, ch| {
            let mut touch = touch_notice(&app);
            sompio_hwkey::challenge(slot, ch, None, &mut touch)
        })
    })
    .await
}

#[tauri::command]
async fn lock_remove_yubikey(state: State<'_, AppState>, index: usize) -> CmdResult<()> {
    with_vault(&state, move |v| v.remove_hardware_key(index)).await
}

#[tauri::command]
async fn lock_change_password_yubikey(app: tauri::AppHandle, state: State<'_, AppState>, old: String, new: String) -> CmdResult<()> {
    with_vault(&state, move |v| v.change_password_with_hardware(&old, &new, &mut |k| ask_key(&app, k))).await
}

#[tauri::command]
fn lock_forget_picture(state: State<'_, AppState>) -> CmdResult<()> {
    state.vault.forget_picture().map_err(|e| e.to_string())
}

#[tauri::command]
fn lock_lock(state: State<'_, AppState>) {
    state.vault.lock();
    state.passwords.close();
}

// Licences: which apps are unlocked. Checked offline against the public key built into the app.

use sompio_licence::{App, Book};

struct Licences {
    path: PathBuf,
    book: std::sync::Mutex<Book>,
    public: Option<ed25519_dalek::VerifyingKey>,
}

fn clock() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

impl Licences {
    fn load(path: PathBuf) -> Self {
        let mut book = Book::load(&path);
        book.start_trial(clock());
        let _ = book.save(&path);
        Self { path, book: std::sync::Mutex::new(book), public: sompio_licence::public_key().ok() }
    }

    fn public(&self) -> CmdResult<&ed25519_dalek::VerifyingKey> {
        self.public.as_ref().ok_or_else(|| "This copy of Sompio has no licence key built in. Build it again with Build Sompio.bat.".to_string())
    }

    fn status(&self) -> CmdResult<sompio_licence::Status> {
        let public = self.public()?;
        let mut book = self.book.lock().map_err(|e| e.to_string())?;
        let st = book.status(clock(), public);
        let _ = book.save(&self.path); // remembers the latest time seen
        Ok(st)
    }

    /// Refuses actions that change things when the app isn't unlocked. Looking stays possible.
    fn require(&self, app: App) -> CmdResult<()> {
        if self.status()?.apps.iter().any(|a| a.app == app && a.active) {
            Ok(())
        } else {
            Err(format!(
                "Your Sompio {} access has ended. Add a key under Licences to continue. Everything already here stays readable.",
                app.name()
            ))
        }
    }
}

#[tauri::command]
fn licence_status(state: State<'_, AppState>) -> CmdResult<sompio_licence::Status> {
    state.licences.status()
}

#[tauri::command]
fn licence_add(state: State<'_, AppState>, key: String) -> CmdResult<sompio_licence::Added> {
    let l = &state.licences;
    let public = l.public()?;
    let mut book = l.book.lock().map_err(|e| e.to_string())?;
    let added = book.add(&key, clock(), public).map_err(|e| e.to_string())?;
    book.save(&l.path).map_err(|e| format!("Couldn't save the key: {e}"))?;
    Ok(added)
}

type DbKey = (PathBuf, zeroize::Zeroizing<[u8; 32]>);

impl AppState {
    fn passwords_shortcut(&self) -> String {
        vault::shortcut(&self.passwords)
    }

    /// Clean's encrypted database and its key, or an error while Sompio is locked.
    fn db(&self) -> CmdResult<DbKey> {
        self.vault.clean_db().map_err(|e| e.to_string())
    }
}

/// Opens the encrypted database on a background thread, runs `f`, and reports errors as plain text.
async fn with_store<T, F>(db: DbKey, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut Store) -> anyhow::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let mut store = Store::open_encrypted(&db.0, &db.1)?;
        f(&mut store)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn has_data(state: State<'_, AppState>) -> CmdResult<bool> {
    with_store(state.db()?, |s| s.has_data()).await
}

#[tauri::command]
async fn import_package(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<ImportSummary> {
    state.licences.require(App::Clean)?;
    with_store(state.db()?, move |s| {
        sompio_clean::import_package(s, Path::new(&path), &mut |p| {
            let _ = app.emit("import-progress", &p);
        })
    })
    .await
}

#[tauri::command]
async fn scan_folders(
    app: AppHandle,
    state: State<'_, AppState>,
    folders: Vec<String>,
    opts: Option<sompio_clean::PcScanOptions>,
) -> CmdResult<ImportSummary> {
    state.licences.require(App::Clean)?;
    let opts = opts.unwrap_or_default();
    with_store(state.db()?, move |s| {
        let folders: Vec<PathBuf> = folders.into_iter().map(PathBuf::from).collect();
        let reader: &dyn Fn(&std::path::Path) -> Option<String> = &ocr::read_image_text;
        sompio_clean::scan_folders_with(s, &folders, opts, opts.image_text.then_some(reader), &mut |p| {
            let _ = app.emit("import-progress", &p);
        })
    })
    .await
}

/// The data exports the person said they asked for, and when (kept encrypted with everything else).
#[tauri::command]
async fn export_requests(state: State<'_, AppState>) -> CmdResult<String> {
    with_store(state.db()?, |s| Ok(s.get_meta("export_requests")?.unwrap_or_else(|| "{}".into()))).await
}

#[tauri::command]
async fn set_export_requests(state: State<'_, AppState>, json: String) -> CmdResult<()> {
    with_store(state.db()?, move |s| {
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&json)?;
        s.set_meta("export_requests", &json)
    })
    .await
}

#[tauri::command]
async fn sources(state: State<'_, AppState>) -> CmdResult<Vec<SourceView>> {
    with_store(state.db()?, |s| s.sources()).await
}

#[tauri::command]
async fn remove_source(state: State<'_, AppState>, platform: String) -> CmdResult<()> {
    with_store(state.db()?, move |s| s.remove_source(&platform)).await
}

/// Moves a file found by the PC scan to the Recycle Bin.
#[tauri::command]
async fn recycle_file(state: State<'_, AppState>, id: i64) -> CmdResult<(String, ScanSummary)> {
    state.licences.require(App::Clean)?;
    with_store(state.db()?, move |s| {
        let path = s.recycle_file(id)?;
        Ok((path, s.scan_summary()?))
    })
    .await
}

/// Opens File Explorer with the file selected. Only paths Sompio itself found can be shown.
#[tauri::command]
async fn show_in_folder(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    let path = with_store(state.db()?, move |s| s.file_path_for(id)).await?;
    let path = path.ok_or_else(|| "This item isn't a file on this PC.".to_string())?;
    tauri_plugin_opener::reveal_item_in_dir(&path).map_err(|e| format!("Couldn't open the folder: {e}"))
}

// ---------------------------------------------------------------------------------
// Telegram (live connection)

use sompio_connect::telegram::{LoginStep, Status as TgStatus, SyncProgress};

fn text_err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[tauri::command]
async fn tg_status(state: State<'_, AppState>) -> CmdResult<TgStatus> {
    state.telegram.lock().await.status().await.map_err(text_err)
}

#[tauri::command]
async fn tg_set_app_key(state: State<'_, AppState>, api_id: String, api_hash: String) -> CmdResult<()> {
    state.telegram.lock().await.set_app_key(&api_id, &api_hash).map_err(text_err)
}

#[tauri::command]
async fn tg_start_login(state: State<'_, AppState>, phone: String) -> CmdResult<()> {
    state.telegram.lock().await.start_login(&phone).await.map_err(text_err)
}

#[tauri::command]
async fn tg_submit_code(state: State<'_, AppState>, code: String) -> CmdResult<LoginStep> {
    state.telegram.lock().await.submit_code(&code).await.map_err(text_err)
}

#[tauri::command]
async fn tg_submit_password(state: State<'_, AppState>, password: String) -> CmdResult<LoginStep> {
    state.telegram.lock().await.submit_password(&password).await.map_err(text_err)
}

#[tauri::command]
async fn tg_disconnect(state: State<'_, AppState>) -> CmdResult<()> {
    state.telegram.lock().await.disconnect().await.map_err(text_err)
}

/// Downloads your own Telegram messages and replaces the Telegram data in Sompio with them.
#[tauri::command]
async fn tg_sync(app: AppHandle, state: State<'_, AppState>) -> CmdResult<ImportSummary> {
    state.licences.require(App::Clean)?;
    let channels = {
        let mut tg = state.telegram.lock().await;
        let app2 = app.clone();
        tg.fetch_own_messages(&mut move |p: SyncProgress| {
            let _ = app2.emit("tg-progress", &p);
        })
        .await
        .map_err(text_err)?
    };
    with_store(state.db()?, move |s| sompio_clean::import_live(s, "telegram", "Connected Telegram account", channels)).await
}

/// Deletes the message a finding is in, on Telegram, for everyone.
#[tauri::command]
async fn tg_delete(state: State<'_, AppState>, id: i64) -> CmdResult<ScanSummary> {
    state.licences.require(App::Clean)?;
    let (platform, uid) = with_store(state.db()?, move |s| s.delete_target(id)).await?;
    if platform != "telegram" {
        return Err("This item isn't a Telegram message.".into());
    }
    state.telegram.lock().await.delete_message(&uid).await.map_err(text_err)?;
    with_store(state.db()?, move |s| {
        s.mark_deleted(id)?;
        s.scan_summary()
    })
    .await
}

// Email

use sompio_connect::email::{Email, Provider, Status as EmailStatus, SyncProgress as EmailProgress};

async fn with_email<T, F>(state: &State<'_, AppState>, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&Email) -> anyhow::Result<T> + Send + 'static,
{
    let email = state.email.clone();
    tauri::async_runtime::spawn_blocking(move || f(&email).map_err(text_err))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn email_providers() -> Vec<Provider> {
    sompio_connect::email::PROVIDERS.to_vec()
}

#[tauri::command]
async fn email_status(state: State<'_, AppState>) -> CmdResult<EmailStatus> {
    with_email(&state, |e| e.status()).await
}

#[tauri::command]
async fn email_connect(state: State<'_, AppState>, provider: String, host: String, address: String, password: String) -> CmdResult<EmailStatus> {
    with_email(&state, move |e| e.connect(&provider, &host, &address, &password)).await
}

#[tauri::command]
async fn email_disconnect(state: State<'_, AppState>) -> CmdResult<()> {
    with_email(&state, |e| e.disconnect()).await
}

/// Reads the Sent folder and replaces the email data with it.
#[tauri::command]
async fn email_sync(app: AppHandle, state: State<'_, AppState>) -> CmdResult<ImportSummary> {
    state.licences.require(App::Clean)?;
    let channels = with_email(&state, move |e| {
        e.fetch_sent(&mut |p: EmailProgress| {
            let _ = app.emit("email-progress", &p);
        })
    })
    .await?;
    with_store(state.db()?, move |s| sompio_clean::import_live(s, "email", "Connected email account", channels)).await
}

/// Moves the sent email a finding is in to the mailbox's Trash.
#[tauri::command]
async fn email_delete(state: State<'_, AppState>, id: i64) -> CmdResult<ScanSummary> {
    state.licences.require(App::Clean)?;
    let (platform, uid) = with_store(state.db()?, move |s| s.delete_target(id)).await?;
    if platform != "email" {
        return Err("This item isn't an email.".into());
    }
    let ext = uid.strip_prefix("email:").unwrap_or(&uid).to_string();
    with_email(&state, move |e| e.delete_message(&ext)).await?;
    with_store(state.db()?, move |s| {
        s.mark_deleted(id)?;
        s.scan_summary()
    })
    .await
}

// The names someone goes by in chats, for exports that don't say which messages are theirs.

#[tauri::command]
async fn my_names(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    with_store(state.db()?, |s| s.my_names()).await
}

#[tauri::command]
async fn set_my_names(state: State<'_, AppState>, names: Vec<String>) -> CmdResult<()> {
    with_store(state.db()?, move |s| s.set_my_names(&names)).await
}

#[tauri::command]
async fn stats(state: State<'_, AppState>) -> CmdResult<Stats> {
    with_store(state.db()?, |s| s.stats()).await
}

#[tauri::command]
async fn channels(state: State<'_, AppState>, filter: Option<String>) -> CmdResult<Vec<ChannelView>> {
    with_store(state.db()?, move |s| s.channels(filter.as_deref())).await
}

#[tauri::command]
async fn messages(state: State<'_, AppState>, channel_id: String, offset: i64, limit: i64) -> CmdResult<MessagesPage> {
    with_store(state.db()?, move |s| s.messages(&channel_id, offset, limit)).await
}

#[tauri::command]
async fn message_index(state: State<'_, AppState>, channel_id: String, message_id: String) -> CmdResult<i64> {
    with_store(state.db()?, move |s| s.message_index(&channel_id, &message_id)).await
}

#[tauri::command]
async fn search(
    state: State<'_, AppState>,
    text: String,
    channel_id: Option<String>,
    offset: i64,
    limit: i64,
) -> CmdResult<SearchPage> {
    with_store(state.db()?, move |s| s.search(&text, channel_id.as_deref(), offset, limit)).await
}

#[tauri::command]
async fn run_scan(app: AppHandle, state: State<'_, AppState>, watch_terms: Vec<String>) -> CmdResult<ScanSummary> {
    state.licences.require(App::Clean)?;
    with_store(state.db()?, move |s| {
        sompio_clean::run_scan(s, &ScanOptions { watch_terms }, &mut |done, total| {
            let _ = app.emit("scan-progress", (done, total));
        })
    })
    .await
}

#[tauri::command]
async fn scan_summary(state: State<'_, AppState>) -> CmdResult<ScanSummary> {
    with_store(state.db()?, |s| s.scan_summary()).await
}

#[tauri::command]
async fn findings(
    state: State<'_, AppState>,
    kind: Option<String>,
    platform: Option<String>,
    status: String,
    offset: i64,
    limit: i64,
    reveal: bool,
    group: Option<bool>,
    same_as: Option<i64>,
) -> CmdResult<FindingsPage> {
    with_store(state.db()?, move |s| {
        let filter = sompio_clean::FindingsFilter {
            kind: kind.as_deref(),
            platform: platform.as_deref(),
            status: StatusFilter::parse(&status),
            group: group.unwrap_or(false),
            same_as,
            only_id: None,
        };
        s.findings_filtered(&filter, offset, limit, reveal)
    })
    .await
}

/// Everywhere the value in a finding appears, within one status list.
#[tauri::command]
async fn group_info(state: State<'_, AppState>, id: i64, status: String) -> CmdResult<sompio_clean::GroupInfo> {
    with_store(state.db()?, move |s| s.group_info(id, StatusFilter::parse(&status))).await
}

type Previous = Vec<(i64, Option<String>)>;

/// One decision for many findings. Returns what they were before, for Undo.
#[tauri::command]
async fn set_status_many(state: State<'_, AppState>, ids: Vec<i64>, status: Option<String>) -> CmdResult<(Previous, ScanSummary)> {
    with_store(state.db()?, move |s| {
        let before = s.set_status_many(&ids, status.as_deref())?;
        Ok((before, s.scan_summary()?))
    })
    .await
}

#[tauri::command]
async fn restore_statuses(state: State<'_, AppState>, previous: Previous) -> CmdResult<ScanSummary> {
    with_store(state.db()?, move |s| {
        s.restore_statuses(&previous)?;
        s.scan_summary()
    })
    .await
}

#[tauri::command]
async fn reveal_finding(state: State<'_, AppState>, id: i64) -> CmdResult<Option<FindingView>> {
    with_store(state.db()?, move |s| s.reveal_finding(id)).await
}

#[tauri::command]
async fn reveal_message(state: State<'_, AppState>, message_id: String) -> CmdResult<Option<MessageView>> {
    with_store(state.db()?, move |s| s.reveal_message(&message_id)).await
}

#[tauri::command]
async fn set_status(state: State<'_, AppState>, id: i64, status: Option<String>) -> CmdResult<ScanSummary> {
    with_store(state.db()?, move |s| {
        s.set_status(id, status.as_deref())?;
        s.scan_summary()
    })
    .await
}

#[tauri::command]
async fn ignore_everywhere(state: State<'_, AppState>, id: i64) -> CmdResult<(usize, ScanSummary)> {
    with_store(state.db()?, move |s| {
        let n = s.ignore_everywhere(id)?;
        Ok((n, s.scan_summary()?))
    })
    .await
}

#[tauri::command]
async fn clear_ignored(state: State<'_, AppState>) -> CmdResult<usize> {
    with_store(state.db()?, |s| s.clear_ignored()).await
}

#[tauri::command]
async fn load_demo(state: State<'_, AppState>) -> CmdResult<()> {
    with_store(state.db()?, |s| sompio_clean::load_demo(s)).await
}

#[tauri::command]
async fn is_demo(state: State<'_, AppState>) -> CmdResult<bool> {
    with_store(state.db()?, |s| s.is_demo()).await
}

#[tauri::command]
async fn watch_terms(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    with_store(state.db()?, |s| s.watch_terms()).await
}

/// Opens a web page or a ready-to-send email in the person's own browser or mail app. Only
/// https, http and mailto: never files, programs or other schemes.
#[tauri::command]
fn open_external(app: AppHandle, url: String) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let u = url.trim();
    let lower = u.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("mailto:")) {
        return Err("Only web and email links can be opened.".into());
    }
    if u.len() > 32_000 || u.chars().any(|c| c.is_control()) || !link_host_ok(u) {
        return Err("That link can't be opened.".into());
    }
    app.opener().open_url(u, None::<&str>).map_err(|e| format!("Couldn't open it: {e}"))
}

/// Web links must name their website plainly: no "name@" in front of it ("https://bank.com@evil.site"
/// really goes to evil.site) and no backslashes, which browsers read in surprising ways.
fn link_host_ok(url: &str) -> bool {
    let Some(rest) = url.split_once("://").map(|(_, r)| r) else {
        return true; // mailto: has no website part
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.is_empty() && !host.contains('@') && !host.contains('\\') && !rest.starts_with('/')
}

#[cfg(test)]
mod link_tests {
    use super::link_host_ok;

    #[test]
    fn disguised_links_are_refused() {
        assert!(link_host_ok("https://sompio.org/pricing.php"));
        assert!(link_host_ok("https://discord.com/channels/1/2/3?x=a@b"));
        assert!(link_host_ok("mailto:privacy@example.com?subject=Hi"));
        assert!(!link_host_ok("https://bank.com@evil.site/login"));
        assert!(!link_host_ok("https://evil.site\\@bank.com"));
        assert!(!link_host_ok("https:///evil"));
        assert!(!link_host_ok("https://"));
    }
}

// ---------- Trace: request centre ----------

#[tauri::command]
async fn trace_data(state: State<'_, AppState>) -> CmdResult<sompio_clean::trace::TraceData> {
    with_store(state.db()?, |s| s.trace_data()).await
}

#[tauri::command]
async fn trace_save(state: State<'_, AppState>, data: sompio_clean::trace::TraceData) -> CmdResult<()> {
    with_store(state.db()?, move |s| s.set_trace_data(&data)).await
}

/// A one-off Calendar reminder (a request deadline, an export to check on), in a schedule of its own.
#[tauri::command]
async fn trace_remind(state: State<'_, AppState>, date: String, title: String, text: String) -> CmdResult<()> {
    if chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").is_err() {
        return Err("That isn't a date.".into());
    }
    let (path, key) = state.vault.calendar_db().map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<()> {
        const NAME: &str = "Trace requests";
        let store = sompio_calendar::CalendarStore::open_encrypted(&path, &key)?;
        let id = match store.load()?.into_iter().find(|l| l.view.name == NAME && l.view.owned) {
            Some(l) => l.view.id,
            None => store.create_shared(NAME, "", &[])?,
        };
        store.add_row(
            id,
            &sompio_calendar::RowInput { days: date, time: "10:00".into(), title, text, ..Default::default() },
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    state.calendar.mark_dirty();
    Ok(())
}

#[tauri::command]
async fn export_flagged(state: State<'_, AppState>, path: String) -> CmdResult<usize> {
    with_store(state.db()?, move |s| s.export_flagged_csv(Path::new(&path))).await
}

#[tauri::command]
async fn wipe_data(state: State<'_, AppState>) -> CmdResult<()> {
    with_store(state.db()?, |s| s.wipe()).await
}

#[tauri::command]
fn data_location(state: State<'_, AppState>) -> String {
    state.vault.clean_db_path().display().to_string()
}

/// Testers ran this app as "Veil" before it became Sompio. Brings their Clean data along once.
fn move_from_veil(dir: &Path, db_path: &Path) {
    if db_path.exists() {
        return;
    }
    if let Some(old) = dir.parent().map(|p| p.join("app.veil.desktop").join("veil.db")) {
        if old.exists() {
            let _ = std::fs::copy(&old, db_path);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Started by a browser for the Sompio extension: pass messages to Sompio, nothing else
    if browser::is_host_launch() {
        return browser::run_host();
    }
    let context = tauri::generate_context!();
    // Updates are only switched on in published releases (tauri.release.conf.json). A local
    // build has no update address, so the updater isn't loaded at all.
    let updates = context
        .config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .is_some_and(|k| !k.is_empty() && k != "SET_UP_UPDATES_FIRST");
    let mut builder = tauri::Builder::default();
    if updates {
        // Checks one small file, installs only versions signed with our update key.
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }
    builder
        .manage(UpdatesOn(updates))
        // A second launch (e.g. the shortcut clicked while Sompio sits in the tray) opens the window.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| calendar::show_main(app)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--autostart"])))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    let _ = shortcut;
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        vault::on_shortcut(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Local app data (not roaming): the database never syncs to other machines.
            let dir = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            problems::start(&dir);
            let secrets = std::sync::Arc::new(sompio_connect::KeyringStore::new("Sompio"));
            move_from_veil(&dir, &dir.join("clean.db"));
            app.manage(AppState {
                licences: Licences::load(dir.join("licences.json")),
                vault: Arc::new(sompio_suite::Vault::new(dir.clone(), secrets.clone())),
                telegram: tokio::sync::Mutex::new(sompio_connect::telegram::Telegram::new(secrets.clone())),
                email: std::sync::Arc::new(sompio_connect::email::Email::new(secrets)),
                calendar: calendar::Calendar::new(),
                passwords: vault::VaultState::new(&dir),
                browser: browser::Bridge::default(),
            });
            // The browser extension: a local pipe, and the browsers told where Sompio is
            browser::start(app.handle().clone());
            let browser_on = app.state::<AppState>().passwords.settings.lock().map(|s| s.browser).unwrap_or(false);
            if browser_on {
                if let Err(e) = browser::register(app.handle(), true) {
                    eprintln!("Browser extension: {e}");
                }
            }
            let shortcut = app.state::<AppState>().passwords_shortcut();
            if let Err(e) = vault::register_shortcut(app.handle(), "", &shortcut) {
                eprintln!("{e}");
            }
            calendar::tray(app.handle())?;
            calendar::start(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the main window keeps Sompio in the tray while reminders are loaded.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" && calendar::keeps_running(window.app_handle()) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            browser::browser_answer,
            browser::browser_clients,
            browser::browser_forget,
            browser::browser_extension_folder,
            browser::browser_open_extension_folder,
            browser::browser_installed,
            browser::browser_open_extensions_page,
            has_data,
            updates_on,
            update_token,
            lock_status,
            lock_setup,
            lock_unlock,
            lock_new_picture,
            lock_setup_with_picture,
            lock_recover,
            lock_change_password,
            lock_forget_picture,
            lock_yubikeys_present,
            lock_unlock_yubikey,
            lock_add_yubikey,
            lock_remove_yubikey,
            lock_change_password_yubikey,
            lock_lock,
            licence_status,
            licence_add,
            import_package,
            scan_folders,
            export_requests,
            set_export_requests,
            sources,
            remove_source,
            recycle_file,
            show_in_folder,
            tg_status,
            tg_set_app_key,
            tg_start_login,
            tg_submit_code,
            tg_submit_password,
            tg_disconnect,
            tg_sync,
            tg_delete,
            email_providers,
            email_status,
            email_connect,
            email_disconnect,
            email_sync,
            email_delete,
            my_names,
            set_my_names,
            group_info,
            set_status_many,
            restore_statuses,
            stats,
            channels,
            messages,
            message_index,
            search,
            run_scan,
            scan_summary,
            findings,
            reveal_finding,
            reveal_message,
            set_status,
            ignore_everywhere,
            clear_ignored,
            load_demo,
            is_demo,
            watch_terms,
            open_external,
            trace_data,
            trace_save,
            trace_remind,
            export_flagged,
            wipe_data,
            data_location,
            calendar::cal_overview,
            calendar::cal_next,
            calendar::cal_add_rows,
            calendar::cal_update_row,
            calendar::cal_remove_row,
            calendar::cal_skip_once,
            calendar::cal_import_file,
            calendar::cal_import_desk_reminders,
            calendar::cal_export,
            calendar::cal_create_shared,
            calendar::cal_set_options,
            calendar::cal_rename,
            calendar::cal_remove_schedule,
            calendar::cal_set_alert,
            calendar::cal_set_settings,
            calendar::cal_popups,
            calendar::cal_popup_action,
            calendar::cal_pause,
            calendar::cal_paused,
            calendar::cal_autostart,
            calendar::cal_set_autostart,
            vault::vault_groups,
            vault::vault_entries,
            vault::vault_entry,
            vault::vault_reveal,
            vault::vault_totp,
            vault::vault_copy,
            vault::vault_copy_text,
            vault::vault_add_entry,
            vault::vault_update_entry,
            vault::vault_delete_entry,
            vault::vault_restore_entry,
            vault::vault_restore_version,
            vault::vault_move_entry,
            vault::vault_clone_entry,
            vault::vault_icons,
            vault::vault_fetch_icon,
            vault::vault_icon_from_file,
            vault::vault_remove_icon,
            vault::vault_fetch_all_icons,
            vault::vault_empty_bin,
            vault::vault_add_group,
            vault::vault_rename_group,
            vault::vault_move_group,
            vault::vault_delete_group,
            vault::vault_generate,
            vault::vault_strength,
            vault::vault_health,
            vault::vault_import_kdbx,
            vault::vault_import_csv,
            vault::vault_export_kdbx,
            vault::vault_export_csv,
            vault::vault_settings,
            vault::vault_set_settings,
            vault::vault_autotype_matches,
            vault::vault_autotype_choose,
            vault::vault_autotype_now,
            vault::vault_add_samples,
            vault::vault_add_attachment,
            vault::vault_save_attachment,
            vault::vault_remove_attachment,
            vault::vault_merge_kdbx,
            vault::vault_check_leaks,
            vault::vault_in_trace,
            samples::samples_write,
            samples::cal_add_samples,
            problems::log_problem,
            problems::app_details,
            problems::open_problems_folder,
        ])
        .run(context)
        .expect("error while running tauri application");
}

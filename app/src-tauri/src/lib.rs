//! The desktop shell. It only passes requests from the UI to sompio-clean and back.
//! Every command runs off the UI thread so the window never freezes.

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
}

type CmdResult<T> = Result<T, String>;

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
async fn lock_unlock(state: State<'_, AppState>, password: String, picture_path: Option<String>, remember: bool) -> CmdResult<()> {
    with_vault(&state, move |v| v.unlock(&password, picture_path.as_deref().map(Path::new), remember)).await
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

#[tauri::command]
fn lock_forget_picture(state: State<'_, AppState>) -> CmdResult<()> {
    state.vault.forget_picture().map_err(|e| e.to_string())
}

#[tauri::command]
fn lock_lock(state: State<'_, AppState>) {
    state.vault.lock();
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
async fn scan_folders(app: AppHandle, state: State<'_, AppState>, folders: Vec<String>) -> CmdResult<ImportSummary> {
    state.licences.require(App::Clean)?;
    with_store(state.db()?, move |s| {
        let folders: Vec<PathBuf> = folders.into_iter().map(PathBuf::from).collect();
        sompio_clean::scan_folders(s, &folders, &mut |p| {
            let _ = app.emit("import-progress", &p);
        })
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
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // Updates: checks one small file on GitHub, installs only versions signed with our update key.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Local app data (not roaming): the database never syncs to other machines.
            let dir = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let secrets = std::sync::Arc::new(sompio_connect::KeyringStore::new("Sompio"));
            move_from_veil(&dir, &dir.join("clean.db"));
            app.manage(AppState {
                licences: Licences::load(dir.join("licences.json")),
                vault: Arc::new(sompio_suite::Vault::new(dir.clone(), secrets.clone())),
                telegram: tokio::sync::Mutex::new(sompio_connect::telegram::Telegram::new(secrets.clone())),
                email: std::sync::Arc::new(sompio_connect::email::Email::new(secrets)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            has_data,
            lock_status,
            lock_setup,
            lock_unlock,
            lock_recover,
            lock_change_password,
            lock_forget_picture,
            lock_lock,
            licence_status,
            licence_add,
            import_package,
            scan_folders,
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
            export_flagged,
            wipe_data,
            data_location,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Sompio");
}

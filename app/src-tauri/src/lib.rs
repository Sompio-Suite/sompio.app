//! The desktop shell. It only passes requests from the UI to veil-core and back.
//! Every command runs off the UI thread so the window never freezes.

use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, State};
use veil_core::{
    ChannelView, FindingView, FindingsPage, ImportSummary, MessageView, MessagesPage, ScanOptions,
    ScanSummary, SearchPage, SourceView, Stats, StatusFilter, Store,
};

struct AppState {
    db_path: PathBuf,
    /// The Telegram connection. Only does anything after the person connects an account.
    telegram: tokio::sync::Mutex<veil_connect::telegram::Telegram>,
    /// The email connection (IMAP). Blocking, so it always runs on a background thread.
    email: std::sync::Arc<veil_connect::email::Email>,
}

type CmdResult<T> = Result<T, String>;

/// Opens the database on a background thread, runs `f`, and reports errors as plain text.
async fn with_store<T, F>(db: PathBuf, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&mut Store) -> anyhow::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let mut store = Store::open(&db)?;
        f(&mut store)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn has_data(state: State<'_, AppState>) -> CmdResult<bool> {
    with_store(state.db_path.clone(), |s| s.has_data()).await
}

#[tauri::command]
async fn import_package(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<ImportSummary> {
    with_store(state.db_path.clone(), move |s| {
        veil_core::import_package(s, Path::new(&path), &mut |p| {
            let _ = app.emit("import-progress", &p);
        })
    })
    .await
}

#[tauri::command]
async fn scan_folders(app: AppHandle, state: State<'_, AppState>, folders: Vec<String>) -> CmdResult<ImportSummary> {
    with_store(state.db_path.clone(), move |s| {
        let folders: Vec<PathBuf> = folders.into_iter().map(PathBuf::from).collect();
        veil_core::scan_folders(s, &folders, &mut |p| {
            let _ = app.emit("import-progress", &p);
        })
    })
    .await
}

#[tauri::command]
async fn sources(state: State<'_, AppState>) -> CmdResult<Vec<SourceView>> {
    with_store(state.db_path.clone(), |s| s.sources()).await
}

#[tauri::command]
async fn remove_source(state: State<'_, AppState>, platform: String) -> CmdResult<()> {
    with_store(state.db_path.clone(), move |s| s.remove_source(&platform)).await
}

/// Moves a file found by the PC scan to the Recycle Bin.
#[tauri::command]
async fn recycle_file(state: State<'_, AppState>, id: i64) -> CmdResult<(String, ScanSummary)> {
    with_store(state.db_path.clone(), move |s| {
        let path = s.recycle_file(id)?;
        Ok((path, s.scan_summary()?))
    })
    .await
}

/// Opens File Explorer with the file selected. Only paths Veil itself found can be shown.
#[tauri::command]
async fn show_in_folder(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    let path = with_store(state.db_path.clone(), move |s| s.file_path_for(id)).await?;
    let path = path.ok_or_else(|| "This item isn't a file on this PC.".to_string())?;
    tauri_plugin_opener::reveal_item_in_dir(&path).map_err(|e| format!("Couldn't open the folder: {e}"))
}

// ---------------------------------------------------------------------------------
// Telegram (live connection)

use veil_connect::telegram::{LoginStep, Status as TgStatus, SyncProgress};

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

/// Downloads your own Telegram messages and replaces the Telegram data in Veil with them.
#[tauri::command]
async fn tg_sync(app: AppHandle, state: State<'_, AppState>) -> CmdResult<ImportSummary> {
    let channels = {
        let mut tg = state.telegram.lock().await;
        let app2 = app.clone();
        tg.fetch_own_messages(&mut move |p: SyncProgress| {
            let _ = app2.emit("tg-progress", &p);
        })
        .await
        .map_err(text_err)?
    };
    with_store(state.db_path.clone(), move |s| veil_core::import_live(s, "telegram", "Connected Telegram account", channels)).await
}

/// Deletes the message a finding is in, on Telegram, for everyone.
#[tauri::command]
async fn tg_delete(state: State<'_, AppState>, id: i64) -> CmdResult<ScanSummary> {
    let (platform, uid) = with_store(state.db_path.clone(), move |s| s.delete_target(id)).await?;
    if platform != "telegram" {
        return Err("This item isn't a Telegram message.".into());
    }
    state.telegram.lock().await.delete_message(&uid).await.map_err(text_err)?;
    with_store(state.db_path.clone(), move |s| {
        s.mark_deleted(id)?;
        s.scan_summary()
    })
    .await
}

// Email

use veil_connect::email::{Email, Provider, Status as EmailStatus, SyncProgress as EmailProgress};

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
    veil_connect::email::PROVIDERS.to_vec()
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
    let channels = with_email(&state, move |e| {
        e.fetch_sent(&mut |p: EmailProgress| {
            let _ = app.emit("email-progress", &p);
        })
    })
    .await?;
    with_store(state.db_path.clone(), move |s| veil_core::import_live(s, "email", "Connected email account", channels)).await
}

/// Moves the sent email a finding is in to the mailbox's Trash.
#[tauri::command]
async fn email_delete(state: State<'_, AppState>, id: i64) -> CmdResult<ScanSummary> {
    let (platform, uid) = with_store(state.db_path.clone(), move |s| s.delete_target(id)).await?;
    if platform != "email" {
        return Err("This item isn't an email.".into());
    }
    let ext = uid.strip_prefix("email:").unwrap_or(&uid).to_string();
    with_email(&state, move |e| e.delete_message(&ext)).await?;
    with_store(state.db_path.clone(), move |s| {
        s.mark_deleted(id)?;
        s.scan_summary()
    })
    .await
}

// The names someone goes by in chats, for exports that don't say which messages are theirs.

#[tauri::command]
async fn my_names(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    with_store(state.db_path.clone(), |s| s.my_names()).await
}

#[tauri::command]
async fn set_my_names(state: State<'_, AppState>, names: Vec<String>) -> CmdResult<()> {
    with_store(state.db_path.clone(), move |s| s.set_my_names(&names)).await
}

#[tauri::command]
async fn stats(state: State<'_, AppState>) -> CmdResult<Stats> {
    with_store(state.db_path.clone(), |s| s.stats()).await
}

#[tauri::command]
async fn channels(state: State<'_, AppState>, filter: Option<String>) -> CmdResult<Vec<ChannelView>> {
    with_store(state.db_path.clone(), move |s| s.channels(filter.as_deref())).await
}

#[tauri::command]
async fn messages(state: State<'_, AppState>, channel_id: String, offset: i64, limit: i64) -> CmdResult<MessagesPage> {
    with_store(state.db_path.clone(), move |s| s.messages(&channel_id, offset, limit)).await
}

#[tauri::command]
async fn message_index(state: State<'_, AppState>, channel_id: String, message_id: String) -> CmdResult<i64> {
    with_store(state.db_path.clone(), move |s| s.message_index(&channel_id, &message_id)).await
}

#[tauri::command]
async fn search(
    state: State<'_, AppState>,
    text: String,
    channel_id: Option<String>,
    offset: i64,
    limit: i64,
) -> CmdResult<SearchPage> {
    with_store(state.db_path.clone(), move |s| s.search(&text, channel_id.as_deref(), offset, limit)).await
}

#[tauri::command]
async fn run_scan(app: AppHandle, state: State<'_, AppState>, watch_terms: Vec<String>) -> CmdResult<ScanSummary> {
    with_store(state.db_path.clone(), move |s| {
        veil_core::run_scan(s, &ScanOptions { watch_terms }, &mut |done, total| {
            let _ = app.emit("scan-progress", (done, total));
        })
    })
    .await
}

#[tauri::command]
async fn scan_summary(state: State<'_, AppState>) -> CmdResult<ScanSummary> {
    with_store(state.db_path.clone(), |s| s.scan_summary()).await
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
    with_store(state.db_path.clone(), move |s| {
        let filter = veil_core::FindingsFilter {
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
async fn group_info(state: State<'_, AppState>, id: i64, status: String) -> CmdResult<veil_core::GroupInfo> {
    with_store(state.db_path.clone(), move |s| s.group_info(id, StatusFilter::parse(&status))).await
}

type Previous = Vec<(i64, Option<String>)>;

/// One decision for many findings. Returns what they were before, for Undo.
#[tauri::command]
async fn set_status_many(state: State<'_, AppState>, ids: Vec<i64>, status: Option<String>) -> CmdResult<(Previous, ScanSummary)> {
    with_store(state.db_path.clone(), move |s| {
        let before = s.set_status_many(&ids, status.as_deref())?;
        Ok((before, s.scan_summary()?))
    })
    .await
}

#[tauri::command]
async fn restore_statuses(state: State<'_, AppState>, previous: Previous) -> CmdResult<ScanSummary> {
    with_store(state.db_path.clone(), move |s| {
        s.restore_statuses(&previous)?;
        s.scan_summary()
    })
    .await
}

#[tauri::command]
async fn reveal_finding(state: State<'_, AppState>, id: i64) -> CmdResult<Option<FindingView>> {
    with_store(state.db_path.clone(), move |s| s.reveal_finding(id)).await
}

#[tauri::command]
async fn reveal_message(state: State<'_, AppState>, message_id: String) -> CmdResult<Option<MessageView>> {
    with_store(state.db_path.clone(), move |s| s.reveal_message(&message_id)).await
}

#[tauri::command]
async fn set_status(state: State<'_, AppState>, id: i64, status: Option<String>) -> CmdResult<ScanSummary> {
    with_store(state.db_path.clone(), move |s| {
        s.set_status(id, status.as_deref())?;
        s.scan_summary()
    })
    .await
}

#[tauri::command]
async fn ignore_everywhere(state: State<'_, AppState>, id: i64) -> CmdResult<(usize, ScanSummary)> {
    with_store(state.db_path.clone(), move |s| {
        let n = s.ignore_everywhere(id)?;
        Ok((n, s.scan_summary()?))
    })
    .await
}

#[tauri::command]
async fn clear_ignored(state: State<'_, AppState>) -> CmdResult<usize> {
    with_store(state.db_path.clone(), |s| s.clear_ignored()).await
}

#[tauri::command]
async fn load_demo(state: State<'_, AppState>) -> CmdResult<()> {
    with_store(state.db_path.clone(), |s| veil_core::load_demo(s)).await
}

#[tauri::command]
async fn is_demo(state: State<'_, AppState>) -> CmdResult<bool> {
    with_store(state.db_path.clone(), |s| s.is_demo()).await
}

#[tauri::command]
async fn watch_terms(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    with_store(state.db_path.clone(), |s| s.watch_terms()).await
}

#[tauri::command]
async fn export_flagged(state: State<'_, AppState>, path: String) -> CmdResult<usize> {
    with_store(state.db_path.clone(), move |s| s.export_flagged_csv(Path::new(&path))).await
}

#[tauri::command]
async fn wipe_data(state: State<'_, AppState>) -> CmdResult<()> {
    with_store(state.db_path.clone(), |s| s.wipe()).await
}

#[tauri::command]
fn data_location(state: State<'_, AppState>) -> String {
    state.db_path.display().to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Local app data (not roaming): the database never syncs to other machines.
            let dir = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let secrets = std::sync::Arc::new(veil_connect::KeyringStore::new("Veil"));
            app.manage(AppState {
                db_path: dir.join("veil.db"),
                telegram: tokio::sync::Mutex::new(veil_connect::telegram::Telegram::new(secrets.clone())),
                email: std::sync::Arc::new(veil_connect::email::Email::new(secrets)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            has_data,
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
        .expect("error while running Veil");
}

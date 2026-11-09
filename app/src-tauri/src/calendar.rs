//! Sompio Calendar in the shell: commands for the UI, the minute-by-minute reminder loop, the
//! pop-up window, and the tray icon. The logic itself lives in sompio-calendar.

use crate::{AppState, CmdResult};
use chrono::{Duration, Local, NaiveDate, NaiveDateTime};
use serde::Serialize;
use sompio_calendar::{AlertMode, CalendarStore, Occurrence, Reminders, RowInput, ScheduleView, Settings, Shown};
use sompio_licence::App;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

/// How many days ahead reminders are worked out, so they keep coming while Sompio is locked.
const AHEAD_DAYS: i64 = 7;

pub struct Calendar {
    reminders: Mutex<Reminders>,
    /// The pop-up look and sound, copied from the settings so they're known while locked
    popup_settings: Mutex<Settings>,
    /// Schedules with their own pop-up sound
    sounds: Mutex<std::collections::HashMap<i64, String>>,
    /// Something changed (or Sompio was unlocked): reload from the store on the next tick
    dirty: AtomicBool,
}

impl Calendar {
    pub fn new() -> Self {
        Self { reminders: Mutex::new(Reminders::new()), popup_settings: Mutex::new(Settings::default()), sounds: Mutex::new(Default::default()), dirty: AtomicBool::new(true) }
    }

    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::SeqCst);
    }
}

fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Opens the encrypted calendar on a background thread, runs `f`, and reports errors as plain text.
async fn with_cal<T, F>(state: &State<'_, AppState>, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&CalendarStore) -> anyhow::Result<T> + Send + 'static,
{
    let (path, key) = state.vault.calendar_db().map_err(err)?;
    tauri::async_runtime::spawn_blocking(move || {
        let store = CalendarStore::open_encrypted(&path, &key)?;
        f(&store)
    })
    .await
    .map_err(err)?
    .map_err(err)
}

/// Same, for changes: checks the licence first and reloads the reminders afterwards.
async fn change<T, F>(state: &State<'_, AppState>, f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce(&CalendarStore) -> anyhow::Result<T> + Send + 'static,
{
    state.licences.require(App::Calendar)?;
    let out = with_cal(state, f).await?;
    state.calendar.mark_dirty();
    Ok(out)
}

fn date(s: &str) -> CmdResult<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| format!("\"{s}\" isn't a date."))
}

#[derive(Serialize)]
pub struct Overview {
    schedules: Vec<ScheduleView>,
    occurrences: Vec<Occurrence>,
    settings: Settings,
}

#[tauri::command]
pub async fn cal_overview(state: State<'_, AppState>, from: String, to: String) -> CmdResult<Overview> {
    let (from, to) = (date(&from)?, date(&to)?);
    if (to - from).num_days() > 62 {
        return Err("Ask for two months at most.".into());
    }
    with_cal(&state, move |s| {
        Ok(Overview { schedules: s.load()?.into_iter().map(|l| l.view).collect(), occurrences: s.occurrences_between(from, to, &Local)?, settings: s.settings()? })
    })
    .await
}

/// The next few reminders. Works while Sompio is locked, from what was loaded before.
#[tauri::command]
pub fn cal_next(state: State<'_, AppState>, count: usize) -> CmdResult<Vec<Occurrence>> {
    let r = state.calendar.reminders.lock().map_err(err)?;
    Ok(r.next(now(), count.min(20)).into_iter().cloned().collect())
}

#[tauri::command]
pub async fn cal_add_rows(state: State<'_, AppState>, schedule_id: i64, rows: Vec<RowInput>) -> CmdResult<()> {
    change(&state, move |s| s.add_rows(schedule_id, &rows)).await
}

#[tauri::command]
pub async fn cal_update_row(state: State<'_, AppState>, schedule_id: i64, line: usize, row: RowInput) -> CmdResult<()> {
    change(&state, move |s| s.update_row(schedule_id, line, &row)).await
}

#[tauri::command]
pub async fn cal_remove_row(state: State<'_, AppState>, schedule_id: i64, line: usize) -> CmdResult<()> {
    change(&state, move |s| s.remove_row(schedule_id, line)).await
}

#[tauri::command]
pub async fn cal_skip_once(state: State<'_, AppState>, schedule_id: i64, day: String, time: String) -> CmdResult<()> {
    let d = date(&day)?;
    change(&state, move |s| s.skip_once(schedule_id, d, &time)).await
}

#[tauri::command]
pub async fn cal_import_file(state: State<'_, AppState>, path: String) -> CmdResult<i64> {
    change(&state, move |s| {
        let p = PathBuf::from(&path);
        let stem = p.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        s.import(&sompio_calendar::store::read_text(&p)?, &stem, Some(&path), false)
    })
    .await
}

#[tauri::command]
pub async fn cal_import_desk_reminders(state: State<'_, AppState>, folder: String) -> CmdResult<usize> {
    change(&state, move |s| s.import_desk_reminders(Path::new(&folder))).await
}

/// Saves a schedule as a CSV file anyone with Desk Reminders or Sompio can open.
#[tauri::command]
pub async fn cal_export(state: State<'_, AppState>, schedule_id: i64, path: String) -> CmdResult<()> {
    with_cal(&state, move |s| {
        let text = s.export(schedule_id)?;
        std::fs::write(&path, text.as_bytes())?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn cal_create_shared(state: State<'_, AppState>, name: String, timezone: String, groups: Vec<String>) -> CmdResult<i64> {
    change(&state, move |s| s.create_shared(&name, &timezone, &groups)).await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn cal_set_options(
    state: State<'_, AppState>,
    schedule_id: i64,
    colour: Option<String>,
    enabled: Option<bool>,
    sound: Option<String>,
    my_groups: Option<Vec<String>>,
    quiet_allow: Option<bool>,
) -> CmdResult<()> {
    // Turning a schedule on or off, its colour and your groups are fine without a licence: they
    // only change what you see, and someone whose access ended should be able to quiet things.
    let out = with_cal(&state, move |s| {
        // sound: "" = use the default sound
        let sound = sound.as_deref().map(|x| if x.is_empty() { None } else { Some(x) });
        s.set_options(schedule_id, colour.as_deref(), enabled, sound, my_groups.as_deref(), quiet_allow)
    })
    .await;
    state.calendar.mark_dirty();
    out
}

#[tauri::command]
pub async fn cal_rename(state: State<'_, AppState>, schedule_id: i64, name: String) -> CmdResult<()> {
    change(&state, move |s| s.rename(schedule_id, &name)).await
}

#[tauri::command]
pub async fn cal_remove_schedule(state: State<'_, AppState>, schedule_id: i64) -> CmdResult<()> {
    let out = with_cal(&state, move |s| s.remove_schedule(schedule_id)).await;
    state.calendar.mark_dirty();
    out
}

/// mode: "mute", "shout", "hide" or null to clear. day: null = every time.
#[tauri::command]
pub async fn cal_set_alert(state: State<'_, AppState>, key: String, day: Option<String>, mode: Option<String>) -> CmdResult<()> {
    let d = day.as_deref().map(date).transpose()?;
    let m = match mode.as_deref() {
        None => None,
        Some("mute") => Some(AlertMode::Mute),
        Some("shout") => Some(AlertMode::Shout),
        Some("hide") => Some(AlertMode::Hide),
        Some(other) => return Err(format!("Unknown alert \"{other}\".")),
    };
    let out = with_cal(&state, move |s| s.set_alert(&key, d, m)).await;
    state.calendar.mark_dirty();
    out
}

#[tauri::command]
pub async fn cal_set_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> CmdResult<()> {
    let s2 = settings.clone();
    with_cal(&state, move |s| s.set_settings(&s2)).await?;
    *state.calendar.popup_settings.lock().map_err(err)? = settings;
    state.calendar.mark_dirty();
    let _ = app.emit_to("popup", "calendar-popups", ());
    Ok(())
}

// The pop-up window

#[derive(Serialize)]
pub struct PopupState {
    shown: Shown,
    sound: String,
    sound_on: bool,
    volume: u8,
    seconds: u32,
    discreet: bool,
    sounds: std::collections::HashMap<i64, String>,
}

#[tauri::command]
pub fn cal_popups(state: State<'_, AppState>) -> CmdResult<PopupState> {
    let shown = state.calendar.reminders.lock().map_err(err)?.shown();
    let s = state.calendar.popup_settings.lock().map_err(err)?.clone();
    let sounds = state.calendar.sounds.lock().map_err(err)?.clone();
    Ok(PopupState { shown, sounds, sound: s.sound, sound_on: s.sound_on, volume: s.volume, seconds: s.popup_seconds, discreet: s.discreet })
}

/// action: "done", "snooze" (with minutes) or "close-all".
#[tauri::command]
pub fn cal_popup_action(app: AppHandle, state: State<'_, AppState>, id: u64, action: String, minutes: Option<i64>) -> CmdResult<()> {
    {
        let mut r = state.calendar.reminders.lock().map_err(err)?;
        match action.as_str() {
            "done" => r.dismiss(id),
            "snooze" => r.snooze(id, minutes.unwrap_or(5), now()),
            "close-all" => r.dismiss_all(),
            _ => return Err(format!("Unknown action \"{action}\".")),
        }
    }
    sync_popup(&app);
    Ok(())
}

/// Pause pop-ups for some minutes; null resumes.
#[tauri::command]
pub fn cal_pause(app: AppHandle, state: State<'_, AppState>, minutes: Option<i64>) -> CmdResult<Option<NaiveDateTime>> {
    let until = minutes.map(|m| now() + Duration::minutes(m.clamp(1, 24 * 60)));
    state.calendar.reminders.lock().map_err(err)?.pause(until);
    update_tray(&app);
    Ok(until)
}

#[tauri::command]
pub fn cal_paused(state: State<'_, AppState>) -> CmdResult<Option<NaiveDateTime>> {
    Ok(state.calendar.reminders.lock().map_err(err)?.shown().paused_until)
}

#[tauri::command]
pub fn cal_autostart(app: AppHandle) -> CmdResult<bool> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(err)
}

#[tauri::command]
pub fn cal_set_autostart(app: AppHandle, on: bool) -> CmdResult<()> {
    use tauri_plugin_autostart::ManagerExt;
    let l = app.autolaunch();
    if on { l.enable() } else { l.disable() }.map_err(err)
}

const POPUP_W: f64 = 380.0;
const POPUP_H: f64 = 150.0;
const MARGIN: f64 = 16.0;

/// Shows, resizes or closes the pop-up window to match the queue. Never takes focus.
fn sync_popup(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let (count, corner, hidden) = {
        let n = state.calendar.reminders.lock().map(|r| r.shown().popups.len()).unwrap_or(0);
        let (c, h) = state.calendar.popup_settings.lock().map(|s| (s.corner.clone(), s.hide_from_capture)).unwrap_or_default();
        (n, c, h)
    };
    let existing = app.get_webview_window("popup");
    if count == 0 {
        if let Some(w) = existing {
            let _ = w.close();
        }
        return;
    }
    let h = POPUP_H * count.min(3) as f64 + 8.0 * (count.min(3) as f64 - 1.0);
    let (x, y) = popup_position(app, &corner, h);
    match existing {
        Some(w) => {
            let _ = w.set_content_protected(hidden);
            let _ = w.set_size(tauri::LogicalSize::new(POPUP_W, h));
            let _ = w.set_position(tauri::LogicalPosition::new(x, y));
            let _ = w.emit("calendar-popups", ());
        }
        None => {
            let built = WebviewWindowBuilder::new(app, "popup", WebviewUrl::App("index.html#popup".into()))
                .title("Sompio reminder")
                .inner_size(POPUP_W, h)
                .position(x, y)
                .decorations(false)
                .resizable(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .focused(false)
                .shadow(true)
                // Screen shares and streams show nothing where the pop-up is
                .content_protected(hidden)
                .visible(true)
                .build();
            if let Err(e) = built {
                eprintln!("couldn't show the reminder: {e}");
            }
        }
    }
}

fn popup_position(app: &AppHandle, corner: &str, h: f64) -> (f64, f64) {
    let monitor = app.get_webview_window("main").and_then(|w| w.primary_monitor().ok().flatten()).or_else(|| app.primary_monitor().ok().flatten());
    let Some(m) = monitor else { return (MARGIN, MARGIN) };
    let scale = m.scale_factor();
    let area = m.work_area();
    let (ax, ay) = (area.position.x as f64 / scale, area.position.y as f64 / scale);
    let (aw, ah) = (area.size.width as f64 / scale, area.size.height as f64 / scale);
    let left = ax + MARGIN;
    let right = ax + aw - POPUP_W - MARGIN;
    let top = ay + MARGIN;
    let bottom = ay + ah - h - MARGIN;
    match corner {
        "top-left" => (left, top),
        "top-right" => (right, top),
        "bottom-left" => (left, bottom),
        _ => (right, bottom),
    }
}

/// Reloads the coming days from the encrypted store. Only works while Sompio is unlocked; while
/// it's locked the reminders already loaded keep running.
fn refresh(app: &AppHandle) -> anyhow::Result<()> {
    let state = app.state::<AppState>();
    let (path, key) = state.vault.calendar_db()?;
    let store = CalendarStore::open_encrypted(&path, &key)?;
    let today = Local::now().date_naive();
    let until = today + Duration::days(AHEAD_DAYS);
    let occ = store.occurrences_between(today - Duration::days(1), until, &Local)?;
    let settings = store.settings()?;
    let quiet = settings.quiet;
    let sounds = store.load()?.into_iter().filter_map(|l| l.view.sound.map(|s| (l.view.id, s))).collect();
    *state.calendar.sounds.lock().map_err(|e| anyhow::anyhow!("{e}"))? = sounds;
    *state.calendar.popup_settings.lock().map_err(|e| anyhow::anyhow!("{e}"))? = settings;
    state.calendar.reminders.lock().map_err(|e| anyhow::anyhow!("{e}"))?.replace(occ, until.and_hms_opt(23, 59, 0).unwrap_or_default(), quiet);
    Ok(())
}

/// The reminder loop: every few seconds, see what's due and show it.
pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let state = app.state::<AppState>();
            let unlocked = state.vault.status().map(|s| s.unlocked).unwrap_or(false);
            let needs = state.calendar.dirty.load(Ordering::SeqCst) || state.calendar.reminders.lock().map(|r| r.needs_refresh(now())).unwrap_or(false);
            if needs && unlocked {
                let a = app.clone();
                let ok = tauri::async_runtime::spawn_blocking(move || refresh(&a)).await.map(|r| r.is_ok()).unwrap_or(false);
                if ok {
                    state.calendar.dirty.store(false, Ordering::SeqCst);
                    let _ = app.emit("calendar-changed", ());
                }
            }
            let changed = state.calendar.reminders.lock().map(|mut r| r.tick(now())).unwrap_or(false);
            if changed {
                sync_popup(&app);
            }
            update_tray(&app);
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });
}

// The tray icon: Sompio keeps running there when the window is closed, so reminders still come.

pub fn tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Sompio", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Pause reminders for 1 hour", true, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume", "Resume reminders", true, None::<&str>)?;
    let lock = MenuItem::with_id(app, "lock", "Lock Sompio", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit (reminders stop)", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &pause, &resume, &sep, &lock, &quit])?;
    let mut b = TrayIconBuilder::with_id("sompio").menu(&menu).show_menu_on_left_click(false).tooltip("Sompio");
    if let Some(icon) = app.default_window_icon() {
        b = b.icon(icon.clone());
    }
    b.on_menu_event(|app, e| match e.id().as_ref() {
        "open" => show_main(app),
        "pause" => {
            let _ = cal_pause(app.clone(), app.state::<AppState>(), Some(60));
        }
        "resume" => {
            let _ = cal_pause(app.clone(), app.state::<AppState>(), None);
        }
        "lock" => {
            app.state::<AppState>().vault.lock();
            app.state::<AppState>().passwords.close();
            let _ = app.emit("locked", ());
        }
        "quit" => app.exit(0),
        _ => {}
    })
    .on_tray_icon_event(|tray, e| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            show_main(tray.app_handle());
        }
    })
    .build(app)?;
    Ok(())
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn update_tray(app: &AppHandle) {
    let Some(tray) = app.tray_by_id("sompio") else { return };
    let state = app.state::<AppState>();
    let discreet = state.calendar.popup_settings.lock().map(|s| s.discreet).unwrap_or(true);
    let text = state
        .calendar
        .reminders
        .lock()
        .map(|r| {
            let now = now();
            if let Some(p) = r.shown().paused_until {
                return format!("Sompio: reminders paused until {}", p.format("%H:%M"));
            }
            if !r.has_data() {
                return "Sompio: unlock to start your reminders".to_string();
            }
            match r.next(now, 1).first() {
                Some(o) if discreet => format!("Sompio: next reminder at {}", o.at.format(if o.at.date() == now.date() { "%H:%M" } else { "%a %H:%M" })),
                Some(o) if o.at.date() == now.date() => format!("Sompio: next {} at {}", o.title, o.at.format("%H:%M")),
                Some(o) => format!("Sompio: next {} on {}", o.title, o.at.format("%a %H:%M")),
                None => "Sompio: nothing coming up".to_string(),
            }
        })
        .unwrap_or_else(|_| "Sompio".into());
    let _ = tray.set_tooltip(Some(text));
}

/// Closing the window keeps Sompio in the tray while there are reminders to show.
pub fn keeps_running(app: &AppHandle) -> bool {
    app.try_state::<AppState>().is_some_and(|s| s.calendar.reminders.lock().map(|r| r.has_data()).unwrap_or(false))
}

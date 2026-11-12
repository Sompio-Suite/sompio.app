//! Sample files and sample data for testers: every import, every feature, nothing real.

use crate::{AppState, CmdResult};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

#[derive(Serialize)]
pub struct Written {
    folder: String,
    files: Vec<(String, String)>,
}

fn folder(app: &AppHandle, name: &str) -> CmdResult<PathBuf> {
    let docs = app.path().document_dir().or_else(|_| app.path().home_dir()).map_err(|e| e.to_string())?;
    Ok(docs.join("Sompio samples").join(name))
}

/// Writes one app's sample files to Documents\Sompio samples\<App> and opens the folder.
/// app: "clean", "calendar" or "vault"
#[tauri::command]
pub async fn samples_write(app: AppHandle, which: String) -> CmdResult<Written> {
    let (name, dir) = match which.as_str() {
        "clean" => ("Trace", folder(&app, "Trace")?),
        "calendar" => ("Calendar", folder(&app, "Calendar")?),
        "vault" => ("Vault", folder(&app, "Vault")?),
        other => return Err(format!("No samples for \"{other}\".")),
    };
    let d = dir.clone();
    let files = tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<Vec<(String, String)>> {
        Ok(match name {
            "Trace" => sompio_clean::samples::write_samples(&d)?.into_iter().map(|f| (f.file, format!("{}: {}", f.platform, f.what))).collect(),
            "Calendar" => sompio_calendar::samples::write_sample_files(&d)?.into_iter().map(|(f, w)| (f, w.to_string())).collect(),
            _ => sompio_vault::samples::write_sample_files(&d)?.into_iter().map(|(f, w)| (f, w.to_string())).collect(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("Couldn't write the samples: {e}"))?;
    let _ = tauri_plugin_opener::open_path(&dir, None::<&str>);
    Ok(Written { folder: dir.display().to_string(), files })
}

/// Adds the sample reminders (the first pops up within a minute). Remove the schedule to undo.
#[tauri::command]
pub async fn cal_add_samples(state: State<'_, AppState>) -> CmdResult<i64> {
    let (path, key) = state.vault.calendar_db().map_err(|e| e.to_string())?;
    let id = tauri::async_runtime::spawn_blocking(move || -> anyhow::Result<i64> {
        let store = sompio_calendar::CalendarStore::open_encrypted(&path, &key)?;
        if let Some(old) = store.load()?.into_iter().find(|l| l.view.name == sompio_calendar::samples::SAMPLE_NAME && l.view.owned) {
            store.remove_schedule(old.view.id)?;
        }
        sompio_calendar::samples::add_live_samples(&store, sompio_calendar::samples::now())
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    state.calendar.mark_dirty();
    Ok(id)
}

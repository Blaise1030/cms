//! The app's only IPC commands, for the bundled error and update pages. The
//! capability in `capabilities/` limits them to local pages; the daemon origin
//! can't call them.

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::logs;
use crate::supervisor::Supervisor;
use crate::updater;

/// Saves the log bundle to a path picked in a native save dialog. Returns the
/// path, or `None` if the user cancelled.
#[tauri::command]
pub async fn collect_logs(app: AppHandle) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(choice) = app
            .dialog()
            .file()
            .set_title("Save Workbench logs")
            .set_file_name("workbench-logs.zip")
            .add_filter("Zip archive", &["zip"])
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let path = choice.into_path().map_err(|e| e.to_string())?;
        if let Err(e) = logs::save_bundle(&path) {
            logs::shell(&format!("could not save log bundle: {e}"));
            return Err(e.to_string());
        }
        logs::shell("saved log bundle");
        Ok(Some(path.display().to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Restarts a daemon that was given up on, with a fresh restart budget.
#[tauri::command]
pub fn retry_daemon(app: AppHandle, supervisor: State<'_, Supervisor>) -> bool {
    supervisor.retry(&app)
}

/// Installs the downloaded update and relaunches, after stopping the daemon.
/// Errs with a message for the page if a terminal session is busy.
#[tauri::command]
pub async fn apply_update(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || updater::apply(&app))
        .await
        .map_err(|e| e.to_string())?
}

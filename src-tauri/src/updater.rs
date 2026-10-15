//! Background updates: check on launch and every 6 h, download quietly, then
//! tell the daemon page once no terminal session is busy. Nothing is applied
//! until the user clicks through the bundled `update.html` (`apply_update`).

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, SystemTime};

use serde_json::json;
use tauri::{AppHandle, Manager, Url};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::busy;
use crate::logs;
use crate::supervisor::Supervisor;

pub const READY_EVENT: &str = "workbench:update-ready";
const UPDATE_PAGE: &str = "tauri://localhost/update.html";
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
/// How often a downloaded update retries reaching the page while sessions are busy.
const DELIVER_INTERVAL: Duration = Duration::from_secs(15);
const MAIN_WINDOW: &str = "main";
const BUSY_MESSAGE: &str =
    "A terminal session is still working. Let it finish (or stop it), then try again.";

/// Managed app state.
#[derive(Default)]
pub struct Updates {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    pending: Option<Pending>,
    /// The notice reached the page currently loaded in the main window.
    delivered: bool,
    deferral_logged: bool,
}

struct Pending {
    update: Update,
    bytes: Arc<Vec<u8>>,
}

impl Updates {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Starts the check/deliver loop on its own thread.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        let mut next_check = SystemTime::now();
        loop {
            if SystemTime::now() >= next_check {
                tauri::async_runtime::block_on(check(&app));
                next_check = SystemTime::now() + CHECK_INTERVAL;
            }
            deliver(&app);
            thread::sleep(DELIVER_INTERVAL);
        }
    });
}

/// A page finished loading in the main window; a new daemon page needs the
/// notice again.
pub fn page_loaded(app: &AppHandle) {
    let Some(updates) = app.try_state::<Updates>() else {
        return;
    };
    let pending = {
        let mut state = updates.state();
        state.delivered = false;
        state.pending.is_some()
    };
    if pending {
        let app = app.clone();
        thread::spawn(move || deliver(&app));
    }
}

async fn check(app: &AppHandle) {
    let update = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    let update = match update {
        Ok(Some(update)) => update,
        Ok(None) => return logs::shell("update check: Workbench is up to date"),
        Err(e) => return logs::shell(&format!("update check failed: {e}")),
    };
    let updates = app.state::<Updates>();
    if updates
        .state()
        .pending
        .as_ref()
        .is_some_and(|p| p.update.version == update.version)
    {
        return;
    }
    logs::shell(&format!("downloading update {}", update.version));
    match update.download(|_, _| {}, || {}).await {
        Ok(bytes) => {
            logs::shell(&format!("update {} downloaded", update.version));
            let mut state = updates.state();
            state.pending = Some(Pending {
                update,
                bytes: Arc::new(bytes),
            });
            state.delivered = false;
            state.deferral_logged = false;
        }
        Err(e) => logs::shell(&format!("update download failed: {e}")),
    }
}

/// Shows the notice in the daemon page unless it's already there or a
/// terminal session is busy.
fn deliver(app: &AppHandle) {
    let Some(updates) = app.try_state::<Updates>() else {
        return;
    };
    let version = {
        let state = updates.state();
        match &state.pending {
            Some(p) if !state.delivered => p.update.version.clone(),
            _ => return,
        }
    };
    let Some(handle) = app.state::<Supervisor>().handle() else {
        return;
    };
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    let showing_daemon = match (window.url(), Url::parse(&handle.base_url)) {
        (Ok(page), Ok(daemon)) => page.origin() == daemon.origin(),
        _ => false,
    };
    if !showing_daemon {
        return;
    }
    if busy::daemon_busy(&handle.base_url, &handle.token) {
        let mut state = updates.state();
        if !state.deferral_logged {
            state.deferral_logged = true;
            logs::shell(&format!(
                "update {version} ready; waiting for terminal sessions to go idle"
            ));
        }
        return;
    }
    match window.eval(notice_script(&version)) {
        Ok(()) => {
            updates.state().delivered = true;
            logs::shell(&format!("update {version} ready; notice shown"));
        }
        Err(e) => logs::shell(&format!("could not show the update notice: {e}")),
    }
}

/// Script run in the daemon page to announce a downloaded update.
pub fn notice_script(version: &str) -> String {
    let mut apply = Url::parse(UPDATE_PAGE).expect("valid update page URL");
    apply.query_pairs_mut().append_pair("version", version);
    let detail = json!({ "version": version, "applyUrl": apply.as_str() });
    format!(
        "window.__WORKBENCH_UPDATE__ = {detail};\
         window.dispatchEvent(new CustomEvent({event}, {{ detail: window.__WORKBENCH_UPDATE__ }}));",
        event = json!(READY_EVENT),
    )
}

/// Refuses while a session is busy; otherwise stops the daemon, installs the
/// downloaded update and relaunches. Blocks for up to the daemon's shutdown
/// grace, so call it off the main thread.
pub fn apply(app: &AppHandle) -> Result<(), String> {
    let (update, bytes) = app
        .state::<Updates>()
        .state()
        .pending
        .as_ref()
        .map(|p| (p.update.clone(), p.bytes.clone()))
        .ok_or("No update has been downloaded yet.")?;
    let supervisor = app.state::<Supervisor>();
    match supervisor.handle() {
        Some(handle) => {
            if busy::daemon_busy(&handle.base_url, &handle.token) {
                return Err(BUSY_MESSAGE.into());
            }
            handle.shutdown();
        }
        // Restarting or given up on: no sessions to protect.
        None => supervisor.shutdown(),
    }
    logs::shell(&format!("installing update {}", update.version));
    if let Err(e) = update.install(bytes.as_slice()) {
        // The daemon is already down; relaunching brings back the current
        // version and the next check retries the update.
        logs::shell(&format!("update install failed: {e}"));
    }
    app.restart()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notice_script_dispatches_the_ready_event_with_the_update_page() {
        let script = notice_script("1.2.3");
        assert!(script.contains(READY_EVENT), "{script}");
        assert!(script.contains(r#""version":"1.2.3""#), "{script}");
        assert!(
            script.contains(r#""applyUrl":"tauri://localhost/update.html?version=1.2.3""#),
            "{script}"
        );
        assert!(script.contains("window.__WORKBENCH_UPDATE__"), "{script}");
    }

    #[test]
    fn notice_script_escapes_the_version() {
        let script = notice_script(r#"1"});alert(1);//"#);
        assert!(!script.contains(r#"1"});alert"#), "{script}");
    }
}

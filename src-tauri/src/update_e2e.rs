//! Test-only hooks for `scripts/update-e2e.sh`, compiled only with the
//! non-default `update-e2e` feature, so release builds never contain them.
//! Driven by args, which survive the updater's relaunch through LaunchServices
//! (the environment does not).

use std::fs::OpenOptions;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::logs;
use crate::supervisor::Supervisor;
use crate::updater;

const HOME_ARG: &str = "--update-e2e-home";
const LOG_ARG: &str = "--update-e2e-log";
const AUTO_APPLY_ENV: &str = "WORKBENCH_DESKTOP_UPDATE_E2E_AUTO_APPLY";
const DAEMON_FILE: &str = "update-e2e-daemon.json";

fn arg(name: &str) -> Option<String> {
    let mut args = std::env::args();
    while let Some(a) = args.next() {
        if a == name {
            return args.next();
        }
    }
    None
}

/// Call before anything else: `--update-e2e-home <dir>` becomes `HOME` (the
/// daemon keeps its projects and agents config there) and
/// `--update-e2e-log <file>` receives stderr, where the shell logs go.
pub fn init() {
    if let Some(home) = arg(HOME_ARG) {
        std::env::set_var("HOME", home);
    }
    if let Some(path) = arg(LOG_ARG) {
        if let Ok(file) = OpenOptions::new().create(true).append(true).open(path) {
            // SAFETY: plain fd duplication; `file` stays open until after the call.
            unsafe {
                libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO);
            }
        }
    }
}

/// Logs the running version and keeps `$HOME/update-e2e-daemon.json` holding
/// the current daemon's address and token, for the harness to drive it.
pub fn start(app: &AppHandle) {
    if arg(HOME_ARG).is_none() {
        return;
    }
    logs::shell(&format!(
        "update-e2e: running version {}",
        app.package_info().version
    ));
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let path = PathBuf::from(home).join(DAEMON_FILE);
    let app = app.clone();
    thread::spawn(move || {
        let mut last = None;
        loop {
            let current = app
                .state::<Supervisor>()
                .handle()
                .map(|h| (h.base_url, h.token));
            if current != last {
                if let Some((base_url, token)) = &current {
                    let body = serde_json::json!({ "base_url": base_url, "token": token });
                    let tmp = path.with_extension("tmp");
                    if std::fs::write(&tmp, body.to_string()).is_ok() {
                        let _ = std::fs::rename(&tmp, &path);
                    }
                }
                last = current;
            }
            thread::sleep(Duration::from_millis(250));
        }
    });
}

/// With `WORKBENCH_DESKTOP_UPDATE_E2E_AUTO_APPLY=1`, keeps doing what the
/// update page's button does (busy re-check included) until it applies.
pub fn downloaded(app: &AppHandle) {
    if std::env::var(AUTO_APPLY_ENV).as_deref() != Ok("1") {
        return;
    }
    let app = app.clone();
    thread::spawn(move || loop {
        if let Err(e) = updater::apply(&app) {
            logs::shell(&format!("update-e2e: apply refused: {e}"));
        }
        thread::sleep(Duration::from_secs(2));
    });
}

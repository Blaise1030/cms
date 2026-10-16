mod backoff;
mod health;
mod ready;

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use tauri::async_runtime::Receiver;
use tauri::{AppHandle, Emitter, Manager, Url};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

use crate::logs;
use backoff::Backoff;
use ready::ReadyInfo;

pub const FAILED_EVENT: &str = "daemon://failed";
const SIDECAR: &str = "workbench";
const READY_TIMEOUT: Duration = Duration::from_secs(15);
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);
const MAIN_WINDOW: &str = "main";
/// Origin of the bundled `ui/` pages on macOS.
const LOCAL_PAGES: &str = "tauri://localhost/";
/// When set, the log bundle is also written here whenever the daemon is
/// given up on, for checks that can't drive the save dialog.
const BUNDLE_ON_FAILURE_ENV: &str = "WORKBENCH_DESKTOP_LOG_BUNDLE_ON_FAILURE";

#[derive(Debug)]
pub enum SupervisorError {
    Spawn(String),
    ExitedBeforeReady,
    ReadyTimeout,
    HealthTimeout,
    Stopped,
}

impl fmt::Display for SupervisorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(e) => write!(f, "could not start the workbench service: {e}"),
            Self::ExitedBeforeReady => write!(f, "the workbench service exited during startup"),
            Self::ReadyTimeout => write!(f, "the workbench service did not report ready in time"),
            Self::HealthTimeout => write!(f, "the workbench service did not become healthy in time"),
            Self::Stopped => write!(f, "the workbench service was stopped"),
        }
    }
}

impl std::error::Error for SupervisorError {}

/// Any failure seen while shutting down is reported as `Stopped`, so quitting
/// never looks like a crash.
fn settle(err: SupervisorError, stopping: bool) -> SupervisorError {
    if stopping {
        SupervisorError::Stopped
    } else {
        err
    }
}

/// The running daemon's address. `shutdown` stops the daemon for good: it is
/// not restarted afterwards.
#[derive(Clone)]
pub struct DaemonHandle {
    pub base_url: String,
    pub token: String,
    control: Arc<Control>,
}

impl DaemonHandle {
    pub fn shutdown(&self) {
        self.control.shutdown();
    }
}

/// Managed app state; lets quit handlers stop the daemon even mid-startup.
/// Manage it before `start` runs so a quit at any point finds it.
#[derive(Default)]
pub struct Supervisor {
    control: Arc<Control>,
}

impl Supervisor {
    /// Spawns the daemon, waits until it is healthy, navigates the main window
    /// to it and keeps it alive with restarts. Blocks until healthy, so call it
    /// off the main thread. Errors after the restart budget is spent, having
    /// emitted `daemon://failed` and shown the error page.
    pub fn start(app: &AppHandle) -> Result<DaemonHandle, SupervisorError> {
        let control = match app.try_state::<Supervisor>() {
            Some(supervisor) => supervisor.control.clone(),
            None => {
                let supervisor = Supervisor::default();
                let control = supervisor.control.clone();
                app.manage(supervisor);
                control
            }
        };

        let mut backoff = Backoff::default();
        let (info, exits) = launch_with_retries(app, &control, &mut backoff)?;
        let handle = control.publish(app, info);

        let app = app.clone();
        let watcher = control.clone();
        thread::spawn(move || supervise(&app, &watcher, backoff, exits));
        Ok(handle)
    }

    /// After the daemon was given up on: shows the splash and starts again
    /// with a fresh restart budget. Returns false if there was nothing to retry.
    pub fn retry(&self, app: &AppHandle) -> bool {
        if self.control.is_stopping() || !self.control.failed.swap(false, Ordering::SeqCst) {
            return false;
        }
        logs::shell("retrying the workbench service");
        show_page(app, "splash.html", None);
        let app = app.clone();
        thread::spawn(move || {
            if let Err(e) = Supervisor::start(&app) {
                logs::shell(&format!("daemon unavailable: {e}"));
            }
        });
        true
    }

    pub fn handle(&self) -> Option<DaemonHandle> {
        let current = lock(&self.control.current).clone()?;
        Some(DaemonHandle {
            base_url: current.base_url,
            token: current.token,
            control: self.control.clone(),
        })
    }

    pub fn shutdown(&self) {
        self.control.shutdown();
    }
}

#[derive(Default)]
struct Control {
    /// Guarded together with `child` so a spawn can't slip past `shutdown`.
    child: Mutex<ChildSlot>,
    current: Mutex<Option<ReadyInfo>>,
    /// Set once the restart budget is spent; cleared by `retry`.
    failed: AtomicBool,
}

#[derive(Default)]
struct ChildSlot {
    stopping: bool,
    running: Option<Running>,
}

struct Running {
    child: CommandChild,
    exited: Arc<ExitSignal>,
}

impl Control {
    fn is_stopping(&self) -> bool {
        lock(&self.child).stopping
    }

    fn shutdown(&self) {
        let running = {
            let mut slot = lock(&self.child);
            slot.stopping = true;
            slot.running.take()
        };
        if let Some(running) = running {
            terminate(running);
        }
    }

    fn stop_current(&self) {
        *lock(&self.current) = None;
        let running = lock(&self.child).running.take();
        if let Some(running) = running {
            terminate(running);
        }
    }

    fn publish(self: &Arc<Self>, app: &AppHandle, info: ReadyInfo) -> DaemonHandle {
        let ui = crate::dev::plan().ui;
        let ui_base = ui.as_ref().map(|u| u.as_str().trim_end_matches('/'));
        let origins = [Some(info.base_url.as_str()), ui_base];
        for origin in origins.into_iter().flatten() {
            if let Err(e) = crate::drag::allow_origin(app, origin) {
                logs::shell(&format!("window dragging unavailable: {e}"));
            }
        }
        navigate(app, ui_base.unwrap_or(&info.base_url), &info.token);
        *lock(&self.current) = Some(info.clone());
        DaemonHandle {
            base_url: info.base_url,
            token: info.token,
            control: self.clone(),
        }
    }

    /// Gives up on the daemon: tells listeners and swaps in the error page.
    fn fail(&self, app: &AppHandle, message: String) {
        if self.is_stopping() {
            return;
        }
        *lock(&self.current) = None;
        self.failed.store(true, Ordering::SeqCst);
        logs::shell(&message);
        let _ = app.emit(FAILED_EVENT, message.clone());
        if let Some(path) = std::env::var_os(BUNDLE_ON_FAILURE_ENV) {
            match logs::save_bundle(path.as_ref()) {
                Ok(()) => logs::shell("wrote log bundle on failure"),
                Err(e) => logs::shell(&format!("could not write log bundle: {e}")),
            }
        }
        show_page(app, "error.html", Some(&message));
    }
}

#[derive(Default)]
struct ExitSignal {
    exited: Mutex<bool>,
    cond: Condvar,
}

impl ExitSignal {
    fn set(&self) {
        *lock(&self.exited) = true;
        self.cond.notify_all();
    }

    fn is_set(&self) -> bool {
        *lock(&self.exited)
    }

    fn wait(&self, timeout: Duration) -> bool {
        let guard = lock(&self.exited);
        let (guard, _) = self
            .cond
            .wait_timeout_while(guard, timeout, |exited| !*exited)
            .unwrap_or_else(PoisonError::into_inner);
        *guard
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// SIGTERM first so the daemon can reap its PTY children; SIGKILL only if it
/// is still alive after the grace period.
fn terminate(running: Running) {
    let Running { child, exited } = running;
    if exited.is_set() {
        return;
    }
    let pid = child.pid() as libc::pid_t;
    // SAFETY: `kill` has no memory-safety preconditions; the risk is signalling
    // a reused pid. An unset exit signal does not prove the daemon is unreaped:
    // the shell plugin sends `Terminated` only after it has waited on the child
    // *and* both pipe readers have hit EOF (tauri-plugin-shell 2.5.0,
    // src/process/mod.rs:397-404). This relies on nothing outliving the daemon
    // with its stdout/stderr — its dev servers are piped, one-shot commands use
    // `.output()` and PTY children get PTY fds — so EOF follows the reap within
    // milliseconds, far too soon for the pid to be reused.
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    if !exited.wait(SHUTDOWN_GRACE) {
        logs::shell(&format!("daemon ignored SIGTERM for {SHUTDOWN_GRACE:?}, killing"));
        let _ = child.kill();
    }
}

fn launch_with_retries(
    app: &AppHandle,
    control: &Arc<Control>,
    backoff: &mut Backoff,
) -> Result<(ReadyInfo, mpsc::Receiver<()>), SupervisorError> {
    loop {
        let err = match launch(app, control) {
            Ok(launched) => return Ok(launched),
            Err(SupervisorError::Stopped) => return Err(SupervisorError::Stopped),
            Err(e) => e,
        };
        logs::shell(&format!("daemon start failed: {err}"));
        match backoff.next_delay() {
            Some(delay) => {
                if !sleep_unless_stopping(control, delay) {
                    return Err(SupervisorError::Stopped);
                }
            }
            None => {
                control.fail(
                    app,
                    format!("Workbench's background service failed to start after 3 restart attempts ({err})."),
                );
                return Err(err);
            }
        }
    }
}

/// One spawn → ready line → healthy attempt. The returned receiver yields
/// once when the daemon exits.
fn launch(
    app: &AppHandle,
    control: &Control,
) -> Result<(ReadyInfo, mpsc::Receiver<()>), SupervisorError> {
    let exited = Arc::new(ExitSignal::default());
    let (ready_tx, ready_rx) = mpsc::channel();
    let (exit_tx, exit_rx) = mpsc::channel();
    {
        let mut slot = lock(&control.child);
        if slot.stopping {
            return Err(SupervisorError::Stopped);
        }
        let plan = crate::dev::plan();
        let command = match &plan.daemon {
            Some(path) => Ok(app.shell().command(path)),
            None => app.shell().sidecar(SIDECAR),
        };
        let (events, child) = command
            .and_then(|cmd| cmd.args(["serve", "--port", &plan.port]).spawn())
            .map_err(|e| SupervisorError::Spawn(e.to_string()))?;
        logs::shell(&format!("spawned daemon pid {}", child.pid()));
        let pump_exited = exited.clone();
        thread::spawn(move || pump(events, ready_tx, exit_tx, &pump_exited));
        slot.running = Some(Running {
            child,
            exited: exited.clone(),
        });
    }

    let result = match ready_rx.recv_timeout(READY_TIMEOUT) {
        Ok(info) => {
            if health::wait_healthy(info.port, || exited.is_set() || control.is_stopping()) {
                Ok(info)
            } else if exited.is_set() {
                Err(SupervisorError::ExitedBeforeReady)
            } else {
                Err(SupervisorError::HealthTimeout)
            }
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(SupervisorError::ExitedBeforeReady),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(SupervisorError::ReadyTimeout),
    };
    match result {
        Ok(info) => {
            logs::shell(&format!("daemon healthy at {}", info.base_url));
            Ok((info, exit_rx))
        }
        Err(e) => {
            control.stop_current();
            Err(settle(e, control.is_stopping()))
        }
    }
}

/// Drains the child's stdout/stderr into the daemon log for its whole life so
/// its pipes never fill, reports the ready line once, and signals exit.
fn pump(
    mut events: Receiver<CommandEvent>,
    ready_tx: mpsc::Sender<ReadyInfo>,
    exit_tx: mpsc::Sender<()>,
    exited: &ExitSignal,
) {
    let mut ready_tx = Some(ready_tx);
    while let Some(event) = events.blocking_recv() {
        match event {
            CommandEvent::Stdout(bytes) => {
                let line = String::from_utf8_lossy(&bytes);
                if ready::is_ready_line(&line) {
                    if let Some(info) = ready::parse_ready_line(&line) {
                        logs::add_secret(&info.token);
                        if let Some(tx) = ready_tx.take() {
                            let _ = tx.send(info);
                        }
                    }
                }
                logs::daemon(line.trim_end());
            }
            CommandEvent::Stderr(bytes) => logs::daemon(String::from_utf8_lossy(&bytes).trim_end()),
            CommandEvent::Error(e) => logs::shell(&format!("daemon pipe error: {e}")),
            CommandEvent::Terminated(payload) => {
                logs::shell(&format!(
                    "daemon exited (code {:?}, signal {:?})",
                    payload.code, payload.signal
                ));
                break;
            }
            _ => {}
        }
    }
    exited.set();
    let _ = exit_tx.send(());
}

/// Watches a healthy daemon and restarts it on unexpected exit.
fn supervise(
    app: &AppHandle,
    control: &Arc<Control>,
    mut backoff: Backoff,
    mut exits: mpsc::Receiver<()>,
) {
    loop {
        let healthy_since = Instant::now();
        let _ = exits.recv();
        if control.is_stopping() {
            return;
        }
        control.stop_current();
        if Backoff::was_stable(healthy_since.elapsed()) {
            backoff.reset();
        }
        logs::shell("daemon exited unexpectedly, restarting");
        // The exit that brought us here counts as a failure.
        let Some(delay) = backoff.next_delay() else {
            control.fail(
                app,
                "Workbench's background service keeps crashing and was not restarted after 3 attempts.".to_owned(),
            );
            return;
        };
        if !sleep_unless_stopping(control, delay) {
            return;
        }
        match launch_with_retries(app, control, &mut backoff) {
            Ok((info, next)) => {
                control.publish(app, info);
                exits = next;
            }
            Err(_) => return,
        }
    }
}

/// Returns false if shutdown began while sleeping.
fn sleep_unless_stopping(control: &Control, delay: Duration) -> bool {
    let deadline = Instant::now() + delay;
    while Instant::now() < deadline {
        if control.is_stopping() {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
    !control.is_stopping()
}

/// `base` is the daemon, or Vite's UI in `just desktop-dev`.
fn navigate(app: &AppHandle, base: &str, token: &str) {
    match Url::parse(&format!("{base}/?t={token}")) {
        Ok(url) => navigate_main(app, url, base),
        Err(e) => logs::shell(&format!("bad daemon URL: {e}")),
    }
}

/// Shows a bundled `ui/` page, optionally passing a `message` query param.
fn show_page(app: &AppHandle, page: &str, message: Option<&str>) {
    let mut url = match Url::parse(LOCAL_PAGES).and_then(|base| base.join(page)) {
        Ok(url) => url,
        Err(e) => {
            logs::shell(&format!("bad page URL {page}: {e}"));
            return;
        }
    };
    if let Some(message) = message {
        url.query_pairs_mut().append_pair("message", message);
    }
    navigate_main(app, url, page);
}

/// `label` is what gets logged instead of the URL, which may carry the token.
fn navigate_main(app: &AppHandle, url: Url, label: &str) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        logs::shell("main window missing, cannot navigate");
        return;
    };
    match window.navigate(url) {
        Ok(()) => logs::shell(&format!("navigating main window to {label}")),
        Err(e) => logs::shell(&format!("navigation failed: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_while_stopping_settle_as_stopped() {
        for err in [
            SupervisorError::ExitedBeforeReady,
            SupervisorError::ReadyTimeout,
            SupervisorError::HealthTimeout,
            SupervisorError::Spawn("x".into()),
        ] {
            assert!(matches!(settle(err, true), SupervisorError::Stopped));
        }
    }

    #[test]
    fn errors_while_running_are_kept() {
        assert!(matches!(
            settle(SupervisorError::ExitedBeforeReady, false),
            SupervisorError::ExitedBeforeReady
        ));
        assert!(matches!(
            settle(SupervisorError::HealthTimeout, false),
            SupervisorError::HealthTimeout
        ));
    }
}

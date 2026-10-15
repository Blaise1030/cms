mod backoff;
mod health;
mod ready;

use std::fmt;
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use tauri::async_runtime::Receiver;
use tauri::{AppHandle, Emitter, Manager, Url};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

use backoff::Backoff;
use ready::ReadyInfo;

pub const FAILED_EVENT: &str = "daemon://failed";
const SIDECAR: &str = "workbench";
const READY_TIMEOUT: Duration = Duration::from_secs(15);
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);
const MAIN_WINDOW: &str = "main";

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
pub struct Supervisor {
    control: Arc<Control>,
}

impl Supervisor {
    /// Spawns the daemon, waits until it is healthy, navigates the main window
    /// to it and keeps it alive with restarts. Blocks until healthy, so call it
    /// off the main thread. Errors after the restart budget is spent, having
    /// emitted `daemon://failed`.
    pub fn start(app: &AppHandle) -> Result<DaemonHandle, SupervisorError> {
        let control = Arc::new(Control::default());
        app.manage(Supervisor {
            control: control.clone(),
        });

        let mut backoff = Backoff::default();
        let (info, exits) = launch_with_retries(app, &control, &mut backoff)?;
        let handle = control.publish(app, info);

        let app = app.clone();
        let watcher = control.clone();
        thread::spawn(move || supervise(&app, &watcher, backoff, exits));
        Ok(handle)
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
        let running = lock(&self.child).running.take();
        if let Some(running) = running {
            terminate(running);
        }
    }

    fn publish(self: &Arc<Self>, app: &AppHandle, info: ReadyInfo) -> DaemonHandle {
        navigate(app, &info);
        *lock(&self.current) = Some(info.clone());
        DaemonHandle {
            base_url: info.base_url,
            token: info.token,
            control: self.clone(),
        }
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
    // SAFETY: `pid` is our own unreaped child (its exit signal is unset).
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    if !exited.wait(SHUTDOWN_GRACE) {
        eprintln!("workbench-desktop: daemon ignored SIGTERM for {SHUTDOWN_GRACE:?}, killing");
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
        eprintln!("workbench-desktop: daemon start failed: {err}");
        match backoff.next_delay() {
            Some(delay) => {
                if !sleep_unless_stopping(control, delay) {
                    return Err(SupervisorError::Stopped);
                }
            }
            None => {
                let message = format!(
                    "Workbench's background service failed to start after 3 restart attempts ({err})."
                );
                eprintln!("workbench-desktop: {message}");
                let _ = app.emit(FAILED_EVENT, message);
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
        let (events, child) = app
            .shell()
            .sidecar(SIDECAR)
            .and_then(|cmd| cmd.args(["serve", "--port", "0"]).spawn())
            .map_err(|e| SupervisorError::Spawn(e.to_string()))?;
        eprintln!("workbench-desktop: spawned daemon pid {}", child.pid());
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
            } else if control.is_stopping() {
                Err(SupervisorError::Stopped)
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
            eprintln!("workbench-desktop: daemon healthy at {}", info.base_url);
            Ok((info, exit_rx))
        }
        Err(e) => {
            control.stop_current();
            Err(e)
        }
    }
}

/// Drains the child's stdout/stderr for its whole life so its pipes never
/// fill, reports the ready line once, and signals exit.
fn pump(
    mut events: Receiver<CommandEvent>,
    ready_tx: mpsc::Sender<ReadyInfo>,
    exit_tx: mpsc::Sender<()>,
    exited: &ExitSignal,
) {
    let mut ready_tx = Some(ready_tx);
    let mut token: Option<String> = None;
    let redact = |line: &str, token: &Option<String>| match token {
        Some(t) => line.replace(t.as_str(), "<redacted>"),
        None => line.to_owned(),
    };
    while let Some(event) = events.blocking_recv() {
        match event {
            CommandEvent::Stdout(bytes) => {
                let line = String::from_utf8_lossy(&bytes);
                if ready::is_ready_line(&line) {
                    if let (Some(info), Some(tx)) = (ready::parse_ready_line(&line), ready_tx.take()) {
                        token = Some(info.token.clone());
                        let _ = tx.send(info);
                    }
                    continue;
                }
                eprintln!("[daemon] {}", redact(line.trim_end(), &token));
            }
            CommandEvent::Stderr(bytes) => {
                let line = String::from_utf8_lossy(&bytes);
                eprintln!("[daemon] {}", redact(line.trim_end(), &token));
            }
            CommandEvent::Error(e) => eprintln!("workbench-desktop: daemon pipe error: {e}"),
            CommandEvent::Terminated(payload) => {
                eprintln!(
                    "workbench-desktop: daemon exited (code {:?}, signal {:?})",
                    payload.code, payload.signal
                );
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
        eprintln!("workbench-desktop: daemon exited unexpectedly, restarting");
        // The exit that brought us here counts as a failure.
        let Some(delay) = backoff.next_delay() else {
            let message = "Workbench's background service keeps crashing and was not restarted after 3 attempts.".to_owned();
            eprintln!("workbench-desktop: {message}");
            let _ = app.emit(FAILED_EVENT, message);
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

fn navigate(app: &AppHandle, info: &ReadyInfo) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        eprintln!("workbench-desktop: main window missing, cannot navigate");
        return;
    };
    let url = match Url::parse(&format!("{}/?t={}", info.base_url, info.token)) {
        Ok(url) => url,
        Err(e) => {
            eprintln!("workbench-desktop: bad daemon URL: {e}");
            return;
        }
    };
    match window.navigate(url) {
        Ok(()) => eprintln!("workbench-desktop: navigating main window to {}", info.base_url),
        Err(e) => eprintln!("workbench-desktop: navigation failed: {e}"),
    }
}

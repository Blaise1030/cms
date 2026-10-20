//! Dev-server supervision: one dev server per project, run from the `dev`
//! command in `orso.json` (default `npm run dev`) in the configured root.
//!
//! A supervisor task per project spawns the dev server on the project's port with
//! `PUBLIC_BASE=/preview/{id}/`, marks it ready when stdout prints its local URL, restarts
//! it after crashes (at most [`MAX_RESTARTS`] times, with exponential backoff), and
//! publishes every transition as a `devserver.status` event. An invalid `orso.json`
//! parks it as `crashed` until the file loads again; a spent crash budget parks it
//! until its packages change (a sync or change switch), which reinstalls and retries.
//!
//! The server runs in its own process group so the package manager's node grandchild dies with it.
//! Lifecycle: it runs from project creation (or daemon start for existing projects) until
//! the project is deleted or the daemon shuts down, plus manual restarts.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::error::{ApiError, ErrorCode};
use crate::events::EventBus;
use crate::projects::{DevserverStatus, ProjectRegistry};

/// Crash restarts before the server is parked as `crashed` (until a manual restart or,
/// for the configured launcher, a change to its packages).
pub const MAX_RESTARTS: u32 = 3;
/// First restart delay; doubles per consecutive crash (1s, 2s, 4s).
const BACKOFF_BASE: Duration = Duration::from_secs(1);
/// A server that was ready for this long before dying starts a fresh crash budget.
const STABLE_AFTER: Duration = Duration::from_secs(60);
/// SIGTERM → SIGKILL grace when stopping.
const TERM_GRACE: Duration = Duration::from_secs(3);
/// Output lines kept for crash messages.
const TAIL_LINES: usize = 20;
const TAIL_CHARS: usize = 800;
/// How often a running npm dev server checks whether its packages changed.
const DEPS_POLL: Duration = Duration::from_secs(3);
/// Fingerprint of the manifests the install command last ran against,
/// relative to the install dir.
const DEPS_MARKER: &str = "node_modules/.orso-deps";
/// Lockfiles recognised in the install dir, in priority order.
const LOCKFILES: &[&str] = &[
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lock",
    "bun.lockb",
];

/// `(install_dir, site_dir)` for a project. On invalid settings the install
/// dir falls back to [`crate::settings::install_dir_lenient`] and the site
/// dir to the project dir, so the poll keeps running.
fn deps_dirs(project: &std::path::Path) -> (PathBuf, PathBuf) {
    match crate::settings::load(project) {
        Ok(s) => (s.install_path(project), s.site_dir(project)),
        Err(_) => (
            crate::settings::install_dir_lenient(project),
            project.to_path_buf(),
        ),
    }
}

fn deps_marker(project: &std::path::Path) -> PathBuf {
    deps_dirs(project).0.join(DEPS_MARKER)
}

/// Fingerprint of `orso.json`, the `package.json` files in the install and
/// site dirs (deduplicated), and the first present lockfile in the install
/// dir. `None` only when neither `package.json` exists.
fn deps_fingerprint(project: &std::path::Path) -> Option<String> {
    use std::hash::{Hash, Hasher};
    let (install_dir, site_dir) = deps_dirs(project);
    let install_manifest = std::fs::read(install_dir.join("package.json")).ok();
    let site_manifest = if site_dir == install_dir {
        None
    } else {
        std::fs::read(site_dir.join("package.json")).ok()
    };
    if install_manifest.is_none() && site_manifest.is_none() {
        return None;
    }
    let settings = std::fs::read(project.join(crate::settings::SETTINGS_FILE)).unwrap_or_default();
    let lock = LOCKFILES
        .iter()
        .map(|name| install_dir.join(name))
        .find(|p| p.is_file())
        .and_then(|p| std::fs::read(p).ok())
        .unwrap_or_default();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    settings.hash(&mut h);
    install_manifest.unwrap_or_default().hash(&mut h);
    if site_dir != install_dir {
        site_manifest.unwrap_or_default().hash(&mut h);
    }
    lock.hash(&mut h);
    Some(format!("{:016x}", h.finish()))
}

/// The packages changed since the last install (a sync, a change switch, the agent's git work).
fn deps_stale(project: &std::path::Path) -> bool {
    let Some(now) = deps_fingerprint(project) else {
        return false;
    };
    std::fs::read_to_string(deps_marker(project)).map_or(true, |m| m.trim() != now)
}

/// Record the manifests a successful install for `project` ran against.
pub(crate) fn record_deps(project: &std::path::Path) {
    if let Some(fp) = deps_fingerprint(project) {
        let marker = deps_marker(project);
        // A package with no dependencies gets no `node_modules` from install.
        if let Some(parent) = marker.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(marker, fp);
    }
}

/// What the supervisor needs to build the dev-server command.
#[derive(Debug, Clone)]
pub struct LaunchSpec {
    pub project_id: String,
    pub dir: PathBuf,
    pub port: u16,
    /// Value of `PUBLIC_BASE`: `/preview/{id}/`.
    pub public_base: String,
}

/// Builds the program and arguments. The supervisor adds `PUBLIC_BASE`, the working
/// directory, stdio and the process group itself.
pub type CommandFactory = Arc<dyn Fn(&LaunchSpec) -> Command + Send + Sync>;

#[derive(Clone)]
pub enum Launcher {
    /// Do not spawn anything (project status stays `starting`).
    Disabled,
    /// The `dev` command from `orso.json` (defaults to the npm command).
    Configured,
    /// Test hook: an arbitrary command standing in for the dev server.
    Custom(CommandFactory),
}

#[derive(Clone)]
pub struct DevServerOptions {
    pub launcher: Launcher,
    pub backoff_base: Duration,
    pub stable_after: Duration,
    /// Daemon listen port; exposed to Astro as `ORSO_PROXY_PORT` so Vite HMR
    /// connects through the preview proxy instead of the raw Astro port.
    pub proxy_port: u16,
}

impl DevServerOptions {
    pub fn configured(proxy_port: u16) -> Self {
        Self::with_launcher(Launcher::Configured, proxy_port)
    }

    pub fn disabled() -> Self {
        Self::with_launcher(Launcher::Disabled, 0)
    }

    pub fn custom(factory: impl Fn(&LaunchSpec) -> Command + Send + Sync + 'static) -> Self {
        Self::with_launcher(Launcher::Custom(Arc::new(factory)), 0)
    }

    fn with_launcher(launcher: Launcher, proxy_port: u16) -> Self {
        Self {
            launcher,
            backoff_base: BACKOFF_BASE,
            stable_after: STABLE_AFTER,
            proxy_port,
        }
    }
}

/// `sh -c <dev command with {port}/{host} substituted>`, run in the site dir.
fn configured_command(settings: &crate::settings::ProjectSettings, spec: &LaunchSpec) -> Command {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(settings.dev_command(spec.port))
        .current_dir(settings.site_dir(&spec.dir));
    cmd
}

struct Shared {
    projects: Arc<ProjectRegistry>,
    events: EventBus,
    opts: DevServerOptions,
    /// Why each `crashed` project stopped (output tail); cleared on any other state.
    crash_messages: std::sync::Mutex<HashMap<String, String>>,
}

struct Running {
    stop: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

pub struct DevServerManager {
    shared: Arc<Shared>,
    running: Mutex<HashMap<String, Running>>,
}

impl DevServerManager {
    pub fn new(projects: Arc<ProjectRegistry>, events: EventBus, opts: DevServerOptions) -> Self {
        Self {
            shared: Arc::new(Shared {
                projects,
                events,
                opts,
                crash_messages: Default::default(),
            }),
            running: Mutex::new(HashMap::new()),
        }
    }

    /// Why the project's dev server stopped, while it is `crashed`.
    pub fn crash_message(&self, id: &str) -> Option<String> {
        self.shared
            .crash_messages
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned()
    }

    /// Whether this manager ever spawns anything.
    pub fn enabled(&self) -> bool {
        !matches!(self.shared.opts.launcher, Launcher::Disabled)
    }

    /// (Re)start supervision for a project with a fresh crash budget. Any previous
    /// process is stopped first. If the persisted port is busy a new one is allocated.
    pub async fn start(&self, id: &str) -> Result<(), ApiError> {
        if !self.enabled() {
            return Ok(());
        }
        let mut running = self.running.lock().await;
        if let Some(old) = running.remove(id) {
            old.stop().await;
        }
        let port = match self.shared.projects.ensure_free_port(id).await {
            Ok(p) => p,
            Err(e) => {
                // No supervisor will run: make the failure visible (status + event) so the
                // project is not left `starting` forever. Restart retries.
                if e.code != ErrorCode::Enoent {
                    if let Some(p) = self.shared.projects.get(id).await {
                        let msg = format!("cannot start dev server: {e}");
                        self.shared
                            .set_state(id, DevserverStatus::Crashed, p.port, Some(&msg))
                            .await;
                    }
                }
                return Err(e);
            }
        };
        self.shared
            .set_state(id, DevserverStatus::Starting, port, None)
            .await;
        let (stop, stop_rx) = oneshot::channel();
        let task = tokio::spawn(supervise(self.shared.clone(), id.to_string(), stop_rx));
        running.insert(id.to_string(), Running { stop, task });
        Ok(())
    }

    /// Stop the project's dev server and wait until the process group is gone.
    pub async fn stop(&self, id: &str) {
        let old = self.running.lock().await.remove(id);
        if let Some(old) = old {
            old.stop().await;
        }
    }

    /// Supervise every registered project (daemon start).
    pub async fn resume_all(&self) {
        for p in self.shared.projects.list().await {
            if let Err(e) = self.start(&p.id).await {
                eprintln!("orso: cannot start dev server for {}: {e}", p.id);
            }
        }
    }

    /// Stop every dev server (daemon shutdown).
    pub async fn shutdown(&self) {
        let all: Vec<Running> = self.running.lock().await.drain().map(|(_, r)| r).collect();
        let mut stopping = Vec::new();
        for r in all {
            let _ = r.stop.send(());
            stopping.push(r.task);
        }
        for t in stopping {
            let _ = t.await;
        }
    }
}

impl Running {
    async fn stop(self) {
        let _ = self.stop.send(());
        let _ = self.task.await;
    }
}

impl Shared {
    /// Record a status, publishing `devserver.status` when the state or port changed
    /// (or when `message` accompanies a terminal state).
    async fn set_state(&self, id: &str, state: DevserverStatus, port: u16, message: Option<&str>) {
        let Some((prev_state, prev_port)) = self.projects.set_devserver(id, state).await else {
            return;
        };
        {
            let mut crashes = self
                .crash_messages
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match (state, message) {
                (DevserverStatus::Crashed, Some(m)) => {
                    crashes.insert(id.to_string(), m.to_string());
                }
                (DevserverStatus::Crashed, None) => {}
                _ => {
                    crashes.remove(id);
                }
            }
        }
        if prev_state == state && prev_port == port && message.is_none() {
            return;
        }
        let mut ev = json!({ "projectId": id, "state": state.as_str(), "port": port });
        if let Some(m) = message {
            ev["message"] = json!(m);
        }
        self.events.emit("devserver.status", ev);
    }
}

enum Outcome {
    /// We were asked to stop.
    Stopped,
    /// The packages changed under a running server: reinstall and start again (not a crash).
    DepsChanged,
    /// The server failed to start or exited. `stable`: it had been ready for a while.
    Died { message: String, stable: bool },
}

async fn supervise(shared: Arc<Shared>, id: String, mut stop: oneshot::Receiver<()>) {
    let mut crashes = 0u32;
    let mut port = 0u16;
    loop {
        let Some(project) = shared.projects.get(&id).await else {
            return;
        };
        let outcome = match shared.projects.ensure_free_port(&id).await {
            Ok(p) => {
                port = p;
                // A bad orso.json is not fixed by retrying: park as crashed
                // without spending the crash budget or scheduling a backoff,
                // until the file loads.
                let settings = match &shared.opts.launcher {
                    Launcher::Configured => match crate::settings::load(&project.dir) {
                        Ok(s) => Some(s),
                        Err(e) => {
                            let message = crate::settings::describe(&e);
                            shared
                                .set_state(&id, DevserverStatus::Crashed, port, Some(&message))
                                .await;
                            if !await_valid_settings(
                                &shared,
                                &id,
                                &project.dir,
                                port,
                                message,
                                &mut stop,
                            )
                            .await
                            {
                                return;
                            }
                            crashes = 0;
                            continue;
                        }
                    },
                    _ => None,
                };
                shared
                    .set_state(&id, DevserverStatus::Starting, port, None)
                    .await;
                let configured = matches!(shared.opts.launcher, Launcher::Configured);
                match configured && deps_stale(&project.dir) {
                    true => match crate::projects::install(&project.dir).await {
                        Ok(()) => {
                            run_once(
                                &shared,
                                &id,
                                project.dir,
                                port,
                                &mut stop,
                                settings.as_ref(),
                            )
                            .await
                        }
                        Err(e) => Outcome::Died {
                            message: e.message,
                            stable: false,
                        },
                    },
                    false => {
                        run_once(
                            &shared,
                            &id,
                            project.dir,
                            port,
                            &mut stop,
                            settings.as_ref(),
                        )
                        .await
                    }
                }
            }
            Err(e) if e.code == ErrorCode::Enoent => return,
            Err(e) => Outcome::Died {
                message: format!("no usable port: {e}"),
                stable: false,
            },
        };
        let (message, stable) = match outcome {
            Outcome::Stopped => return,
            Outcome::DepsChanged => continue,
            Outcome::Died { message, stable } => (message, stable),
        };
        if stable {
            crashes = 0;
        }
        eprintln!("orso: dev server for {id} died: {message}");
        if crashes >= MAX_RESTARTS {
            shared
                .set_state(&id, DevserverStatus::Crashed, port, Some(&message))
                .await;
            let Some(project) = shared.projects.get(&id).await else {
                return;
            };
            if !matches!(shared.opts.launcher, Launcher::Configured)
                || !await_deps_change(&project.dir, &mut stop).await
            {
                return;
            }
            crashes = 0;
            continue;
        }
        crashes += 1;
        // Back to `starting` for the duration of the backoff.
        shared
            .set_state(&id, DevserverStatus::Starting, port, None)
            .await;
        let delay = shared.opts.backoff_base * 2u32.pow(crashes - 1);
        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
            _ = &mut stop => return,
        }
    }
}

/// Poll the package fingerprint until it differs from the one the server crashed
/// with (`true`) or we are asked to stop (`false`).
async fn await_deps_change(dir: &std::path::Path, stop: &mut oneshot::Receiver<()>) -> bool {
    let parked = deps_fingerprint(dir);
    loop {
        tokio::select! {
            _ = &mut *stop => return false,
            _ = tokio::time::sleep(DEPS_POLL) => {
                if deps_fingerprint(dir) != parked {
                    return true;
                }
            }
        }
    }
}

/// Poll `orso.json` until it loads (`true`) or we are asked to stop (`false`).
/// A changed error replaces the crash message.
async fn await_valid_settings(
    shared: &Shared,
    id: &str,
    dir: &std::path::Path,
    port: u16,
    mut message: String,
    stop: &mut oneshot::Receiver<()>,
) -> bool {
    loop {
        tokio::select! {
            _ = &mut *stop => return false,
            _ = tokio::time::sleep(DEPS_POLL) => match crate::settings::load(dir) {
                Ok(_) => return true,
                Err(e) => {
                    let now = crate::settings::describe(&e);
                    if now != message {
                        shared
                            .set_state(id, DevserverStatus::Crashed, port, Some(&now))
                            .await;
                        message = now;
                    }
                }
            },
        }
    }
}

/// Kills the whole process group when dropped (also when the task is aborted at runtime
/// shutdown). Safe after the leader was reaped: a group id cannot be recycled as a pid
/// while any member is alive, and if none is, the signal finds nothing.
struct GroupGuard(Option<i32>);

impl GroupGuard {
    fn signal(&self, sig: i32) {
        if let Some(pgid) = self.0 {
            // SAFETY: plain syscall on the group we created for this child.
            unsafe {
                libc::killpg(pgid, sig);
            }
        }
    }
}

impl Drop for GroupGuard {
    fn drop(&mut self) {
        self.signal(libc::SIGKILL);
    }
}

async fn run_once(
    shared: &Arc<Shared>,
    id: &str,
    dir: PathBuf,
    port: u16,
    stop: &mut oneshot::Receiver<()>,
    settings: Option<&crate::settings::ProjectSettings>,
) -> Outcome {
    let spec = LaunchSpec {
        project_id: id.to_string(),
        dir: dir.clone(),
        port,
        public_base: format!("/preview/{id}/"),
    };
    let mut cmd = match &shared.opts.launcher {
        Launcher::Configured => match settings {
            Some(settings) => configured_command(settings, &spec),
            None => {
                return Outcome::Died {
                    message: "dev server settings were not loaded".into(),
                    stable: false,
                }
            }
        },
        Launcher::Custom(f) => f(&spec),
        Launcher::Disabled => {
            return Outcome::Died {
                message: "dev server launcher is disabled".into(),
                stable: false,
            }
        }
    };
    // `configured_command` already set the site dir; other launchers run in
    // the project dir.
    if !matches!(shared.opts.launcher, Launcher::Configured) {
        cmd.current_dir(&dir);
    }
    cmd.env("PUBLIC_BASE", &spec.public_base)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        // Own group, so terminating also reaches npm's node grandchild.
        .process_group(0);
    if shared.opts.proxy_port > 0 {
        cmd.env("ORSO_PROXY_PORT", shared.opts.proxy_port.to_string());
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return Outcome::Died {
                message: format!("failed to start dev server: {e}"),
                stable: false,
            }
        }
    };
    let guard = GroupGuard(child.id().map(|p| p as i32));
    let (tx, mut lines) = mpsc::unbounded_channel::<String>();
    let readers = [
        child
            .stdout
            .take()
            .map(|r| tokio::spawn(read_lines(r, tx.clone()))),
        child
            .stderr
            .take()
            .map(|r| tokio::spawn(read_lines(r, tx.clone()))),
    ];
    drop(tx);

    let mut tail = Tail::default();
    let mut ready_at: Option<Instant> = None;
    let mut port = port;
    let watch_deps = matches!(shared.opts.launcher, Launcher::Configured);
    let mut deps_tick = tokio::time::interval(DEPS_POLL);
    deps_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let outcome = loop {
        tokio::select! {
            biased;
            _ = &mut *stop => {
                terminate(&mut child, &guard).await;
                break Outcome::Stopped;
            }
            _ = deps_tick.tick(), if watch_deps => {
                if deps_stale(&dir) {
                    terminate(&mut child, &guard).await;
                    break Outcome::DepsChanged;
                }
            }
            status = child.wait() => {
                // Kill stragglers so the pipes close, then collect the last output.
                guard.signal(libc::SIGKILL);
                let _ = tokio::time::timeout(Duration::from_secs(1), async {
                    while let Some(l) = lines.recv().await {
                        tail.push(l);
                    }
                }).await;
                let how = match status {
                    Ok(s) => format!("exited with {s}"),
                    Err(e) => format!("wait failed: {e}"),
                };
                let stable = ready_at.is_some_and(|t| t.elapsed() >= shared.opts.stable_after);
                break Outcome::Died { message: tail.describe(&how), stable };
            }
            Some(line) = lines.recv() => {
                if ready_at.is_none() {
                    if let Some(actual) = ready_port(&line) {
                        // The server may have picked another port if ours was taken.
                        if actual != port {
                            // The proxy routes by the registry's port, so it must agree
                            // with the process before anyone is told it is ready.
                            if let Err(e) = shared.projects.set_port(id, actual).await {
                                terminate(&mut child, &guard).await;
                                break Outcome::Died {
                                    message: format!(
                                        "dev server listens on port {actual} but it could not be recorded: {e}"
                                    ),
                                    stable: false,
                                };
                            }
                            port = actual;
                        }
                        ready_at = Some(Instant::now());
                        shared.set_state(id, DevserverStatus::Ready, port, None).await;
                    }
                }
                tail.push(line);
            }
        }
    };
    for r in readers.into_iter().flatten() {
        r.abort();
    }
    drop(guard);
    outcome
}

/// SIGTERM the group, wait for the leader, escalate to SIGKILL after the grace period.
async fn terminate(child: &mut Child, guard: &GroupGuard) {
    guard.signal(libc::SIGTERM);
    if tokio::time::timeout(TERM_GRACE, child.wait())
        .await
        .is_err()
    {
        guard.signal(libc::SIGKILL);
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    // The leader may exit before its children do.
    guard.signal(libc::SIGKILL);
}

async fn read_lines(reader: impl AsyncRead + Unpin, tx: mpsc::UnboundedSender<String>) {
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if tx.send(line).is_err() {
            break;
        }
    }
}

#[derive(Default)]
struct Tail(VecDeque<String>);

impl Tail {
    fn push(&mut self, line: String) {
        let line = strip_ansi(&line);
        if line.trim().is_empty() {
            return;
        }
        if self.0.len() == TAIL_LINES {
            self.0.pop_front();
        }
        self.0.push_back(line);
    }

    fn describe(&self, how: &str) -> String {
        let joined = self.0.iter().cloned().collect::<Vec<_>>().join("\n");
        let start = joined
            .char_indices()
            .rev()
            .nth(TAIL_CHARS - 1)
            .map_or(0, |(i, _)| i);
        let out = &joined[start..];
        if out.is_empty() {
            how.to_string()
        } else {
            format!("{how}: {out}")
        }
    }
}

/// Remove ANSI escape sequences (CSI and OSC-free subset used by Vite/Astro output).
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for n in chars.by_ref() {
                    if ('@'..='~').contains(&n) {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The port from a "server is listening" line, e.g. Astro's `┃ Local    http://localhost:4321/`
/// or Vite's `➜  Local:   http://127.0.0.1:5173/`. Only loopback URLs count.
pub fn ready_port(line: &str) -> Option<u16> {
    let clean = strip_ansi(line);
    let rest = &clean[clean.find("http://")? + "http://".len()..];
    let authority = rest.split(|c: char| c == '/' || c.is_whitespace()).next()?;
    let (host, port) = authority.rsplit_once(':')?;
    matches!(host, "localhost" | "127.0.0.1" | "[::1]" | "0.0.0.0")
        .then(|| port.parse().ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_line_detection() {
        assert_eq!(
            ready_port("\u{1b}[2m ┃ \u{1b}[22mLocal    \u{1b}[1mhttp://localhost:4321/preview/x/\u{1b}[22m"),
            Some(4321)
        );
        assert_eq!(
            ready_port("  ➜  Local:   http://127.0.0.1:5173/"),
            Some(5173)
        );
        // Astro's `--json` logging (used when it detects an agent) keeps the URL in the message.
        assert_eq!(
            ready_port(
                r#"{"message":" astro  v7.3.5 ready in 657 ms\n┃ Local    http://127.0.0.1:4999/","label":"SKIP_FORMAT","level":"info"}"#
            ),
            Some(4999)
        );
        assert_eq!(ready_port(" ┃ Network  use --host to expose"), None);
        assert_eq!(ready_port("Network http://192.168.1.4:4321/"), None);
        assert_eq!(ready_port("astro v7 ready in 300 ms"), None);
        assert_eq!(ready_port("see http://example.com:80/ for docs"), None);
    }

    #[test]
    fn configured_command_line() {
        let spec = LaunchSpec {
            project_id: "site".into(),
            dir: PathBuf::from("/tmp/site"),
            port: 4567,
            public_base: "/preview/site/".into(),
        };
        let settings = crate::settings::ProjectSettings::defaults();
        let cmd = configured_command(&settings, &spec);
        let std = cmd.as_std();
        assert_eq!(std.get_program(), "sh");
        let args: Vec<_> = std
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "-c",
                "npm run dev -- --port 4567 --host 127.0.0.1 --ignore-lock"
            ]
        );
        assert_eq!(
            std.get_current_dir(),
            Some(std::path::Path::new("/tmp/site"))
        );

        // A configured subfolder root is the process working directory.
        let settings = crate::settings::ProjectSettings {
            root: "site".to_string(),
            ..crate::settings::ProjectSettings::defaults()
        };
        let cmd = configured_command(&settings, &spec);
        assert_eq!(
            cmd.as_std().get_current_dir(),
            Some(std::path::Path::new("/tmp/site/site"))
        );
    }

    #[test]
    fn fingerprint_covers_each_lockfile() {
        // Root-only assertions (the pre-settings behaviour).
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        assert!(!deps_stale(dir), "no package.json: nothing to install");
        std::fs::write(dir.join("package.json"), r#"{"dependencies":{}}"#).unwrap();
        assert!(deps_stale(dir), "never installed");
        record_deps(dir);
        assert!(!deps_stale(dir));
        std::fs::write(dir.join("package.json"), r#"{"dependencies":{"x":"1"}}"#).unwrap();
        assert!(deps_stale(dir), "manifest changed");

        for lock in [
            "package-lock.json",
            "pnpm-lock.yaml",
            "yarn.lock",
            "bun.lock",
            "bun.lockb",
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let dir = tmp.path();
            std::fs::write(dir.join("package.json"), r#"{"name":"site"}"#).unwrap();
            record_deps(dir);
            assert!(!deps_stale(dir), "{lock}: fresh after record");
            // A package with no dependencies gets no `node_modules` from install;
            // record_deps must still leave a readable marker.
            std::fs::write(dir.join(lock), "{}").unwrap();
            assert!(deps_stale(dir), "{lock}: writing lockfile makes deps stale");
        }
    }

    #[test]
    fn fingerprint_covers_site_and_install_manifests() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::create_dir_all(dir.join("site")).unwrap();
        std::fs::write(dir.join("site/astro.config.mjs"), "export default {};\n").unwrap();
        std::fs::write(dir.join("orso.json"), r#"{"root":"site","installDir":"."}"#).unwrap();
        std::fs::write(dir.join("package.json"), r#"{"name":"root"}"#).unwrap();
        std::fs::write(dir.join("site/package.json"), r#"{"name":"site"}"#).unwrap();
        record_deps(dir);
        assert!(!deps_stale(dir));
        std::fs::write(dir.join("site/package.json"), r#"{"name":"site2"}"#).unwrap();
        assert!(deps_stale(dir), "site manifest changed");
        record_deps(dir);
        assert!(!deps_stale(dir));
        std::fs::write(dir.join("package.json"), r#"{"name":"root2"}"#).unwrap();
        assert!(deps_stale(dir), "install manifest changed");
    }

    #[test]
    fn fingerprint_covers_settings_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("package.json"), r#"{"name":"site"}"#).unwrap();
        record_deps(dir);
        assert!(!deps_stale(dir));
        std::fs::write(dir.join("orso.json"), r#"{"install":"true"}"#).unwrap();
        assert!(deps_stale(dir), "settings appeared");
        record_deps(dir);
        assert!(!deps_stale(dir));
        std::fs::write(dir.join("orso.json"), r#"{"install":"echo hi"}"#).unwrap();
        assert!(deps_stale(dir), "settings edited");
        record_deps(dir);
        assert!(!deps_stale(dir));
        std::fs::remove_file(dir.join("orso.json")).unwrap();
        assert!(deps_stale(dir), "settings deleted (branch switch)");
    }

    #[test]
    fn marker_lives_in_install_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::create_dir_all(dir.join("site")).unwrap();
        std::fs::write(dir.join("site/astro.config.mjs"), "export default {};\n").unwrap();
        std::fs::write(dir.join("orso.json"), r#"{"root":"site","installDir":"."}"#).unwrap();
        std::fs::write(dir.join("package.json"), r#"{"name":"root"}"#).unwrap();
        std::fs::write(dir.join("site/package.json"), r#"{"name":"site"}"#).unwrap();
        record_deps(dir);
        assert!(dir.join("node_modules/.orso-deps").is_file());

        // A distinct installDir gets its own marker.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::create_dir_all(dir.join("vendor")).unwrap();
        std::fs::write(dir.join("orso.json"), r#"{"installDir":"vendor"}"#).unwrap();
        std::fs::write(dir.join("vendor/package.json"), r#"{"name":"v"}"#).unwrap();
        record_deps(dir);
        assert!(dir.join("vendor/node_modules/.orso-deps").is_file());
        assert!(!deps_stale(dir));
    }

    #[test]
    fn invalid_settings_still_fingerprint() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("orso.json"), r#"{"dev":"x"}"#).unwrap();
        std::fs::write(dir.join("package.json"), r#"{"name":"site"}"#).unwrap();
        assert!(deps_fingerprint(dir).is_some());
        record_deps(dir);
        assert!(
            dir.join("node_modules/.orso-deps").is_file(),
            "default installDir is the project dir"
        );
        assert!(!deps_stale(dir));
    }

    #[test]
    fn no_manifest_means_never_stale() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("package-lock.json"), "{}").unwrap();
        assert!(deps_fingerprint(dir).is_none());
        assert!(!deps_stale(dir), "non-Node repos never reinstall");
    }

    #[test]
    fn tail_is_bounded_and_clean() {
        let mut t = Tail::default();
        for i in 0..100 {
            t.push(format!("\u{1b}[31mline {i}\u{1b}[0m"));
        }
        let d = t.describe("exited with 1");
        assert!(d.starts_with("exited with 1: "));
        assert!(d.contains("line 99") && !d.contains("line 10\n") && !d.contains('\u{1b}'));
        assert_eq!(Tail::default().describe("x"), "x");
    }
}

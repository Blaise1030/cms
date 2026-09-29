//! Dev-server supervision: one `npm run dev` per project.
//!
//! A supervisor task per project spawns the dev server on the project's port with
//! `PUBLIC_BASE=/preview/{id}/`, marks it ready when stdout prints its local URL, restarts
//! it after crashes (at most [`MAX_RESTARTS`] times, with exponential backoff), and
//! publishes every transition as a `devserver.status` event.
//!
//! The server runs in its own process group so `npm`'s node grandchild dies with it.
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

/// Crash restarts before the server is parked as `crashed` (until a manual restart).
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
    /// `npm run dev -- --port N --host 127.0.0.1`.
    Npm,
    /// Test hook: an arbitrary command standing in for the dev server.
    Custom(CommandFactory),
}

#[derive(Clone)]
pub struct DevServerOptions {
    pub launcher: Launcher,
    pub backoff_base: Duration,
    pub stable_after: Duration,
}

impl DevServerOptions {
    pub fn npm() -> Self {
        Self::with_launcher(Launcher::Npm)
    }

    pub fn disabled() -> Self {
        Self::with_launcher(Launcher::Disabled)
    }

    pub fn custom(factory: impl Fn(&LaunchSpec) -> Command + Send + Sync + 'static) -> Self {
        Self::with_launcher(Launcher::Custom(Arc::new(factory)))
    }

    fn with_launcher(launcher: Launcher) -> Self {
        Self {
            launcher,
            backoff_base: BACKOFF_BASE,
            stable_after: STABLE_AFTER,
        }
    }
}

fn npm_command(spec: &LaunchSpec) -> Command {
    let mut cmd = Command::new("npm");
    cmd.args(["run", "dev", "--", "--port"])
        .arg(spec.port.to_string())
        // Bind IPv4 loopback explicitly: the proxy dials 127.0.0.1, and `localhost` may
        // resolve to ::1 only.
        .args(["--host", "127.0.0.1"]);
    cmd
}

struct Shared {
    projects: Arc<ProjectRegistry>,
    events: EventBus,
    opts: DevServerOptions,
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
            }),
            running: Mutex::new(HashMap::new()),
        }
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
        let port = self.shared.projects.ensure_free_port(id).await?;
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
                eprintln!("workbench: cannot start dev server for {}: {e}", p.id);
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
                shared
                    .set_state(&id, DevserverStatus::Starting, port, None)
                    .await;
                run_once(&shared, &id, project.dir, port, &mut stop).await
            }
            Err(e) if e.code == ErrorCode::Enoent => return,
            Err(e) => Outcome::Died {
                message: format!("no usable port: {e}"),
                stable: false,
            },
        };
        let (message, stable) = match outcome {
            Outcome::Stopped => return,
            Outcome::Died { message, stable } => (message, stable),
        };
        if stable {
            crashes = 0;
        }
        eprintln!("workbench: dev server for {id} died: {message}");
        if crashes >= MAX_RESTARTS {
            shared
                .set_state(&id, DevserverStatus::Crashed, port, Some(&message))
                .await;
            return;
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
) -> Outcome {
    let spec = LaunchSpec {
        project_id: id.to_string(),
        dir: dir.clone(),
        port,
        public_base: format!("/preview/{id}/"),
    };
    let mut cmd = match &shared.opts.launcher {
        Launcher::Npm => npm_command(&spec),
        Launcher::Custom(f) => f(&spec),
        Launcher::Disabled => {
            return Outcome::Died {
                message: "dev server launcher is disabled".into(),
                stable: false,
            }
        }
    };
    cmd.env("PUBLIC_BASE", &spec.public_base)
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        // Own group, so terminating also reaches npm's node grandchild.
        .process_group(0);
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
    let outcome = loop {
        tokio::select! {
            biased;
            _ = &mut *stop => {
                terminate(&mut child, &guard).await;
                break Outcome::Stopped;
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
                            if let Err(e) = shared.projects.set_port(id, actual).await {
                                eprintln!("workbench: cannot record port {actual} for {id}: {e}");
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
        assert_eq!(ready_port(" ┃ Network  use --host to expose"), None);
        assert_eq!(ready_port("Network http://192.168.1.4:4321/"), None);
        assert_eq!(ready_port("astro v7 ready in 300 ms"), None);
        assert_eq!(ready_port("see http://example.com:80/ for docs"), None);
    }

    #[test]
    fn npm_command_line() {
        let spec = LaunchSpec {
            project_id: "site".into(),
            dir: PathBuf::from("/tmp/site"),
            port: 4567,
            public_base: "/preview/site/".into(),
        };
        let cmd = npm_command(&spec);
        let std = cmd.as_std();
        assert_eq!(std.get_program(), "npm");
        let args: Vec<_> = std
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            ["run", "dev", "--", "--port", "4567", "--host", "127.0.0.1"]
        );
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

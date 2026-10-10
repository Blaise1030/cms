//! Per-project coding CLIs running in daemon-owned PTYs.
//!
//! At most one terminal is live per project. Each runs `$SHELL -lc 'exec <argv>'` (a login
//! shell, so the user's PATH applies) in the project dir, with the `WORKBENCH_*` variables the
//! `hook turn-end` subcommand needs. PTY I/O is blocking, so each session has three threads
//! (after tabsh `src/sessions/pty.rs`):
//!
//! - a writer fed by an mpsc channel ([`TerminalManager::write`]);
//! - a reader that appends 8 KB chunks to a 512 KB scrollback ring and broadcasts them, both
//!   under the ring's lock, so an [`Attach`] (replay + subscribe under that same lock) sees
//!   every byte exactly once, in its replay or live;
//! - a waiter on `child.wait()` that broadcasts [`Output::Exit`], forgets the session and
//!   emits `terminal.exited { projectId, code }`.
//!
//! Terminal modes set by bytes trimmed off the ring's front are tracked ([`modes`]) and
//! prefixed to the replay. `portable-pty` makes the child a session (and process-group)
//! leader, so [`TerminalManager::kill`] signals the whole group: `SIGTERM`, then `SIGKILL`
//! after 2 s.

mod modes;

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde_json::json;
use tokio::sync::{broadcast, watch};

use crate::auth::ct_eq;
use crate::config::{random_token, Config};
use crate::error::ApiError;
use crate::events::EventBus;
use crate::providers::LaunchSpec;
use modes::ModeTracker;

const INITIAL_SIZE: PtySize = PtySize {
    rows: 24,
    cols: 80,
    pixel_width: 0,
    pixel_height: 0,
};
const READ_CHUNK: usize = 8 * 1024;
const SCROLLBACK_BYTES: usize = 512 * 1024;
const BROADCAST_CAPACITY: usize = 1024;
/// Grace between `SIGTERM` and `SIGKILL`, and the wait for the reap after `SIGKILL`.
const KILL_GRACE: Duration = Duration::from_secs(2);
/// How long the waiter lets the reader drain the child's last output before `Exit`.
const DRAIN_WAIT: Duration = Duration::from_millis(200);

static SPAWN_LOCK: Mutex<()> = Mutex::new(());

/// What attached clients receive.
#[derive(Clone, Debug)]
pub enum Output {
    /// Bytes the PTY produced.
    Data(Arc<[u8]>),
    /// The child exited: its code, or `None` if a signal ended it.
    Exit(Option<i32>),
}

/// A client attaching: the history to replay, then the live output after it.
pub struct Attach {
    pub replay: Vec<u8>,
    pub rx: broadcast::Receiver<Output>,
}

/// Scrollback and the broadcast sender, always used together under one lock.
struct Ring {
    scrollback: VecDeque<u8>,
    trimmed_modes: ModeTracker,
    tx: broadcast::Sender<Output>,
    exited: bool,
}

impl Ring {
    /// Keep a chunk (trimming the oldest bytes, remembering their modes) and send it.
    fn append(&mut self, chunk: &[u8]) {
        self.scrollback.extend(chunk);
        let excess = self.scrollback.len().saturating_sub(SCROLLBACK_BYTES);
        self.trimmed_modes.feed(self.scrollback.drain(..excess));
        let _ = self.tx.send(Output::Data(chunk.into()));
    }

    /// Copies only: the reader waits on this lock for every chunk.
    fn attach(&self) -> Attach {
        let mut replay = self.trimmed_modes.replay_prefix();
        let (front, back) = self.scrollback.as_slices();
        replay.reserve(front.len() + back.len());
        replay.extend_from_slice(front);
        replay.extend_from_slice(back);
        Attach {
            replay,
            rx: self.tx.subscribe(),
        }
    }
}

struct Session {
    provider: String,
    token: String,
    pid: Option<u32>,
    input: mpsc::Sender<Vec<u8>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    ring: Mutex<Ring>,
    /// Flips to `true` once the waiter has reaped the child and forgotten the session.
    done: watch::Receiver<bool>,
}

struct Inner {
    config: Arc<Config>,
    events: EventBus,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
}

/// Owns every project's terminal. Cheap to clone.
#[derive(Clone)]
pub struct TerminalManager {
    inner: Arc<Inner>,
}

impl TerminalManager {
    pub fn new(config: Arc<Config>, events: EventBus) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                events,
                sessions: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Spawn `launch` in a PTY for `project_id`. `conflict` if one is already live.
    pub async fn start(
        &self,
        project_id: &str,
        provider: &str,
        cwd: &Path,
        launch: LaunchSpec,
    ) -> Result<(), ApiError> {
        // Spawning is quick and synchronous; holding the map lock makes check-and-insert atomic.
        let mut sessions = self.inner.sessions.lock().unwrap();
        if sessions.contains_key(project_id) {
            return Err(ApiError::conflict(format!(
                "a terminal is already running for project {project_id}"
            )));
        }
        let session = self
            .spawn(project_id, provider, cwd, launch)
            .map_err(|e| ApiError::io(format!("failed to start terminal: {e}")))?;
        sessions.insert(project_id.to_string(), session);
        Ok(())
    }

    /// The provider id of the project's live terminal.
    pub fn status(&self, project_id: &str) -> Option<String> {
        self.get(project_id).map(|s| s.provider.clone())
    }

    /// Replay and subscription, taken atomically. `enoent` if no terminal is live.
    pub fn attach(&self, project_id: &str) -> Result<Attach, ApiError> {
        let session = self.require(project_id)?;
        let ring = session.ring.lock().unwrap();
        // Exited but not yet forgotten: a subscriber now would never hear the exit.
        if ring.exited {
            return Err(no_terminal(project_id));
        }
        Ok(ring.attach())
    }

    /// Queue bytes for the PTY's input.
    pub fn write(&self, project_id: &str, bytes: Vec<u8>) -> Result<(), ApiError> {
        self.require(project_id)?
            .input
            .send(bytes)
            .map_err(|_| ApiError::io("terminal input is closed"))
    }

    pub fn resize(&self, project_id: &str, cols: u16, rows: u16) -> Result<(), ApiError> {
        let session = self.require(project_id)?;
        let master = session.master.lock().unwrap();
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| ApiError::io(format!("failed to resize terminal: {e}")))
    }

    /// Whether `token` is the live session's hook token (constant-time compare).
    pub fn token_matches(&self, project_id: &str, token: &str) -> bool {
        self.get(project_id).is_some_and(|s| ct_eq(&s.token, token))
    }

    /// `SIGTERM` the child's process group, `SIGKILL` after 2 s, then wait for the reap.
    /// No-op if no terminal is live.
    pub async fn kill(&self, project_id: &str) {
        let Some(session) = self.get(project_id) else {
            return;
        };
        let mut done = session.done.clone();
        signal_group(session.pid, libc::SIGTERM);
        if tokio::time::timeout(KILL_GRACE, done.wait_for(|d| *d))
            .await
            .is_ok()
        {
            return;
        }
        signal_group(session.pid, libc::SIGKILL);
        if tokio::time::timeout(KILL_GRACE, done.wait_for(|d| *d))
            .await
            .is_err()
        {
            // Unkillable (e.g. stuck in the kernel): forget it anyway so the project is usable.
            forget(&self.inner, project_id, &session);
        }
    }

    /// Kill every terminal.
    pub async fn shutdown(&self) {
        let ids: Vec<String> = self
            .inner
            .sessions
            .lock()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        futures_util::future::join_all(ids.iter().map(|id| self.kill(id))).await;
    }

    fn get(&self, project_id: &str) -> Option<Arc<Session>> {
        self.inner.sessions.lock().unwrap().get(project_id).cloned()
    }

    fn require(&self, project_id: &str) -> Result<Arc<Session>, ApiError> {
        self.get(project_id).ok_or_else(|| no_terminal(project_id))
    }

    fn spawn(
        &self,
        project_id: &str,
        provider: &str,
        cwd: &Path,
        launch: LaunchSpec,
    ) -> Result<Arc<Session>, Box<dyn std::error::Error + Send + Sync>> {
        let config = &self.inner.config;
        let token = random_token()[..32].to_string();

        let spawn_guard = SPAWN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let pair = native_pty_system().openpty(INITIAL_SIZE)?;
        let mut cmd = CommandBuilder::new(user_shell());
        cmd.arg("-lc");
        cmd.arg(exec_line(&launch.argv));
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("WORKBENCH_URL", format!("http://127.0.0.1:{}", config.port));
        cmd.env("WORKBENCH_PROJECT", project_id);
        cmd.env("WORKBENCH_HOOK_TOKEN", &token);
        cmd.env("WORKBENCH_HOOK_EXE", config.hook_exe_path());
        for (k, v) in &launch.env {
            cmd.env(k, v);
        }
        let mut child = pair.slave.spawn_command(cmd)?;
        // The reader sees EOF only once every slave handle is closed.
        drop(pair.slave);
        drop(spawn_guard);

        let mut reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;

        let (input, in_rx) = mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            while let Ok(data) = in_rx.recv() {
                if writer
                    .write_all(&data)
                    .and_then(|()| writer.flush())
                    .is_err()
                {
                    break;
                }
            }
        });

        let (done_tx, done) = watch::channel(false);
        let session = Arc::new(Session {
            provider: provider.to_string(),
            token,
            pid: child.process_id(),
            input,
            master: Mutex::new(pair.master),
            ring: Mutex::new(Ring {
                scrollback: VecDeque::new(),
                trimmed_modes: ModeTracker::default(),
                tx: broadcast::channel(BROADCAST_CAPACITY).0,
                exited: false,
            }),
            done,
        });

        let (drained_tx, drained) = mpsc::channel::<()>();
        let s = session.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; READ_CHUNK];
            while let Ok(n @ 1..) = reader.read(&mut buf) {
                // Only the append and the broadcast happen under the lock.
                s.ring.lock().unwrap().append(&buf[..n]);
            }
            let _ = drained_tx.send(());
        });

        let s = session.clone();
        let inner = self.inner.clone();
        let id = project_id.to_string();
        std::thread::spawn(move || {
            let code = match child.wait() {
                Ok(status) if status.signal().is_none() => Some(status.exit_code() as i32),
                _ => None,
            };
            // Let the reader pass on the child's last words (a background process still
            // holding the PTY open would keep it reading forever, hence the bound).
            let _ = drained.recv_timeout(DRAIN_WAIT);
            {
                let mut ring = s.ring.lock().unwrap();
                ring.exited = true;
                let _ = ring.tx.send(Output::Exit(code));
            }
            forget(&inner, &id, &s);
            inner
                .events
                .emit("terminal.exited", json!({ "projectId": id, "code": code }));
            let _ = done_tx.send(true);
        });

        Ok(session)
    }
}

/// Forget `session` if it is still the project's live one (a new one may have replaced it).
fn forget(inner: &Inner, project_id: &str, session: &Arc<Session>) {
    let mut sessions = inner.sessions.lock().unwrap();
    if sessions
        .get(project_id)
        .is_some_and(|s| Arc::ptr_eq(s, session))
    {
        sessions.remove(project_id);
    }
}

fn no_terminal(project_id: &str) -> ApiError {
    ApiError::enoent(format!("no terminal is running for project {project_id}"))
}

/// Signal the child's process group. The child is a session leader (`portable-pty` calls
/// `setsid`), so its pgid is its pid; if that ever stops holding, signal just the child.
fn signal_group(pid: Option<u32>, signal: libc::c_int) {
    let Some(pid) = pid.and_then(|p| libc::pid_t::try_from(p).ok()) else {
        return;
    };
    unsafe {
        if libc::getpgid(pid) == pid {
            libc::killpg(pid, signal);
        } else {
            libc::kill(pid, signal);
        }
    }
}

/// `exec` followed by each argument single-quoted for `sh`.
fn exec_line(argv: &[String]) -> String {
    let mut line = String::from("exec");
    for arg in argv {
        line.push(' ');
        line.push_str(&sh_quote(arg));
    }
    line
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// `$SHELL`, else the account's login shell (a launchd service has no `$SHELL`), else
/// `/bin/sh`.
fn user_shell() -> String {
    match std::env::var("SHELL") {
        Ok(shell) if !shell.is_empty() => shell,
        _ => login_shell().unwrap_or_else(|| "/bin/sh".into()),
    }
}

fn login_shell() -> Option<String> {
    let mut pw: libc::passwd = unsafe { std::mem::zeroed() };
    let mut buf = vec![0 as libc::c_char; 4096];
    let mut found = std::ptr::null_mut();
    let rc = unsafe {
        libc::getpwuid_r(
            libc::getuid(),
            &mut pw,
            buf.as_mut_ptr(),
            buf.len(),
            &mut found,
        )
    };
    if rc != 0 || found.is_null() || pw.pw_shell.is_null() {
        return None;
    }
    let shell = unsafe { std::ffi::CStr::from_ptr(pw.pw_shell) }
        .to_str()
        .ok()?;
    (!shell.is_empty()).then(|| shell.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_line_quotes_every_argument() {
        let argv = ["claude", "it's a test", "$HOME", "a b"].map(String::from);
        assert_eq!(
            exec_line(&argv),
            r#"exec 'claude' 'it'\''s a test' '$HOME' 'a b'"#
        );
    }

    #[test]
    fn ring_trims_and_remembers_modes() {
        let mut ring = Ring {
            scrollback: VecDeque::new(),
            trimmed_modes: ModeTracker::default(),
            tx: broadcast::channel(4).0,
            exited: false,
        };
        ring.append(b"\x1b[?1049h");
        ring.append(&vec![b'x'; SCROLLBACK_BYTES]);
        let a = ring.attach();
        assert!(a.replay.starts_with(b"\x1b[?1049h"));
        assert_eq!(a.replay.len(), 8 + SCROLLBACK_BYTES);
    }
}

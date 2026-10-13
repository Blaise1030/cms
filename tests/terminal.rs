//! `TerminalManager`: PTY spawn, streaming, replay, resize, exit and process-group kill.
//! Drives `tests/fixtures/fake_cli.sh`, a line-driven stand-in for a coding CLI.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, Once};
use std::time::{Duration, Instant};

use tokio::sync::broadcast;
use workbench::config::Config;
use workbench::events::EventBus;
use workbench::providers::LaunchSpec;
use workbench::terminal::{Output, TerminalManager};

const TIMEOUT: Duration = Duration::from_secs(10);

/// One PTY session at a time: parallel `cargo test` across crates can exhaust PTYs on macOS.
static TERMINAL_TEST_LOCK: Mutex<()> = Mutex::new(());

fn fake_cli() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fake_cli.sh").to_string()
}

fn manager() -> (MutexGuard<'static, ()>, TerminalManager, EventBus) {
    // Login zsh under parallel `cargo test` can stall; the fixture is POSIX sh.
    static SHELL: Once = Once::new();
    SHELL.call_once(|| std::env::set_var("SHELL", "/bin/sh"));
    let lock = TERMINAL_TEST_LOCK.lock().unwrap();
    let events = EventBus::new();
    let config = Config {
        hook_exe: Some(PathBuf::from(env!("CARGO_BIN_EXE_workbench"))),
        ..Default::default()
    };
    (
        lock,
        TerminalManager::new(Arc::new(config), events.clone()),
        events,
    )
}

fn launch(argv: &[&str]) -> LaunchSpec {
    LaunchSpec {
        argv: argv.iter().map(|s| s.to_string()).collect(),
        env: vec![],
    }
}

/// Accumulate PTY output (starting from `seed`, e.g. a replay) until `found` extracts a value.
async fn read_matching<T>(
    rx: &mut broadcast::Receiver<Output>,
    seed: &[u8],
    found: impl Fn(&str) -> Option<T>,
) -> T {
    let mut acc = String::from_utf8_lossy(seed).into_owned();
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(v) = found(&acc) {
            return v;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        match tokio::time::timeout(left, rx.recv()).await {
            Ok(Ok(Output::Data(d))) => acc.push_str(&String::from_utf8_lossy(&d)),
            Ok(Ok(Output::Exit(code))) => panic!("exited ({code:?}) early: {acc:?}"),
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => {}
            Ok(Err(e)) => panic!("channel closed: {e}; {acc:?}"),
            Err(_) => panic!("timed out; output so far: {acc:?}"),
        }
    }
}

/// Accumulate PTY output until it contains `needle`.
async fn read_until(rx: &mut broadcast::Receiver<Output>, seed: &[u8], needle: &str) -> String {
    read_matching(rx, seed, |acc| {
        acc.contains(needle).then(|| acc.to_string())
    })
    .await
}

/// The token after `prefix` once it is complete: `len` chars of `ok`, then a non-`ok` char.
fn word_after(acc: &str, prefix: &str, ok: fn(char) -> bool) -> Option<String> {
    let tail = acc.split(prefix).nth(1)?;
    let word: String = tail.chars().take_while(|c| ok(*c)).collect();
    (!word.is_empty() && tail.len() > word.len()).then_some(word)
}

/// Wait for `Output::Exit`, ignoring data.
async fn read_exit(rx: &mut broadcast::Receiver<Output>) -> Option<i32> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match tokio::time::timeout(left, rx.recv()).await {
            Ok(Ok(Output::Exit(code))) => return code,
            Ok(Ok(Output::Data(_))) | Ok(Err(broadcast::error::RecvError::Lagged(_))) => {}
            Ok(Err(e)) => panic!("channel closed before exit: {e}"),
            Err(_) => panic!("timed out waiting for exit"),
        }
    }
}

/// Wait for a `terminal.exited` event for `project` and return its `code`.
async fn exited_event(rx: &mut broadcast::Receiver<Arc<str>>, project: &str) -> serde_json::Value {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let text = tokio::time::timeout(left, rx.recv())
            .await
            .expect("timed out waiting for terminal.exited")
            .expect("event bus closed");
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        if v["type"] == "terminal.exited" && v["projectId"] == project {
            return v["code"].clone();
        }
    }
}

async fn start_fake(m: &TerminalManager, project: &str, cwd: &Path) {
    m.start(project, "fake", cwd, launch(&[&fake_cli()]))
        .await
        .unwrap();
}

#[tokio::test]
async fn spawns_in_cwd_and_streams_output() {
    let (_lock, m, _) = manager();
    let dir = tempfile::tempdir().unwrap();
    start_fake(&m, "p1", dir.path()).await;
    assert_eq!(m.status("p1").as_deref(), Some("fake"));
    let mut a = m.attach("p1").unwrap();
    read_until(&mut a.rx, &a.replay, "FAKE-CLI READY").await;
    // The child runs in the project dir: `edit` appends there (the hook is a no-op here).
    m.write("p1", b"pwd-check\r".to_vec()).unwrap();
    read_until(&mut a.rx, b"", "ECHO pwd-check").await;
    m.write("p1", b"edit hi\r".to_vec()).unwrap();
    read_until(&mut a.rx, b"", "TURN-END").await;
    assert_eq!(
        std::fs::read_to_string(dir.path().join("notes.md")).unwrap(),
        "hi\n"
    );
    m.kill("p1").await;
}

#[tokio::test]
async fn replay_contains_earlier_output() {
    let (_lock, m, _) = manager();
    let dir = tempfile::tempdir().unwrap();
    start_fake(&m, "p1", dir.path()).await;
    let mut a = m.attach("p1").unwrap();
    read_until(&mut a.rx, &a.replay, "FAKE-CLI READY").await;
    let fresh = m.attach("p1").unwrap();
    assert!(String::from_utf8_lossy(&fresh.replay).contains("FAKE-CLI READY"));
    m.kill("p1").await;
}

#[tokio::test]
async fn write_reaches_stdin() {
    let (_lock, m, _) = manager();
    let dir = tempfile::tempdir().unwrap();
    start_fake(&m, "p1", dir.path()).await;
    let mut a = m.attach("p1").unwrap();
    read_until(&mut a.rx, &a.replay, "FAKE-CLI READY").await;
    m.write("p1", b"hello\r".to_vec()).unwrap();
    read_until(&mut a.rx, b"", "ECHO hello").await;
    m.kill("p1").await;
}

#[tokio::test]
async fn resize_reaches_pty() {
    let (_lock, m, _) = manager();
    let dir = tempfile::tempdir().unwrap();
    start_fake(&m, "p1", dir.path()).await;
    let mut a = m.attach("p1").unwrap();
    read_until(&mut a.rx, &a.replay, "FAKE-CLI READY").await;
    m.resize("p1", 120, 40).unwrap();
    m.write("p1", b"size\r".to_vec()).unwrap();
    read_until(&mut a.rx, b"", "40 120").await;
    m.kill("p1").await;
}

#[tokio::test]
async fn second_start_conflicts() {
    let (_lock, m, _) = manager();
    let dir = tempfile::tempdir().unwrap();
    start_fake(&m, "p1", dir.path()).await;
    let err = m
        .start("p1", "fake", dir.path(), launch(&[&fake_cli()]))
        .await
        .unwrap_err();
    assert_eq!(err.code.as_str(), "CONFLICT");
    m.kill("p1").await;
}

#[tokio::test]
async fn exit_is_broadcast_and_session_removed() {
    let (_lock, m, events) = manager();
    let mut ev = events.subscribe();
    let dir = tempfile::tempdir().unwrap();
    // Gated on a line of input so the attach below cannot race the exit.
    m.start(
        "p1",
        "fake",
        dir.path(),
        launch(&["sh", "-c", "read _; exit 3"]),
    )
    .await
    .unwrap();
    let mut a = m.attach("p1").unwrap();
    m.write("p1", b"\r".to_vec()).unwrap();
    assert_eq!(read_exit(&mut a.rx).await, Some(3));
    assert_eq!(exited_event(&mut ev, "p1").await, 3);
    assert_eq!(m.status("p1"), None);
    assert_eq!(m.attach("p1").err().unwrap().code.as_str(), "ENOENT");
}

#[tokio::test]
async fn kill_reaps_process_group() {
    let (_lock, m, events) = manager();
    let mut ev = events.subscribe();
    let dir = tempfile::tempdir().unwrap();
    start_fake(&m, "p1", dir.path()).await;
    let mut a = m.attach("p1").unwrap();
    read_until(&mut a.rx, &a.replay, "FAKE-CLI READY").await;
    m.write("p1", b"spawn-child\r".to_vec()).unwrap();
    let pid: i32 = read_matching(&mut a.rx, b"", |acc| {
        word_after(acc, "CHILD ", |c| c.is_ascii_digit())
    })
    .await
    .parse()
    .unwrap();
    assert_eq!(unsafe { libc::kill(pid, 0) }, 0, "child should be alive");

    m.kill("p1").await;
    assert_eq!(m.status("p1"), None);
    exited_event(&mut ev, "p1").await;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let rc = unsafe { libc::kill(pid, 0) };
        let errno = std::io::Error::last_os_error().raw_os_error();
        if rc == -1 && errno == Some(libc::ESRCH) {
            break;
        }
        assert!(Instant::now() < deadline, "grandchild {pid} survived kill");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // Killing again (or a never-started project) is a no-op.
    m.kill("p1").await;
    m.kill("nope").await;
}

/// After the leader is reaped, the waiter must `killpg` the group recorded at spawn.
/// A HUP-ignoring grandchild stays in that group; `kill` of the reaped pid would leave it
/// alive (and could hit a recycled pid). A bystander outside the group must survive.
#[tokio::test]
async fn leader_exit_signals_recorded_group_only() {
    let (_lock, m, events) = manager();
    let mut ev = events.subscribe();
    let dir = tempfile::tempdir().unwrap();
    let mut bystander = std::process::Command::new("sleep")
        .arg("300")
        .spawn()
        .unwrap();
    let bystander_pid = bystander.id();
    let kill_bystander = KillPid(bystander_pid);

    m.start(
        "p1",
        "fake",
        dir.path(),
        launch(&[
            "sh",
            "-c",
            r#"(trap "" HUP; exec sleep 300) & echo $! > child.pid; exit 0"#,
        ]),
    )
    .await
    .unwrap();
    assert_eq!(exited_event(&mut ev, "p1").await, 0);
    assert_eq!(m.status("p1"), None);

    let grand: u32 = std::fs::read_to_string(dir.path().join("child.pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut grand_dead = false;
    while Instant::now() < deadline {
        let rc = unsafe { libc::kill(grand as libc::pid_t, 0) };
        let errno = std::io::Error::last_os_error().raw_os_error();
        if rc == -1 && errno == Some(libc::ESRCH) {
            grand_dead = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    unsafe { libc::kill(grand as libc::pid_t, libc::SIGKILL) };
    let bystander_alive = unsafe { libc::kill(bystander_pid as libc::pid_t, 0) } == 0;
    let _ = bystander.kill();
    let _ = bystander.wait();
    drop(kill_bystander);
    assert!(grand_dead, "grandchild {grand} survived leader exit");
    assert!(bystander_alive, "bystander {bystander_pid} was signaled");
}

struct KillPid(u32);
impl Drop for KillPid {
    fn drop(&mut self) {
        if let Ok(pid) = libc::pid_t::try_from(self.0) {
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
    }
}

#[tokio::test]
async fn argv_with_spaces_and_quotes_reaches_child() {
    let (_lock, m, _) = manager();
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("with space");
    std::fs::create_dir(&dir).unwrap();
    m.start("p1", "fake", &dir, launch(&[&fake_cli(), "it's a test"]))
        .await
        .unwrap();
    let mut a = m.attach("p1").unwrap();
    read_until(&mut a.rx, &a.replay, "FAKE-CLI READY").await;
    m.write("p1", b"argv\r".to_vec()).unwrap();
    read_until(&mut a.rx, b"", "it's a test").await;
    m.kill("p1").await;
}

#[tokio::test]
async fn missing_command_exits_127() {
    let (_lock, m, events) = manager();
    let mut ev = events.subscribe();
    let dir = tempfile::tempdir().unwrap();
    m.start("p1", "fake", dir.path(), launch(&["definitely-not-a-cli"]))
        .await
        .unwrap();
    // The process may exit before an attach could subscribe, so assert via the event bus
    // (subscribed before start), and via the output channel when the attach wins the race.
    if let Ok(mut a) = m.attach("p1") {
        assert_eq!(read_exit(&mut a.rx).await, Some(127));
    }
    assert_eq!(exited_event(&mut ev, "p1").await, 127);
}

#[tokio::test]
async fn token_matches_only_the_live_session_token() {
    let (_lock, m, _) = manager();
    let dir = tempfile::tempdir().unwrap();
    assert!(!m.token_matches("p1", ""));
    m.start(
        "p1",
        "fake",
        dir.path(),
        launch(&["sh", "-c", "echo \"TOKEN=$WORKBENCH_HOOK_TOKEN\"; read _"]),
    )
    .await
    .unwrap();
    let mut a = m.attach("p1").unwrap();
    let token = read_matching(&mut a.rx, &a.replay, |acc| {
        word_after(acc, "TOKEN=", |c| c.is_ascii_hexdigit())
    })
    .await;
    assert_eq!(token.len(), 32, "{token:?}");
    assert!(token.chars().all(|c| c.is_ascii_hexdigit()), "{token:?}");
    assert!(m.token_matches("p1", &token));
    assert!(!m.token_matches("p1", "0".repeat(32).as_str()));
    assert!(!m.token_matches("p2", &token));
    m.kill("p1").await;
}

#[tokio::test]
async fn shutdown_kills_all() {
    let (_lock, m, _) = manager();
    let dir = tempfile::tempdir().unwrap();
    start_fake(&m, "p1", dir.path()).await;
    start_fake(&m, "p2", dir.path()).await;
    m.shutdown().await;
    assert_eq!(m.status("p1"), None);
    assert_eq!(m.status("p2"), None);
}

/// The next `terminal.busy` event for `project`: its `busy` flag.
async fn busy_event(rx: &mut broadcast::Receiver<Arc<str>>, project: &str) -> bool {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let text = tokio::time::timeout(left, rx.recv())
            .await
            .expect("timed out waiting for terminal.busy")
            .expect("event bus closed");
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        if v["type"] == "terminal.busy" && v["projectId"] == project {
            return v["busy"].as_bool().unwrap();
        }
    }
}

#[tokio::test]
async fn lone_enter_is_busy_until_turn_end() {
    let (_lock, m, events) = manager();
    let dir = tempfile::tempdir().unwrap();
    start_fake(&m, "p1", dir.path()).await;
    let mut a = m.attach("p1").unwrap();
    read_until(&mut a.rx, &a.replay, "FAKE-CLI READY").await;
    let mut ev = events.subscribe();
    // Typed text, then Enter as its own keystroke (how xterm sends it).
    m.write("p1", b"hi".to_vec()).unwrap();
    assert!(!m.busy("p1"));
    m.write("p1", b"\r".to_vec()).unwrap();
    assert!(busy_event(&mut ev, "p1").await);
    assert!(m.busy("p1"));
    m.set_idle("p1");
    assert!(!busy_event(&mut ev, "p1").await);
    assert!(!m.busy("p1"));
    m.kill("p1").await;
}

#[tokio::test]
async fn busy_ends_when_output_goes_quiet() {
    let (_lock, _, events) = manager();
    let config = Config {
        hook_exe: Some(PathBuf::from(env!("CARGO_BIN_EXE_workbench"))),
        ..Default::default()
    };
    let m = TerminalManager::with_quiet_idle(Arc::new(config), events.clone(), Duration::from_millis(300));
    let dir = tempfile::tempdir().unwrap();
    start_fake(&m, "p1", dir.path()).await;
    let mut a = m.attach("p1").unwrap();
    read_until(&mut a.rx, &a.replay, "FAKE-CLI READY").await;
    let mut ev = events.subscribe();
    m.write("p1", b"\r".to_vec()).unwrap();
    assert!(busy_event(&mut ev, "p1").await);
    assert!(!busy_event(&mut ev, "p1").await);
    assert!(!m.busy("p1"));
    m.kill("p1").await;
}

// ---------------------------------------------------------------------------
// HTTP / WebSocket (daemon routes)

mod http {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::Instant;

    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use futures_util::{SinkExt, StreamExt};
    use http_body_util::BodyExt;
    use serde_json::{json, Value};
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message as WsMessage;
    use tower::ServiceExt;
    use workbench::config::Config;
    use workbench::events::EventBus;
    use workbench::http;
    use workbench::http::terminal::commit_message;
    use workbench::projects::Source;
    use workbench::state::AppState;
    use workbench::git;

    use super::{fake_cli, TERMINAL_TEST_LOCK, TIMEOUT, word_after};

    const AUTH_TOKEN: &str = "abc123def4567890abcdef1234567890ab";

    fn git_cli(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn seed_bare(tmp: &Path) -> PathBuf {
        let src = tmp.join("src");
        std::fs::create_dir_all(&src).unwrap();
        git_cli(&src, &["init", "-b", "main"]);
        git_cli(&src, &["config", "user.name", "Ada"]);
        git_cli(&src, &["config", "user.email", "ada@example.com"]);
        std::fs::write(src.join("package.json"), "{}\n").unwrap();
        git_cli(&src, &["add", "-A"]);
        git_cli(&src, &["commit", "-m", "init"]);
        let bare = tmp.join("remote.git");
        git_cli(
            tmp,
            &["clone", "--bare", src.to_str().unwrap(), "remote.git"],
        );
        bare
    }

    struct HttpEnv {
        _pty_lock: std::sync::MutexGuard<'static, ()>,
        _tmp: tempfile::TempDir,
        root: PathBuf,
        addr: std::net::SocketAddr,
        state: AppState,
        events: EventBus,
        project: String,
        dir: PathBuf,
        server: tokio::task::JoinHandle<()>,
    }

    async fn http_env(token_echo: bool) -> HttpEnv {
        let pty_lock = TERMINAL_TEST_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let agents = root.join("agents.json");
        let cli = fake_cli();
        let agents_json = if token_echo {
            let script = format!("echo TOKEN=$WORKBENCH_HOOK_TOKEN; exec {cli}");
            json!({ "fake": ["sh", "-c", script] })
        } else {
            json!({ "fake": [cli] })
        };
        std::fs::write(&agents, agents_json.to_string()).unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let config = Config {
            port: addr.port(),
            projects_dir: root.clone(),
            no_auth: false,
            token: Some(AUTH_TOKEN.into()),
            agents_config: Some(agents),
            hook_exe: Some(PathBuf::from(env!("CARGO_BIN_EXE_workbench"))),
            ..Default::default()
        };
        let state = AppState::with_scaffold_install(config, false);
        let events = state.events.clone();
        let info = state.projects.create("Site").await.unwrap();
        let dir = root.join(&info.id);
        let state2 = state.clone();
        let server = tokio::spawn(async move {
            workbench::serve_on(listener, state2).await.unwrap();
        });
        HttpEnv {
            _pty_lock: pty_lock,
            _tmp: tmp,
            root,
            addr,
            state,
            events,
            project: info.id,
            dir,
            server,
        }
    }

    fn http_req(method: &str, path: &str, body: Option<Value>) -> Request<Body> {
        let mut b = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, "localhost")
            .header(header::AUTHORIZATION, format!("Bearer {AUTH_TOKEN}"));
        let body = match body {
            Some(v) => {
                b = b.header(header::CONTENT_TYPE, "application/json");
                Body::from(v.to_string())
            }
            None => Body::empty(),
        };
        b.body(body).unwrap()
    }

    async fn call(app: &axum::Router, r: Request<Body>) -> (StatusCode, Value) {
        let res = app.clone().oneshot(r).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    fn app(env: &HttpEnv) -> axum::Router {
        http::router(env.state.clone())
    }

    async fn call_http(env: &HttpEnv, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        call(&app(env), http_req(method, path, body)).await
    }

    async fn post_turn_end(env: &HttpEnv, body: Value) -> (StatusCode, Value) {
        let req = Request::builder()
            .method("POST")
            .uri("/api/terminal/turn-end")
            .header(header::HOST, "localhost")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        call(&app(env), req).await
    }

    async fn start_terminal(env: &HttpEnv) {
        let (s, v) = call_http(
            env,
            "POST",
            "/api/terminal",
            Some(json!({ "project": env.project, "provider": "fake" })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        assert_eq!(v["running"], true);
    }

    async fn ws_connect(env: &HttpEnv) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
        let url = format!(
            "ws://{}/api/terminal/{}/ws?t={}",
            env.addr,
            env.project,
            AUTH_TOKEN
        );
        let (ws, _) = connect_async(url).await.unwrap();
        ws
    }

    async fn hook_token_from_ws(env: &HttpEnv) -> String {
        let mut ws = ws_connect(env).await;
        hook_token_from_ws_stream(&mut ws).await
    }

    #[tokio::test]
    async fn start_refused_on_main_with_remote() {
        let env = http_env(false).await;
        let bare = seed_bare(&env.root.join("git"));
        let gh = env
            .state
            .projects
            .create_from(
                "gh-site",
                Source::Clone {
                    repo: "acme/site".into(),
                    url: format!("file://{}", bare.display()),
                    env: vec![],
                },
                &git::Identity::workbench(),
            )
            .await
            .unwrap();
        let (s, v) = call_http(
            &env,
            "POST",
            "/api/terminal",
            Some(json!({ "project": gh.id, "provider": "fake" })),
        )
        .await;
        assert_eq!(s, StatusCode::CONFLICT, "{v}");
        assert_eq!(v["error"]["code"], "CHANGE_REQUIRED");
        env.server.abort();
    }

    #[tokio::test]
    async fn get_latest_runs_while_terminal_is_live() {
        let env = http_env(false).await;
        start_terminal(&env).await;
        let (s, v) = call_http(
            &env,
            "POST",
            "/api/changes/update",
            Some(json!({ "project": env.project })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");

        let (s, v) = call_http(
            &env,
            "DELETE",
            &format!("/api/terminal/{}", env.project),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");

        let (s, v) = call_http(
            &env,
            "POST",
            "/api/changes/update",
            Some(json!({ "project": env.project })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        env.server.abort();
    }

    #[tokio::test]
    async fn ws_first_frame_is_replay() {
        let env = http_env(false).await;
        start_terminal(&env).await;
        let mut ws = ws_connect(&env).await;
        let msg = tokio::time::timeout(TIMEOUT, ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let bytes = match msg {
            WsMessage::Binary(b) => b,
            other => panic!("expected binary replay, got {other:?}"),
        };
        assert!(
            String::from_utf8_lossy(&bytes).contains("FAKE-CLI READY"),
            "{bytes:?}"
        );
        env.server.abort();
    }

    #[tokio::test]
    async fn ws_resize_and_input() {
        let env = http_env(false).await;
        start_terminal(&env).await;
        let mut ws = ws_connect(&env).await;
        let _ = tokio::time::timeout(TIMEOUT, ws.next()).await;
        ws.send(WsMessage::Text(r#"{"cols":100,"rows":30}"#.into()))
            .await
            .unwrap();
        ws.send(WsMessage::Binary(b"size\r".to_vec().into()))
            .await
            .unwrap();
        let deadline = Instant::now() + TIMEOUT;
        let mut acc = String::new();
        while Instant::now() < deadline {
            if acc.contains("30 100") {
                break;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if let Ok(Some(Ok(WsMessage::Binary(b)))) =
                tokio::time::timeout(left, ws.next()).await
            {
                acc.push_str(&String::from_utf8_lossy(&b));
            }
        }
        assert!(acc.contains("30 100"), "got {acc:?}");
        env.server.abort();
    }

    #[tokio::test]
    async fn two_clients_both_receive_output() {
        let env = http_env(false).await;
        start_terminal(&env).await;
        let mut a = ws_connect(&env).await;
        let mut b = ws_connect(&env).await;
        let _ = tokio::time::timeout(TIMEOUT, a.next()).await;
        let _ = tokio::time::timeout(TIMEOUT, b.next()).await;
        a.send(WsMessage::Binary(b"hello\r".to_vec().into()))
            .await
            .unwrap();
        let deadline = Instant::now() + TIMEOUT;
        let mut saw_a = false;
        let mut saw_b = false;
        while Instant::now() < deadline && (!saw_a || !saw_b) {
            tokio::select! {
                m = a.next() => if let Some(Ok(WsMessage::Binary(bin))) = m {
                    if String::from_utf8_lossy(&bin).contains("ECHO hello") { saw_a = true; }
                },
                m = b.next() => if let Some(Ok(WsMessage::Binary(bin))) = m {
                    if String::from_utf8_lossy(&bin).contains("ECHO hello") { saw_b = true; }
                },
            }
        }
        assert!(saw_a && saw_b);
        env.server.abort();
    }

    #[tokio::test]
    async fn edit_turn_creates_checkpoint() {
        let env = http_env(false).await;
        let mut ev = env.events.subscribe();
        start_terminal(&env).await;
        let mut ws = ws_connect(&env).await;
        let _ = tokio::time::timeout(TIMEOUT, ws.next()).await;
        ws.send(WsMessage::Binary(b"edit hello\r".to_vec().into()))
            .await
            .unwrap();
        let deadline = Instant::now() + TIMEOUT;
        let mut got_cp = false;
        while Instant::now() < deadline && !got_cp {
            let left = deadline.saturating_duration_since(Instant::now());
            if let Ok(Ok(text)) = tokio::time::timeout(left, ev.recv()).await {
                let v: Value = serde_json::from_str(&text).unwrap();
                if v["type"] == "checkpoint.created" && v["projectId"] == env.project {
                    assert_eq!(v["summary"], "hello");
                    got_cp = true;
                }
            }
        }
        assert!(got_cp, "checkpoint.created not seen");
        let subject = git_cli(&env.dir, &["log", "-1", "--format=%s"]);
        assert_eq!(subject, "checkpoint: hello");
        env.server.abort();
    }

    #[tokio::test]
    async fn new_change_saves_edits_left_on_the_previous_change() {
        let env = http_env(false).await;
        let new_change = |title: &str| {
            call_http(
                &env,
                "POST",
                "/api/changes",
                Some(json!({ "project": env.project, "title": title })),
            )
        };
        let (s, v) = new_change("First").await;
        assert_eq!(s, StatusCode::CREATED, "{v}");
        let tracked = git_cli(&env.dir, &["ls-files"]);
        let file = tracked.lines().next().unwrap();
        std::fs::write(env.dir.join(file), "left over\n").unwrap();

        let (s, v) = new_change("Second").await;
        assert_eq!(s, StatusCode::CREATED, "{v}");
        assert_eq!(
            git_cli(&env.dir, &["log", "-1", "--format=%s", "content/first"]),
            "checkpoint: Saved edits"
        );
        assert_eq!(git_cli(&env.dir, &["status", "--porcelain", "--untracked-files=no"]), "");
        env.server.abort();
    }

    #[tokio::test]
    async fn skipped_change_catches_up_at_turn_end() {
        let env = http_env(true).await;
        let (s, v) = call_http(
            &env,
            "POST",
            "/api/changes",
            Some(json!({ "project": env.project, "title": "Landing" })),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED, "{v}");
        let branch = v["branch"].as_str().unwrap().to_string();
        let wt = env.root.join("main-wt");
        git_cli(&env.dir, &["worktree", "add", wt.to_str().unwrap(), "main"]);
        std::fs::write(wt.join("news.md"), "fresh\n").unwrap();
        git_cli(&wt, &["add", "news.md"]);
        git_cli(&wt, &["commit", "-m", "news"]);
        git_cli(&env.dir, &["worktree", "remove", wt.to_str().unwrap()]);

        let mut ev = env.events.subscribe();
        start_terminal(&env).await;
        let token = hook_token_from_ws(&env).await;
        let (s, v) = call_http(
            &env,
            "POST",
            "/api/changes/update",
            Some(json!({ "project": env.project })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        assert_eq!(v["skipped"]["branch"], branch);
        assert!(!env.dir.join("news.md").exists());

        let (s, _) = post_turn_end(&env, json!({ "project": env.project, "token": token })).await;
        assert_eq!(s, StatusCode::OK);
        let deadline = Instant::now() + TIMEOUT;
        let mut updated = None;
        while Instant::now() < deadline && updated.is_none() {
            let left = deadline.saturating_duration_since(Instant::now());
            if let Ok(Ok(text)) = tokio::time::timeout(left, ev.recv()).await {
                let v: Value = serde_json::from_str(&text).unwrap();
                if v["type"] == "changes.updated" && v["projectId"] == env.project {
                    updated = Some(v);
                }
            }
        }
        let updated = updated.expect("changes.updated not seen");
        assert_eq!(updated["result"]["updated"][0]["branch"], branch, "{updated}");
        assert!(env.dir.join("news.md").exists());
        env.server.abort();
    }

    #[tokio::test]
    async fn turn_end_without_changes_makes_no_commit() {
        let env = http_env(true).await;
        start_terminal(&env).await;
        let token = hook_token_from_ws(&env).await;
        let before = git_cli(&env.dir, &["rev-parse", "HEAD"]);
        let (s, v) = post_turn_end(
            &env,
            json!({ "project": env.project, "token": token }),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v["committed"], false);
        assert_eq!(git_cli(&env.dir, &["rev-parse", "HEAD"]), before);
        env.server.abort();
    }

    #[tokio::test]
    async fn sessions_are_recorded_and_titled_by_turn_end() {
        let env = http_env(true).await;
        let (s, v) = call_http(
            &env,
            "POST",
            "/api/terminal",
            Some(json!({ "project": env.project, "provider": "fake" })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let id = v["sessionId"].as_str().unwrap().to_string();
        let (_, status) = call_http(&env, "GET", &format!("/api/terminal/{}", env.project), None).await;
        assert_eq!(status["sessionId"], id.as_str());

        let token = hook_token_from_ws(&env).await;
        let (s, _) = post_turn_end(
            &env,
            json!({ "project": env.project, "token": token, "summary": "Add a pricing page\nmore", "sessionId": "agent-1" }),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let sessions_path = format!("/api/terminal/{}/sessions", env.project);
        let (s, v) = call_http(&env, "GET", &sessions_path, None).await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let list = v["sessions"].as_array().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["id"], id.as_str());
        assert_eq!(list[0]["provider"], "fake");
        assert_eq!(list[0]["title"], "Add a pricing page");
        // `fake` is not a provider the daemon knows how to resume.
        assert_eq!(list[0]["resumable"], false);

        call_http(&env, "DELETE", &format!("/api/terminal/{}", env.project), None).await;
        let (s, v) = call_http(
            &env,
            "POST",
            "/api/terminal",
            Some(json!({ "project": env.project, "provider": "fake", "resume": id })),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
        let (s, _) = call_http(
            &env,
            "POST",
            "/api/terminal",
            Some(json!({ "project": env.project, "provider": "fake", "resume": "nope" })),
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        env.server.abort();
    }

    fn attachment_req(project: &str, content_type: &str, bytes: &'static [u8]) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(format!("/api/terminal/{project}/attachments"))
            .header(header::HOST, "localhost")
            .header(header::AUTHORIZATION, format!("Bearer {AUTH_TOKEN}"))
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(bytes))
            .unwrap()
    }

    #[tokio::test]
    async fn attachment_is_saved_outside_the_project() {
        let env = http_env(false).await;
        let (s, v) = call(&app(&env), attachment_req(&env.project, "image/png", b"png-bytes")).await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let path = PathBuf::from(v["path"].as_str().unwrap());
        assert!(path.is_absolute(), "{}", path.display());
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("png"));
        assert!(!path.starts_with(&env.dir), "{}", path.display());
        assert_eq!(std::fs::read(&path).unwrap(), b"png-bytes");

        let (s, _) = call(&app(&env), attachment_req(&env.project, "text/plain", b"hi")).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        let (s, _) = call(&app(&env), attachment_req("nope", "image/png", b"x")).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        env.server.abort();
    }

    #[tokio::test]
    async fn turn_end_bad_token_is_403() {
        let env = http_env(false).await;
        start_terminal(&env).await;
        let (s, v) = post_turn_end(
            &env,
            json!({
                "project": env.project,
                "token": "0".repeat(32),
            }),
        )
        .await;
        assert_eq!(s, StatusCode::FORBIDDEN);
        assert_eq!(v["error"]["code"], "UNAUTH");
        env.server.abort();
    }

    #[tokio::test]
    async fn turn_end_blocked_on_main_emits_event() {
        let env = http_env(true).await;
        let bare = seed_bare(&env.root.join("gh"));
        let gh = env
            .state
            .projects
            .create_from(
                "gh-site",
                Source::Clone {
                    repo: "acme/site".into(),
                    url: format!("file://{}", bare.display()),
                    env: vec![],
                },
                &git::Identity::workbench(),
            )
            .await
            .unwrap();
        let gh_dir = env.root.join(&gh.id);
        workbench::changes::create(&gh_dir, "New post: Launch week")
            .await
            .unwrap();
        let (s, v) = call_http(
            &env,
            "POST",
            "/api/terminal",
            Some(json!({ "project": gh.id, "provider": "fake" })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        git_cli(&gh_dir, &["checkout", "main"]);
        let tracked = git_cli(&gh_dir, &["ls-files"]);
        std::fs::write(gh_dir.join(tracked.lines().next().unwrap()), "edited on main\n").unwrap();
        let mut ws = ws_connect_for_project(&env, &gh.id).await;
        let token = hook_token_from_ws_stream(&mut ws).await;
        let mut ev = env.events.subscribe();
        let (s, v) = post_turn_end(
            &env,
            json!({ "project": gh.id, "token": token }),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v["committed"], false);
        let deadline = Instant::now() + TIMEOUT;
        let mut blocked = false;
        while Instant::now() < deadline && !blocked {
            let left = deadline.saturating_duration_since(Instant::now());
            if let Ok(Ok(text)) = tokio::time::timeout(left, ev.recv()).await {
                let v: Value = serde_json::from_str(&text).unwrap();
                if v["type"] == "checkpoint.blocked" && v["projectId"] == gh.id {
                    blocked = true;
                }
            }
        }
        assert!(blocked);
        env.server.abort();
    }

    async fn ws_connect_for_project(
        env: &HttpEnv,
        project: &str,
    ) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
        let url = format!("ws://{}/api/terminal/{}/ws?t={}", env.addr, project, AUTH_TOKEN);
        let (ws, _) = connect_async(url).await.unwrap();
        ws
    }

    async fn hook_token_from_ws_stream(
        ws: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> String {
        let deadline = Instant::now() + TIMEOUT;
        let first = tokio::time::timeout(TIMEOUT, ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let mut acc = match first {
            WsMessage::Binary(b) => String::from_utf8_lossy(&b).into_owned(),
            _ => String::new(),
        };
        while Instant::now() < deadline {
            if let Some(tok) = word_after(&acc, "TOKEN=", |c| c.is_ascii_hexdigit()) {
                return tok;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if let Ok(Some(Ok(WsMessage::Binary(b)))) =
                tokio::time::timeout(left, ws.next()).await
            {
                acc.push_str(&String::from_utf8_lossy(&b));
            }
        }
        panic!("no TOKEN= in replay: {acc:?}");
    }

    #[tokio::test]
    async fn delete_project_kills_terminal() {
        let env = http_env(false).await;
        start_terminal(&env).await;
        let (s, v) = call_http(
            &env,
            "DELETE",
            &format!("/api/terminal/{}", env.project),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let (s, v) = call_http(
            &env,
            "GET",
            &format!("/api/terminal/{}", env.project),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        assert_eq!(v["running"], false);

        start_terminal(&env).await;
        let (s, v) = call(
            &app(&env),
            http_req(
                "DELETE",
                &format!("/api/projects/{}?confirm=true", env.project),
                None,
            ),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        let (s, v) = call_http(
            &env,
            "GET",
            &format!("/api/terminal/{}", env.project),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        assert_eq!(v["running"], false);
        env.server.abort();
    }

    #[test]
    fn commit_message_is_one_line_and_char_safe() {
        assert_eq!(commit_message(Some("a\nb"), &[]), "a");
        let long = "é".repeat(500);
        let msg = commit_message(Some(&long), &[]);
        assert!(msg.chars().count() <= 72);
        assert!(msg.is_char_boundary(msg.len()));
        assert_eq!(
            commit_message(None, &["x".into(), "y".into()]),
            "Update 2 files: x, y"
        );
        assert_eq!(commit_message(Some(""), &["x".into()]), "Update 1 files: x");
    }
}

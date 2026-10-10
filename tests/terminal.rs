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

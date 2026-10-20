//! Dev-server supervision, driven by stand-in shell scripts (no npm/Astro needed), plus an
//! ignored end-to-end test with the real thing.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::sync::broadcast;
use tower::ServiceExt;
use orso::config::Config;
use orso::devserver::{DevServerOptions, MAX_RESTARTS};
use orso::http;
use orso::http::ctl::preview_status_value;
use orso::state::AppState;

const TIMEOUT: Duration = Duration::from_secs(10);

fn config(dir: &Path) -> Config {
    Config {
        port: 0,
        origin: None,
        projects_dir: dir.to_path_buf(),
        no_auth: true,
        token: None,
        agents_config: None,
        ..Default::default()
    }
}

/// A stand-in dev server: `sh -c script`, run in the project dir with `$PORT` and
/// `$PUBLIC_BASE` set. Backoff is shrunk so crash loops finish quickly.
fn script(body: &'static str) -> DevServerOptions {
    let mut opts = DevServerOptions::custom(move |spec| {
        let mut c = Command::new("sh");
        c.arg("-c").arg(body).env("PORT", spec.port.to_string());
        c
    });
    opts.backoff_base = Duration::from_millis(10);
    opts
}

/// The configured launcher (`orso.json`), with a shrunk crash backoff.
fn configured() -> DevServerOptions {
    let mut opts = DevServerOptions::configured(0);
    opts.backoff_base = Duration::from_millis(50);
    opts
}

const READY: &str = r#"
echo $$ >> spawns.log
echo "$PUBLIC_BASE $PORT" > env.txt
echo "  ➜  Local:   http://127.0.0.1:$PORT$PUBLIC_BASE"
exec sleep 300
"#;

const CRASH: &str = r#"
echo x >> spawns.log
echo "boom-output"
exit 1
"#;

const FLAKY: &str = r#"
echo x >> spawns.log
if [ -f ok ]; then
  echo "  Local  http://127.0.0.1:$PORT$PUBLIC_BASE"
  exec sleep 300
fi
echo "not yet"
exit 1
"#;

const TREE: &str = r#"
echo $$ > pid
sleep 300 &
echo $! > childpid
echo "  Local  http://127.0.0.1:$PORT$PUBLIC_BASE"
wait
"#;

const SHORT_LIVED: &str = r#"
echo x >> spawns.log
echo "  Local  http://127.0.0.1:$PORT$PUBLIC_BASE"
sleep 0.4
exit 1
"#;

struct Env {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    state: AppState,
    app: axum::Router,
}

fn env_with(opts: DevServerOptions) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let state = AppState::with_options(config(&root), false, opts);
    let app = http::router(state.clone());
    Env {
        _tmp: tmp,
        root,
        state,
        app,
    }
}

impl Env {
    async fn call(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut b = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::HOST, "localhost");
        let body = match body {
            Some(v) => {
                b = b.header(header::CONTENT_TYPE, "application/json");
                Body::from(v.to_string())
            }
            None => Body::empty(),
        };
        let res = self
            .app
            .clone()
            .oneshot(b.body(body).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn create(&self, name: &str) -> Value {
        let (s, v) = self
            .call("POST", "/api/projects", Some(json!({ "name": name })))
            .await;
        assert_eq!(s, StatusCode::CREATED, "{v}");
        v
    }

    fn events(&self) -> Events {
        Events {
            rx: self.state.events.subscribe(),
            seen: Vec::new(),
        }
    }

    fn file(&self, id: &str, name: &str) -> PathBuf {
        self.root.join(id).join(name)
    }

    async fn devserver_state(&self, id: &str) -> String {
        let (_, v) = self.call("GET", "/api/projects", None).await;
        v["projects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == id)
            .unwrap()["devserver"]
            .as_str()
            .unwrap()
            .to_string()
    }
}

struct Events {
    rx: broadcast::Receiver<Arc<str>>,
    seen: Vec<Value>,
}

impl Events {
    /// Wait for a `devserver.status` of `state` for `id`, skipping earlier events.
    async fn status(&mut self, id: &str, state: &str) -> Value {
        let deadline = tokio::time::sleep(TIMEOUT);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                ev = self.rx.recv() => {
                    let v: Value = serde_json::from_str(&ev.unwrap()).unwrap();
                    if v["type"] == "devserver.status" && v["projectId"] == id {
                        self.seen.push(v.clone());
                        if v["state"] == state {
                            return v;
                        }
                    }
                }
                _ = &mut deadline => panic!("no {state} status for {id}; saw {:?}", self.seen),
            }
        }
    }
}

async fn eventually(what: &str, mut cond: impl FnMut() -> bool) {
    let start = Instant::now();
    while !cond() {
        assert!(start.elapsed() < TIMEOUT, "timed out: {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only probes for existence.
    unsafe { libc::kill(pid, 0) == 0 }
}

fn read_pid(path: &Path) -> i32 {
    std::fs::read_to_string(path)
        .unwrap()
        .trim()
        .parse()
        .unwrap()
}

#[tokio::test]
async fn create_supervises_dev_server_with_public_base_and_reports_ready() {
    let env = env_with(script(READY));
    let mut events = env.events();
    let info = env.create("Site").await;
    assert_eq!(info["id"], "site");
    assert_eq!(info["devserver"], "starting");
    let port = info["port"].as_u64().unwrap();

    let ready = events.status("site", "ready").await;
    assert_eq!(ready["port"].as_u64().unwrap(), port);
    assert_eq!(env.devserver_state("site").await, "ready");
    // The process saw the preview prefix and its port.
    assert_eq!(
        lines(&env.file("site", "env.txt")),
        [format!("/preview/site/ {port}")]
    );
    env.state.devservers.shutdown().await;
}

#[tokio::test]
async fn crashes_restart_at_most_three_times_then_park_as_crashed_until_manual_restart() {
    let env = env_with(script(FLAKY));
    let mut events = env.events();
    env.create("Site").await;

    let crashed = events.status("site", "crashed").await;
    let msg = crashed["message"].as_str().unwrap();
    assert!(
        msg.contains("exited with") && msg.contains("not yet"),
        "{msg}"
    );
    assert!(crashed["port"].is_u64());
    // Initial run + MAX_RESTARTS restarts, and then it stops trying.
    let spawns = env.file("site", "spawns.log");
    assert_eq!(lines(&spawns).len() as u32, 1 + MAX_RESTARTS);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(lines(&spawns).len() as u32, 1 + MAX_RESTARTS);
    assert_eq!(env.devserver_state("site").await, "crashed");

    // The user fixes the problem and restarts: fresh budget, comes up.
    std::fs::write(env.file("site", "ok"), "").unwrap();
    let (s, v) = env
        .call("POST", "/api/projects/site/devserver/restart", None)
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["id"], "site");
    assert_eq!(v["devserver"], "starting");
    events.status("site", "ready").await;
    assert_eq!(env.devserver_state("site").await, "ready");
    env.state.devservers.shutdown().await;
}

#[tokio::test]
async fn restart_budget_resets_after_a_stable_run() {
    let mut opts = script(SHORT_LIVED);
    opts.stable_after = Duration::from_millis(100);
    let env = env_with(opts);
    let mut events = env.events();
    env.create("Site").await;
    // A ready server that dies is `starting` again (restarting), then ready once more.
    events.status("site", "ready").await;
    events.status("site", "starting").await;
    events.status("site", "ready").await;
    // Each run is ready for 400ms > stable_after, so the crash budget never runs out.
    let spawns = env.file("site", "spawns.log");
    eventually("more restarts than the crash budget allows", || {
        lines(&spawns).len() as u32 > 2 * (1 + MAX_RESTARTS)
    })
    .await;
    assert_ne!(env.devserver_state("site").await, "crashed");
    env.state.devservers.shutdown().await;
}

#[tokio::test]
async fn startup_crashes_only_publish_the_terminal_state_with_the_output_tail() {
    let env = env_with(script(CRASH));
    let mut events = env.events();
    let info = env.create("Site").await;
    assert_eq!(info["devserver"], "starting");
    let crashed = events.status("site", "crashed").await;
    // The project never left `starting` while retrying, so nothing else was published.
    assert_eq!(events.seen.len(), 1, "{:?}", events.seen);
    assert!(crashed["message"].as_str().unwrap().contains("boom-output"));
}

#[tokio::test]
async fn ctl_preview_status_carries_the_crash_output_until_a_restart() {
    let env = env_with(script(FLAKY));
    let mut events = env.events();
    env.create("Site").await;
    events.status("site", "crashed").await;
    let v = preview_status_value(&env.state, "site").await.unwrap();
    assert_eq!(v["devserver"], "crashed", "{v}");
    assert!(v["message"].as_str().unwrap().contains("not yet"), "{v}");

    std::fs::write(env.file("site", "ok"), "").unwrap();
    env.state.devservers.start("site").await.unwrap();
    events.status("site", "ready").await;
    let v = preview_status_value(&env.state, "site").await.unwrap();
    assert_eq!(v["devserver"], "ready", "{v}");
    assert!(v.get("message").is_none(), "{v}");
    env.state.devservers.shutdown().await;
}

/// Make every registry write fail: `projects.json` becomes a directory, so the atomic
/// rename of the temp file onto it errors.
fn break_registry_persistence(root: &Path) {
    let f = root.join(".orso/projects.json");
    std::fs::remove_file(&f).unwrap();
    std::fs::create_dir(&f).unwrap();
}

fn restore_registry_persistence(root: &Path) {
    std::fs::remove_dir(root.join(".orso/projects.json")).unwrap();
}

const WRONG_PORT: &str = r#"
echo x >> spawns.log
echo "  Local  http://127.0.0.1:$((PORT + 1))/"
exec sleep 300
"#;

#[tokio::test]
async fn unrecordable_reported_port_is_never_announced_ready() {
    let env = env_with(script(WRONG_PORT));
    let info = env.state.projects.create("Site").await.unwrap();
    let mut events = env.events();
    break_registry_persistence(&env.root);
    env.state.devservers.start("site").await.unwrap();

    let crashed = events.status("site", "crashed").await;
    assert!(
        crashed["message"]
            .as_str()
            .unwrap()
            .contains("could not be recorded"),
        "{crashed}"
    );
    assert!(
        events.seen.iter().all(|e| e["state"] != "ready"),
        "{:?}",
        events.seen
    );
    // Registry and proxy routing still agree on the original port.
    let p = env.state.projects.get("site").await.unwrap();
    assert_eq!(p.port, info.port);
    // Every attempt's process was killed.
    assert_eq!(
        lines(&env.file("site", "spawns.log")).len() as u32,
        1 + MAX_RESTARTS
    );
    env.state.devservers.shutdown().await;
}

#[tokio::test]
async fn start_failure_marks_project_crashed_and_restart_recovers() {
    let env = env_with(script(READY));
    let info = env.state.projects.create("Site").await.unwrap();
    let mut events = env.events();
    // Port taken + registry unwritable => no usable port can be assigned.
    let squatter = std::net::TcpListener::bind(("127.0.0.1", info.port)).unwrap();
    break_registry_persistence(&env.root);

    let err = env.state.devservers.start("site").await.unwrap_err();
    assert_eq!(err.code.as_str(), "IO");
    let crashed = events.status("site", "crashed").await;
    assert!(
        crashed["message"]
            .as_str()
            .unwrap()
            .contains("cannot start"),
        "{crashed}"
    );
    assert_eq!(env.devserver_state("site").await, "crashed");

    // Fixed: the restart endpoint recovers it (on a fresh port, since ours is squatted).
    restore_registry_persistence(&env.root);
    let (s, v) = env
        .call("POST", "/api/projects/site/devserver/restart", None)
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["devserver"], "starting");
    assert_ne!(v["port"].as_u64().unwrap(), info.port as u64);
    events.status("site", "ready").await;
    drop(squatter);
    env.state.devservers.shutdown().await;
}

#[tokio::test]
async fn manual_restart_replaces_the_running_process() {
    let env = env_with(script(READY));
    let mut events = env.events();
    env.create("Site").await;
    events.status("site", "ready").await;
    let first = read_pid(&env.file("site", "spawns.log"));

    let (s, v) = env
        .call("POST", "/api/projects/site/devserver/restart", None)
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["devserver"], "starting");
    events.status("site", "ready").await;
    let all = lines(&env.file("site", "spawns.log"));
    assert_eq!(all.len(), 2);
    assert!(!alive(first), "old process must be gone");
    env.state.devservers.shutdown().await;
}

#[tokio::test]
async fn restart_unknown_project_is_404() {
    let env = env_with(script(READY));
    let (s, v) = env
        .call("POST", "/api/projects/ghost/devserver/restart", None)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(v["error"]["code"], "ENOENT");
}

#[tokio::test]
async fn delete_stops_the_whole_process_tree() {
    let env = env_with(script(TREE));
    let mut events = env.events();
    env.create("Site").await;
    events.status("site", "ready").await;
    let pid = read_pid(&env.file("site", "pid"));
    let child = read_pid(&env.file("site", "childpid"));
    assert!(alive(pid) && alive(child));

    let (s, v) = env
        .call("DELETE", "/api/projects/site?confirm=true", None)
        .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    eventually("dev server process gone", || !alive(pid)).await;
    eventually("dev server child gone", || !alive(child)).await;
    // Nothing restarts it afterwards.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!env.root.join("site").exists());
}

#[tokio::test]
async fn daemon_shutdown_stops_dev_servers() {
    let env = env_with(script(TREE));
    let mut events = env.events();
    env.create("Site").await;
    events.status("site", "ready").await;
    let pid = read_pid(&env.file("site", "pid"));
    let child = read_pid(&env.file("site", "childpid"));

    env.state.devservers.shutdown().await;
    eventually("dev server process gone", || !alive(pid)).await;
    eventually("dev server child gone", || !alive(child)).await;
}

#[tokio::test]
async fn daemon_restart_resumes_projects_and_reallocates_a_busy_port() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().to_path_buf();
    let (busy_port, free_port) = {
        let first = AppState::with_options(config(&root), false, script(READY));
        let a = first.projects.create("Alpha").await.unwrap();
        let b = first.projects.create("Beta").await.unwrap();
        (a.port, b.port)
    };
    // Something else grabbed Alpha's port while the daemon was down.
    let squatter = std::net::TcpListener::bind(("127.0.0.1", busy_port)).unwrap();

    let state = AppState::with_options(config(&root), false, script(READY));
    let mut events = state.events.subscribe();
    state.devservers.resume_all().await;

    let mut ready = std::collections::HashMap::new();
    let start = Instant::now();
    while ready.len() < 2 {
        assert!(start.elapsed() < TIMEOUT, "ready so far: {ready:?}");
        let ev: Value = serde_json::from_str(&events.recv().await.unwrap()).unwrap();
        if ev["type"] == "devserver.status" && ev["state"] == "ready" {
            ready.insert(
                ev["projectId"].as_str().unwrap().to_string(),
                ev["port"].as_u64().unwrap() as u16,
            );
        }
    }
    assert_ne!(ready["alpha"], busy_port, "busy port must be replaced");
    assert_eq!(ready["beta"], free_port, "free port is kept");
    // The process really ran on the new port, and it was persisted.
    let env_txt = std::fs::read_to_string(root.join("alpha/env.txt")).unwrap();
    assert_eq!(
        env_txt.trim(),
        format!("/preview/alpha/ {}", ready["alpha"])
    );
    let persisted: Value =
        serde_json::from_slice(&std::fs::read(root.join(".orso/projects.json")).unwrap())
            .unwrap();
    let alpha = persisted
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "alpha")
        .unwrap();
    assert_eq!(alpha["port"].as_u64().unwrap() as u16, ready["alpha"]);
    drop(squatter);
    state.devservers.shutdown().await;
}

#[tokio::test]
async fn configured_dev_command_reaches_ready() {
    let env = env_with(configured());
    let info = env.state.projects.create("Site").await.unwrap();
    let dir = env.file("site", "");
    std::fs::create_dir_all(dir.join("site")).unwrap();
    std::fs::write(dir.join("site/astro.config.mjs"), "export default {};\n").unwrap();
    std::fs::write(
        dir.join("orso.json"),
        r#"{"root":"site","dev":"pwd > ../cwd.txt; echo Local http://127.0.0.1:{port}/; sleep 30"}"#,
    )
    .unwrap();
    let mut events = env.events();
    env.state.devservers.start("site").await.unwrap();

    let ready = events.status("site", "ready").await;
    assert_eq!(ready["port"].as_u64().unwrap(), info.port as u64);
    assert_eq!(env.devserver_state("site").await, "ready");
    // The process ran with `site` (the configured root) as its cwd.
    let cwd = std::fs::read_to_string(dir.join("cwd.txt")).unwrap();
    assert!(cwd.trim().ends_with("/site"), "cwd: {cwd}");
    env.state.devservers.shutdown().await;
}

#[tokio::test]
async fn invalid_settings_crash_without_retry() {
    let env = env_with(configured());
    env.state.projects.create("Site").await.unwrap();
    std::fs::write(env.file("site", "orso.json"), r#"{"dev":"pnpm dev"}"#).unwrap();
    let mut events = env.events();
    env.state.devservers.start("site").await.unwrap();

    let crashed = events.status("site", "crashed").await;
    let msg = crashed["message"].as_str().unwrap();
    assert!(msg.contains("orso.json: dev must contain {port}"), "{msg}");
    let seen_at_crash = events.seen.len();
    // Past 2 x backoff_base (50ms) it must still be parked, with no restart.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(env.devserver_state("site").await, "crashed");
    while let Ok(ev) = events.rx.try_recv() {
        let v: Value = serde_json::from_str(&ev).unwrap();
        if v["type"] == "devserver.status" && v["projectId"] == "site" {
            events.seen.push(v);
        }
    }
    assert_eq!(events.seen.len(), seen_at_crash, "{:?}", events.seen);
    env.state.devservers.shutdown().await;
}

#[tokio::test]
async fn invalid_settings_at_resume() {
    let env = env_with(configured());
    env.state.projects.create("Site").await.unwrap();
    std::fs::write(env.file("site", "orso.json"), r#"{"dev":"pnpm dev"}"#).unwrap();
    let mut events = env.events();
    env.state.devservers.resume_all().await;

    let crashed = events.status("site", "crashed").await;
    let msg = crashed["message"].as_str().unwrap();
    assert!(msg.contains("orso.json: dev must contain {port}"), "{msg}");
    assert_eq!(env.devserver_state("site").await, "crashed");
    env.state.devservers.shutdown().await;
}

#[tokio::test]
async fn settings_change_restarts() {
    let env = env_with(configured());
    env.state.projects.create("Site").await.unwrap();
    std::fs::write(env.file("site", "package.json"), r#"{"name":"site"}"#).unwrap();
    std::fs::write(
        env.file("site", "orso.json"),
        r#"{"install":"true","dev":"echo Local http://127.0.0.1:{port}/; sleep 30"}"#,
    )
    .unwrap();
    let mut events = env.events();
    env.state.devservers.start("site").await.unwrap();
    events.status("site", "ready").await;

    std::fs::write(
        env.file("site", "orso.json"),
        r#"{"install":"true","dev":"echo Local http://127.0.0.1:{port}/; sleep 31"}"#,
    )
    .unwrap();
    // The deps poll notices the edited orso.json: a new starting, then ready.
    events.status("site", "starting").await;
    events.status("site", "ready").await;
    assert_eq!(env.devserver_state("site").await, "ready");
    env.state.devservers.shutdown().await;
}

#[tokio::test]
async fn disabled_supervision_leaves_projects_starting_and_restart_is_a_noop() {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::with_scaffold_install(config(tmp.path()), false);
    let app = http::router(state.clone());
    let info = state.projects.create("Site").await.unwrap();
    assert!(!state.devservers.enabled());
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/projects/site/devserver/restart")
                .header(header::HOST, "localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let v: Value =
        serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(v["devserver"], "starting");
    assert_eq!(v["port"].as_u64().unwrap() as u16, info.port);
}

/// Real npm + Astro: create a project (network for `npm install`), wait for the dev server,
/// fetch the site through the preview proxy, and check nothing lingers after shutdown.
/// Run with `cargo test -- --ignored`.
#[tokio::test]
#[ignore = "needs network + npm"]
async fn real_astro_dev_server_is_served_through_the_preview_proxy() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::new(config(tmp.path()));
    let mut events = state.events.subscribe();
    let app = http::router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    let body = json!({ "name": "Real Site" }).to_string();
    s.write_all(
        format!(
            "POST /api/projects HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    )
    .await
    .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    assert!(out.starts_with("HTTP/1.1 201"), "{out}");

    let deadline = tokio::time::sleep(Duration::from_secs(120));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            ev = events.recv() => {
                let v: Value = serde_json::from_str(&ev.unwrap()).unwrap();
                if v["type"] == "devserver.status" && v["state"] == "ready" { break; }
                assert_ne!(v["state"], "crashed", "{v}");
            }
            _ = &mut deadline => panic!("dev server never became ready"),
        }
    }

    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    s.write_all(
        b"GET /preview/real-site/ HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    )
    .await
    .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    assert!(out.starts_with("HTTP/1.1 200"), "{out}");
    assert!(out.contains("/preview/real-site/"), "assets are prefixed");

    // The template pins Vite's HMR socket under the prefix (`vite-hmr` subprotocol).
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let mut req = format!("ws://{addr}/preview/real-site/vite-hmr")
        .into_client_request()
        .unwrap();
    req.headers_mut()
        .insert("sec-websocket-protocol", "vite-hmr".parse().unwrap());
    let (mut ws, res) = tokio_tungstenite::connect_async(req).await.unwrap();
    assert_eq!(res.headers()["sec-websocket-protocol"], "vite-hmr");
    let first = ws.next().await.unwrap().unwrap().into_text().unwrap();
    assert!(first.contains("connected"), "{first}");
    drop(ws);
    state.devservers.shutdown().await;
}

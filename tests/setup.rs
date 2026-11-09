//! `/api/setup/*`: status shape, runtime download/cancel/retry, events,
//! OpenCode readiness while signed out, provider verify, and sign-in.
//!
//! Managed agent downloads are macOS-only (`RuntimeTarget::current()`), so this
//! suite is compiled only on Darwin. Linux CI still exercises the rest of the
//! daemon tests.

#![cfg(target_os = "macos")]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::net::SocketAddr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::State as AxumState;
use axum::http::{header, Request, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use flate2::write::GzEncoder;
use flate2::Compression;
use http_body_util::BodyExt;
use orso::config::Config;
use orso::http;
use orso::runtime::descriptors::{
    write_manifest, AcpPackage, InstallFile, InstallManifest, RuntimeArtifact, RuntimeLock,
};
use orso::runtime::login::LoginManager;
use orso::runtime::{LockFile, RuntimeId, RuntimeTarget};
use orso::state::AppState;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tar::{Builder, Header};
use tower::ServiceExt;

const VERSION: &str = "2.2.5";
const NODE_VERSION: &str = "26.13.0";
const CLAUDE_VERSION: &str = "2.1.318";
const CLAUDE_ADAPTER_VERSION: &str = "0.100.0";
const CLAUDE_ADAPTER_PACKAGE: &str = "@agentclientprotocol/claude-agent-acp";
const TOKEN: &str = "abc123";

fn sha(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn archive(version: &str) -> Vec<u8> {
    let script = format!(
        "#!/bin/sh\n\
         if [ \"$1\" = \"--version\" ]; then echo \"{version}\"; exit 0; fi\n\
         if [ \"$1\" = \"auth\" ] && [ \"$2\" = \"list\" ]; then echo \"[]\"; exit 0; fi\n\
         if [ \"$1\" = \"auth\" ] && [ \"$2\" = \"login\" ]; then\n\
           echo \"Open https://example.test/device\"\n\
           echo \"Enter code: ABCD-1234\"\n\
           sleep 2\n\
           exit 0\n\
         fi\n\
         if [ \"$1\" = \"auth\" ]; then echo \"[{{\\\"provider\\\":\\\"opencode\\\"}}]\"; exit 0; fi\n\
         echo \"{version}\"\n"
    );
    let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
    {
        let mut tar = Builder::new(&mut gz);
        for (name, data, mode) in [
            ("package/bin/opencode", script.into_bytes(), 0o755),
            (
                "package/package.json",
                br#"{"name":"test"}"#.to_vec(),
                0o644,
            ),
        ] {
            let mut header = Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(mode);
            header.set_cksum();
            tar.append_data(&mut header, name, data.as_slice()).unwrap();
        }
        tar.into_inner().unwrap();
    }
    gz.finish().unwrap()
}

fn lock_file(bytes: &[u8]) -> LockFile {
    let artifact = |target: &str| {
        (
            target.to_string(),
            RuntimeArtifact {
                package: Some("@opencode/cli-darwin-arm64".into()),
                asset: Some(format!("opencode-{VERSION}.tar.gz")),
                url: Some(format!("https://example.test/opencode-{VERSION}.tar.gz")),
                asset_sha256: sha(bytes),
                binary_sha256: None,
                download_bytes: Some(bytes.len() as u64),
                installed_bytes: Some(bytes.len() as u64),
                executable: "bin/opencode".into(),
                executables: vec!["bin/opencode".into()],
                files: BTreeMap::new(),
                root: Some("package".into()),
                strip_components: Some(1),
                format: Some("tar.gz".into()),
            },
        )
    };
    LockFile {
        schema_version: 1,
        runtimes: BTreeMap::from([(
            "opencode".into(),
            RuntimeLock {
                version: VERSION.into(),
                sdk_version: None,
                tag: None,
                repository: None,
                registry: Some("https://registry.npmjs.org".into()),
                distribution: None,
                artifacts: BTreeMap::from([artifact("darwin-arm64"), artifact("darwin-x64")]),
            },
        )]),
        acp: BTreeMap::new(),
    }
}

fn write_lock(dir: &Path, bytes: &[u8]) -> PathBuf {
    let path = dir.join("agent-runtime.lock.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&lock_file(bytes)).unwrap()).unwrap();
    path
}

fn config(root: &Path, runtime_dir: &Path, lock: Option<&Path>) -> Config {
    Config {
        projects_dir: root.to_path_buf(),
        no_auth: true,
        token: Some(TOKEN.into()),
        agents_config: None,
        search_path: Some(OsString::new()),
        runtime_dir: runtime_dir.to_path_buf(),
        runtime_base_url: None,
        runtime_lock: lock.map(Path::to_path_buf),
        ..Default::default()
    }
}

fn app(state: AppState) -> Router {
    http::router(state)
}

async fn get_json(app: Router, uri: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .uri(uri)
        .header(header::HOST, "localhost")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn post_json(app: Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::HOST, "localhost")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn install_fake(store: &Path, script: &str) {
    let root = store.join("opencode").join(VERSION);
    let executable = root.join("bin/opencode");
    std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
    std::fs::write(&executable, script).unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    let manifest = InstallManifest {
        schema_version: 1,
        runtime: RuntimeId::OpenCode,
        version: VERSION.into(),
        target: RuntimeTarget::current()
            .map(|target| target.as_str().to_string())
            .unwrap_or_else(|| "darwin-arm64".into()),
        source_url: "https://example.test/opencode".into(),
        installed_at: 0,
        executable: "bin/opencode".into(),
        files: BTreeMap::from([(
            "bin/opencode".into(),
            InstallFile {
                sha256: sha(script.as_bytes()),
                size: script.len() as u64,
                mode: 0o755,
            },
        )]),
    };
    write_manifest(&root, &manifest).unwrap();
}

fn provider<'a>(body: &'a Value, id: &str) -> &'a Value {
    body["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap_or_else(|| panic!("no provider {id} in {body}"))
}

#[tokio::test]
async fn status_reports_runtime_and_opencode_free_models_signed_out() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("runtimes");
    let bytes = archive(VERSION);
    let lock = write_lock(tmp.path(), &bytes);
    install_fake(
        &runtime_dir,
        &format!(
            "#!/bin/sh\n\
             if [ \"$1\" = \"--version\" ]; then echo \"{VERSION}\"; exit 0; fi\n\
             if [ \"$1\" = \"auth\" ]; then echo \"[]\"; exit 0; fi\n\
             echo \"{VERSION}\"\n"
        ),
    );
    let state = AppState::new(config(tmp.path(), &runtime_dir, Some(&lock)));
    let (status, body) = get_json(app(state), "/api/setup/status").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["node"]["phase"], "notDownloaded");
    let opencode = provider(&body, "opencode");
    assert_eq!(opencode["runtime"]["phase"], "ready");
    assert_eq!(opencode["runtime"]["version"], VERSION);
    assert_eq!(opencode["runtime"]["source"], "managed");
    assert_eq!(opencode["installed"], true);
    assert_eq!(opencode["signedIn"], false);
    assert_eq!(opencode["freeModels"], true);
    assert_eq!(opencode["chatReady"], true);
    assert!(opencode["reason"].is_null());

    let claude = provider(&body, "claude");
    assert_eq!(claude["chatReady"], false);
    assert_eq!(claude["freeModels"], false);
    assert!(claude["reason"].as_str().unwrap().contains("Claude"));
}

#[tokio::test]
async fn runtimes_snapshot_has_node_and_providers() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("runtimes");
    let state = AppState::new(config(tmp.path(), &runtime_dir, None));
    let (status, body) = get_json(app(state), "/api/setup/runtimes").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["node"]["phase"].is_string());
    assert_eq!(body["providers"].as_array().unwrap().len(), 4);
    assert!(body.get("git").is_none());
}

#[derive(Clone)]
struct Stub {
    bytes: Arc<Vec<u8>>,
    slow: bool,
}

async fn stub_handler(AxumState(stub): AxumState<Stub>) -> Response {
    let slice = stub.bytes.as_ref().clone();
    let total = slice.len();
    if stub.slow {
        let stream = futures_util::stream::unfold((slice, 0usize), |(bytes, at)| async move {
            if at >= bytes.len() {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
            let end = (at + 1024).min(bytes.len());
            Some((
                Ok::<_, std::convert::Infallible>(axum::body::Bytes::from(bytes[at..end].to_vec())),
                (bytes, end),
            ))
        });
        let mut response = Response::new(Body::from_stream(stream));
        response
            .headers_mut()
            .insert(header::CONTENT_LENGTH, total.to_string().parse().unwrap());
        return response;
    }
    Response::new(Body::from(slice))
}

async fn serve(stub: Stub) -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new().fallback(get(stub_handler)).with_state(stub);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), task)
}

#[tokio::test]
async fn download_returns_ready_status_and_emits_events() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("runtimes");
    let bytes = archive(VERSION);
    let lock = write_lock(tmp.path(), &bytes);
    let (url, server) = serve(Stub {
        bytes: Arc::new(bytes),
        slow: false,
    })
    .await;

    let mut config = config(tmp.path(), &runtime_dir, Some(&lock));
    config.runtime_base_url = Some(url);
    let state = AppState::new(config);
    let mut events = state.events.subscribe();
    let (status, body) = post_json(
        app(state),
        "/api/setup/runtimes/opencode/download",
        json!({}),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["phase"], "ready");
    assert_eq!(body["version"], VERSION);
    assert_eq!(body["source"], "managed");
    assert!(runtime_dir
        .join("opencode")
        .join(VERSION)
        .join("bin/opencode")
        .is_file());

    let mut phases = Vec::new();
    while let Ok(Ok(text)) = tokio::time::timeout(Duration::from_millis(500), events.recv()).await {
        let event: Value = serde_json::from_str(&text).unwrap();
        if event["type"] == "setup.runtime" && event["runtime"] == "opencode" {
            phases.push(event["phase"].as_str().unwrap().to_string());
        }
        if event["phase"] == "ready" {
            break;
        }
    }
    assert!(phases.contains(&"downloading".to_string()), "{phases:?}");
    assert_eq!(phases.last().map(String::as_str), Some("ready"));
    server.abort();
}

#[tokio::test]
async fn cancel_and_retry_return_runtime_status() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("runtimes");
    // Slow enough that cancel lands mid-transfer.
    let mut bytes = archive(VERSION);
    bytes.resize(100_000, 0);
    let lock = write_lock(tmp.path(), &bytes);
    let (url, server) = serve(Stub {
        bytes: Arc::new(bytes),
        slow: true,
    })
    .await;

    let mut config = config(tmp.path(), &runtime_dir, Some(&lock));
    config.runtime_base_url = Some(url);
    let state = AppState::new(config);
    let router = app(state);

    let download = tokio::spawn(post_json(
        router.clone(),
        "/api/setup/runtimes/opencode/download",
        json!({}),
    ));
    tokio::time::sleep(Duration::from_millis(100)).await;
    let (cancel_status, cancel) = post_json(
        router.clone(),
        "/api/setup/runtimes/opencode/cancel",
        json!({}),
    )
    .await;
    assert_eq!(cancel_status, StatusCode::OK);
    assert!(cancel["phase"].is_string());

    let (download_status, download) = download.await.unwrap();
    assert_eq!(download_status, StatusCode::OK);
    assert_eq!(download["phase"], "notDownloaded");

    let (retry_status, retry) =
        post_json(router, "/api/setup/runtimes/opencode/retry", json!({})).await;
    assert_eq!(retry_status, StatusCode::OK);
    assert_eq!(retry["phase"], "ready");
    server.abort();
}

#[tokio::test]
async fn verify_runs_the_resolved_acp_command() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("runtimes");
    let state = AppState::new(config(tmp.path(), &runtime_dir, None));
    let (command, args) = orso::chat::acp::fake_agent();
    let mut argv = vec![command];
    argv.extend(args);
    orso::providers::set_test_acp_argv("fake-verify", argv);

    let (status, body) = post_json(
        app(state),
        "/api/setup/providers/fake-verify/verify",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["chatReady"], true, "{body}");
}

#[tokio::test]
async fn sign_in_reads_the_url_and_cancels_the_process() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime_dir = tmp.path().join("runtimes");
    let lock = write_lock(tmp.path(), &archive(VERSION));
    install_fake(&runtime_dir, &archive_script(VERSION));
    let state = AppState::new(config(tmp.path(), &runtime_dir, Some(&lock)));
    let resolver = (*state.resolver).clone();
    let logins = Arc::new(LoginManager::new(resolver).with_opener(|_url: &str| {
        // no browser in tests
    }));
    let state = AppState { logins, ..state };
    let router = app(state);

    let (status, body) = post_json(
        router.clone(),
        "/api/setup/providers/opencode/sign-in",
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["state"], "browser");
    assert_eq!(body["url"], "https://example.test/device");
    assert!(body["message"].as_str().unwrap().contains("ABCD-1234"));

    let (status, body) = post_json(router, "/api/setup/providers/opencode/cancel", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["state"], "cancelled");
}

fn archive_script(version: &str) -> String {
    format!(
        "#!/bin/sh\n\
         if [ \"$1\" = \"--version\" ]; then echo \"{version}\"; exit 0; fi\n\
         if [ \"$1\" = \"auth\" ] && [ \"$2\" = \"list\" ]; then echo \"[]\"; exit 0; fi\n\
         if [ \"$1\" = \"auth\" ] && [ \"$2\" = \"login\" ]; then\n\
           echo \"Open https://example.test/device\"\n\
           echo \"Enter code: ABCD-1234\"\n\
           sleep 2\n\
           exit 0\n\
         fi\n\
         if [ \"$1\" = \"auth\" ]; then echo \"[{{\\\"provider\\\":\\\"opencode\\\"}}]\"; exit 0; fi\n\
         echo \"{version}\"\n"
    )
}

// ---------------------------------------------------------------------------
// Claude/Codex ACP adapters
// ---------------------------------------------------------------------------

fn artifact_for(target: &str, executable: &str, version: &str) -> (String, RuntimeArtifact) {
    (
        target.to_string(),
        RuntimeArtifact {
            package: None,
            asset: Some(format!("{executable}-{version}.tar.gz")),
            url: Some(format!(
                "https://example.test/{executable}-{version}.tar.gz"
            )),
            asset_sha256: "00".into(),
            binary_sha256: None,
            download_bytes: Some(1),
            installed_bytes: Some(1),
            executable: executable.into(),
            executables: vec![executable.into()],
            files: BTreeMap::new(),
            root: None,
            strip_components: Some(0),
            format: Some("tar.gz".into()),
        },
    )
}

fn lock_with_claude_adapter(bytes: &[u8], adapter_version: &str) -> LockFile {
    let mut lock = lock_file(bytes);
    lock.runtimes.insert(
        "node".into(),
        RuntimeLock {
            version: NODE_VERSION.into(),
            sdk_version: None,
            tag: None,
            repository: None,
            registry: None,
            distribution: None,
            artifacts: BTreeMap::from([
                artifact_for("darwin-arm64", "bin/node", NODE_VERSION),
                artifact_for("darwin-x64", "bin/node", NODE_VERSION),
            ]),
        },
    );
    lock.runtimes.insert(
        "claude".into(),
        RuntimeLock {
            version: CLAUDE_VERSION.into(),
            sdk_version: None,
            tag: None,
            repository: None,
            registry: None,
            distribution: None,
            artifacts: BTreeMap::from([
                artifact_for("darwin-arm64", "bin/claude", CLAUDE_VERSION),
                artifact_for("darwin-x64", "bin/claude", CLAUDE_VERSION),
            ]),
        },
    );
    lock.acp.insert(
        "claude".into(),
        AcpPackage {
            package: CLAUDE_ADAPTER_PACKAGE.into(),
            version: adapter_version.into(),
        },
    );
    lock
}

fn write_lock_file(dir: &Path, lock: &LockFile) -> PathBuf {
    let path = dir.join("agent-runtime.lock.json");
    std::fs::write(&path, serde_json::to_vec_pretty(lock).unwrap()).unwrap();
    path
}

fn install_runtime(store: &Path, id: RuntimeId, version: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = store.join(id.as_str()).join(version);
    let mut recorded = BTreeMap::new();
    for (relative, body) in files {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, body).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        recorded.insert(
            (*relative).to_string(),
            InstallFile {
                sha256: sha(body.as_bytes()),
                size: body.len() as u64,
                mode: 0o755,
            },
        );
    }
    let manifest = InstallManifest {
        schema_version: 1,
        runtime: id,
        version: version.into(),
        target: RuntimeTarget::current()
            .map(|target| target.as_str().to_string())
            .unwrap_or_else(|| "darwin-arm64".into()),
        source_url: "https://example.test".into(),
        installed_at: 0,
        executable: files[0].0.into(),
        files: recorded,
    };
    write_manifest(&root, &manifest).unwrap();
    root
}

/// Install a fake `npm` that writes a plausible adapter package tree. `fail`
/// simulates a network/npm failure.
fn write_fake_npm(store: &Path, version: &str, adapter_bin: &str, fail: bool) {
    let bin = store.join("node").join(NODE_VERSION).join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let npm = bin.join("npm");
    let fail_line = if fail {
        "echo \"npm ERR! simulated network failure\" >&2\nexit 1"
    } else {
        ""
    };
    let script = format!(
        r#"#!/bin/sh
{fail_line}
prefix=""
pkg=""
while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) prefix="$2"; shift 2 ;;
    *) pkg="$1"; shift ;;
  esac
done
base=${{pkg%@*}}
mkdir -p "$prefix/node_modules/$base/dist"
cat > "$prefix/node_modules/$base/package.json" <<EOF
{{"name":"$base","version":"{version}","bin":{{"claude-agent-acp":"dist/index.js"}}}}
EOF
cat > "$prefix/node_modules/$base/dist/index.js" <<'ADAPTER'
{adapter_bin}
ADAPTER
chmod 755 "$prefix/node_modules/$base/dist/index.js"
"#
    );
    std::fs::write(&npm, script).unwrap();
    std::fs::set_permissions(&npm, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn claude_script() -> String {
    format!(
        "#!/bin/sh\n\
         if [ \"$1\" = \"--version\" ]; then echo \"{CLAUDE_VERSION}\"; exit 0; fi\n\
         if [ \"$1\" = \"auth\" ] && [ \"$2\" = \"status\" ]; then echo '{{\"loggedIn\":true}}'; exit 0; fi\n\
         exit 1\n"
    )
}

fn adapter_store(tmp: &Path) -> PathBuf {
    let store = tmp.join("runtimes");
    install_runtime(
        &store,
        RuntimeId::Node,
        NODE_VERSION,
        &[
            ("bin/node", "#!/bin/sh\nexec \"$@\"\n"),
            ("bin/npm", "#!/bin/sh\nexit 1\n"),
        ],
    );
    install_runtime(
        &store,
        RuntimeId::Claude,
        CLAUDE_VERSION,
        &[("bin/claude", &claude_script())],
    );
    store
}

async fn wait_for_claude(app: Router, predicate: impl Fn(&Value) -> bool) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        let (status, body) = get_json(app.clone(), "/api/setup/status").await;
        assert_eq!(status, StatusCode::OK);
        let claude = provider(&body, "claude").clone();
        if predicate(&claude) {
            return claude;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for Claude status: {claude}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn claude_chat_ready_after_the_adapter_is_installed() {
    let tmp = tempfile::tempdir().unwrap();
    let store = adapter_store(tmp.path());
    write_fake_npm(&store, CLAUDE_ADAPTER_VERSION, "#!/bin/sh\nexit 1\n", false);
    let lock = lock_with_claude_adapter(&archive(VERSION), CLAUDE_ADAPTER_VERSION);
    let lock_path = write_lock_file(tmp.path(), &lock);
    let state = AppState::new(config(tmp.path(), &store, Some(&lock_path)));

    let claude = wait_for_claude(app(state), |claude| {
        claude["chatReady"].as_bool() == Some(true)
    })
    .await;
    assert_eq!(claude["runtime"]["phase"], "ready");
    assert_eq!(claude["runtime"]["source"], "managed");
    assert_eq!(claude["signedIn"], true);
    assert!(claude["reason"].is_null(), "{claude}");
    assert!(store.join("acp/claude/orso-adapter.json").is_file());

    let manifest: Value =
        serde_json::from_slice(&std::fs::read(store.join("acp/claude/orso-adapter.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["version"], CLAUDE_ADAPTER_VERSION);
    assert!(manifest["bin"].as_str().unwrap().ends_with("dist/index.js"));
}

#[tokio::test]
async fn failed_adapter_install_keeps_claude_unready_with_a_plain_reason() {
    let tmp = tempfile::tempdir().unwrap();
    let store = adapter_store(tmp.path());
    write_fake_npm(&store, CLAUDE_ADAPTER_VERSION, "#!/bin/sh\nexit 1\n", true);
    let lock = lock_with_claude_adapter(&archive(VERSION), CLAUDE_ADAPTER_VERSION);
    let lock_path = write_lock_file(tmp.path(), &lock);
    let state = AppState::new(config(tmp.path(), &store, Some(&lock_path)));

    let claude = wait_for_claude(app(state), |claude| {
        claude["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("Couldn't finish setting up chat with Claude"))
    })
    .await;
    assert_eq!(claude["chatReady"], false);
    let reason = claude["reason"].as_str().unwrap();
    assert!(!reason.contains("npm"), "{reason}");
    assert!(!reason.contains("ERR!"), "{reason}");
}

#[tokio::test]
async fn verify_installs_the_adapter_before_the_handshake() {
    let tmp = tempfile::tempdir().unwrap();
    let store = adapter_store(tmp.path());
    let (command, args) = orso::chat::acp::fake_agent();
    let adapter_bin = format!(
        "#!/bin/sh\nexec {} {}\n",
        command,
        args.iter()
            .map(|arg| format!("'{}'", arg))
            .collect::<Vec<_>>()
            .join(" ")
    );
    write_fake_npm(&store, CLAUDE_ADAPTER_VERSION, &adapter_bin, false);
    let lock = lock_with_claude_adapter(&archive(VERSION), CLAUDE_ADAPTER_VERSION);
    let lock_path = write_lock_file(tmp.path(), &lock);
    let state = AppState::new(config(tmp.path(), &store, Some(&lock_path)));

    let (status, body) =
        post_json(app(state), "/api/setup/providers/claude/verify", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["chatReady"], true, "{body}");
    assert!(store.join("acp/claude/orso-adapter.json").is_file());
}

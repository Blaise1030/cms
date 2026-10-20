use std::path::Path;
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use orso::config::Config;
use orso::http;
use orso::state::AppState;
use serde_json::{json, Value};
use tower::ServiceExt;

fn config(dir: &Path, no_auth: bool) -> Config {
    Config {
        port: 0,
        origin: None,
        projects_dir: dir.to_path_buf(),
        no_auth,
        token: if no_auth { None } else { Some("abc123".into()) },
        agents_config: None,
        ..Default::default()
    }
}

/// Router with `npm install` skipped (test hook: `AppState::with_scaffold_install(_, false)`).
fn app(dir: &Path) -> axum::Router {
    http::router(AppState::with_scaffold_install(config(dir, true), false))
}

fn req(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
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

async fn create(app: &axum::Router, name: &str) -> Value {
    let (status, body) = call(
        app,
        req("POST", "/api/projects", Some(json!({ "name": name }))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

fn git(dir: &Path, args: &[&str]) -> String {
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

#[tokio::test]
async fn create_copies_template_inits_git_and_returns_id_and_port() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path());

    let body = create(&app, "My Cool Site").await;
    assert_eq!(body["id"], "my-cool-site");
    assert_eq!(body["name"], "My Cool Site");
    assert_eq!(body["devserver"], "starting");
    let port = body["port"].as_u64().unwrap();
    assert!(port > 1024 && port < 65536);

    let dir = tmp.path().join("my-cool-site");
    for f in [
        "package.json",
        "package-lock.json",
        "AGENTS.md",
        "DESIGN.md",
        ".gitignore",
        "astro.config.mjs",
        "src/content.config.ts",
        "src/site.ts",
        "src/content/blog/en/welcome.md",
        "src/pages/index.astro",
        "src/components/sections/Hero.astro",
    ] {
        assert!(dir.join(f).is_file(), "missing {f}");
    }
    assert!(
        !dir.join("node_modules").exists(),
        "install skipped by hook"
    );

    // git: exactly one commit, clean tree, on main.
    assert_eq!(git(&dir, &["rev-list", "--count", "HEAD"]), "1");
    assert_eq!(git(&dir, &["status", "--porcelain"]), "");
    assert_eq!(git(&dir, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    assert!(git(&dir, &["ls-files"]).contains("src/content/blog/en/welcome.md"));

    // appears in the list with the same shape
    let (status, list) = call(&app, req("GET", "/api/projects", None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["projects"], json!([body]));
}

#[tokio::test]
async fn duplicate_names_get_distinct_ids_dirs_and_ports() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path());
    let a = create(&app, "Site").await;
    let b = create(&app, "site").await;
    assert_eq!(a["id"], "site");
    assert_eq!(b["id"], "site-2");
    assert_ne!(a["port"], b["port"]);
    assert!(tmp.path().join("site-2/AGENTS.md").is_file());
}

#[tokio::test]
async fn create_does_not_reuse_an_existing_unregistered_directory() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir(tmp.path().join("taken")).unwrap();
    std::fs::write(tmp.path().join("taken/keep.txt"), "x").unwrap();
    let app = app(tmp.path());
    let body = create(&app, "taken").await;
    assert_eq!(body["id"], "taken-2");
    assert!(tmp.path().join("taken/keep.txt").is_file());
}

#[tokio::test]
async fn concurrent_creates_do_not_collide() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path());
    let (a, b, c) = tokio::join!(
        create(&app, "Same"),
        create(&app, "Same"),
        create(&app, "Same")
    );
    let mut ids: Vec<_> = [&a, &b, &c]
        .iter()
        .map(|v| v["id"].as_str().unwrap().to_string())
        .collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 3);
    let mut ports: Vec<_> = [&a, &b, &c]
        .iter()
        .map(|v| v["port"].as_u64().unwrap())
        .collect();
    ports.sort();
    ports.dedup();
    assert_eq!(ports.len(), 3);
}

#[tokio::test]
async fn invalid_create_requests_use_error_envelope() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path());
    for body in [
        Some(json!({ "name": "" })),
        Some(json!({ "name": "   " })),
        Some(json!({ "name": "!!!" })),
        Some(json!({ "name": "a".repeat(65) })),
        Some(json!({ "name": "line\nbreak" })),
        Some(json!({ "nope": 1 })),
        Some(json!({ "name": 5 })),
        None,
    ] {
        let (status, v) = call(&app, req("POST", "/api/projects", body.clone())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
        assert_eq!(v["error"]["code"], "BADREQ", "{body:?}");
    }
    let (_, list) = call(&app, req("GET", "/api/projects", None)).await;
    assert_eq!(list["projects"], json!([]));
    // nothing (besides possibly the state dir) was scaffolded
    let dirs: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n != ".orso")
        .collect();
    assert!(dirs.is_empty(), "{dirs:?}");
}

#[tokio::test]
async fn delete_requires_confirm_then_removes_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path());
    let id = create(&app, "Doomed").await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let dir = tmp.path().join(&id);
    // Daemon state outside the repo is removed too
    let state = tmp.path().join(".orso").join(&id);
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("daemon-state.txt"), "").unwrap();

    for uri in [
        format!("/api/projects/{id}"),
        format!("/api/projects/{id}?confirm=false"),
        format!("/api/projects/{id}?confirm=1"),
    ] {
        let (status, v) = call(&app, req("DELETE", &uri, None)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(v["error"]["code"], "BADREQ");
        assert!(dir.is_dir(), "dir must survive unconfirmed delete");
    }
    let (_, list) = call(&app, req("GET", "/api/projects", None)).await;
    assert_eq!(list["projects"].as_array().unwrap().len(), 1);

    let (status, v) = call(
        &app,
        req("DELETE", &format!("/api/projects/{id}?confirm=true"), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["deleted"], id);
    assert!(!dir.exists());
    assert!(!state.exists());
    let (_, list) = call(&app, req("GET", "/api/projects", None)).await;
    assert_eq!(list["projects"], json!([]));

    // deleting again is ENOENT
    let (status, v) = call(
        &app,
        req("DELETE", &format!("/api/projects/{id}?confirm=true"), None),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(v["error"]["code"], "ENOENT");
}

#[tokio::test]
async fn delete_only_touches_registered_projects() {
    let tmp = tempfile::tempdir().unwrap();
    let outside = tmp.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let projects = tmp.path().join("projects");
    let app = app(&projects);
    create(&app, "real").await;
    for id in ["outside", "..", "%2e%2e", "real/../../outside", ".orso"] {
        let (status, _) = call(
            &app,
            req("DELETE", &format!("/api/projects/{id}?confirm=true"), None),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{id}");
    }
    assert!(outside.is_dir());
    assert!(projects.join("real").is_dir());
    assert!(projects.join(".orso").is_dir());
}

#[tokio::test]
async fn registry_survives_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let first = app(tmp.path());
    let created = create(&first, "Persist Me").await;
    drop(first);

    let second = app(tmp.path());
    let (_, list) = call(&second, req("GET", "/api/projects", None)).await;
    assert_eq!(list["projects"], json!([created]));

    // a project whose dir vanished is dropped on load
    std::fs::remove_dir_all(tmp.path().join("persist-me")).unwrap();
    let third = app(tmp.path());
    let (_, list) = call(&third, req("GET", "/api/projects", None)).await;
    assert_eq!(list["projects"], json!([]));
}

fn quarantined(state_dir: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(state_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("projects.json.corrupt-")
        })
        .collect()
}

#[cfg(unix)]
fn chmod(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[tokio::test]
async fn corrupt_registry_is_quarantined_under_a_unique_name() {
    let tmp = tempfile::tempdir().unwrap();
    let state = tmp.path().join(".orso");
    std::fs::create_dir_all(&state).unwrap();
    // A pre-existing backup must never be clobbered by a second quarantine.
    std::fs::write(state.join("projects.json.corrupt"), "old").unwrap();
    std::fs::write(state.join("projects.json"), "{not json").unwrap();
    let app = app(tmp.path());
    let (status, list) = call(&app, req("GET", "/api/projects", None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["projects"], json!([]));
    let q = quarantined(&state);
    assert_eq!(q.len(), 1);
    assert_eq!(std::fs::read_to_string(&q[0]).unwrap(), "{not json");
    assert_eq!(
        std::fs::read_to_string(state.join("projects.json.corrupt")).unwrap(),
        "old"
    );
    // registry is usable afterwards
    create(&app, "fresh").await;
}

#[cfg(unix)]
#[tokio::test]
async fn failed_quarantine_blocks_writes_and_keeps_corrupt_file() {
    let tmp = tempfile::tempdir().unwrap();
    let state = tmp.path().join(".orso");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("projects.json"), "{not json").unwrap();
    chmod(&state, 0o500); // readable, but no rename/create inside

    let app = app(tmp.path());
    let (status, v) = call(
        &app,
        req("POST", "/api/projects", Some(json!({ "name": "x" }))),
    )
    .await;
    let serve_err = orso::serve(config(tmp.path(), true)).await.unwrap_err();
    chmod(&state, 0o700);

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{v}");
    assert_eq!(v["error"]["code"], "IO");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("refusing"),
        "{v}"
    );
    assert_eq!(
        std::fs::read_to_string(state.join("projects.json")).unwrap(),
        "{not json"
    );
    assert!(!tmp.path().join("x").exists(), "nothing scaffolded");
    // the daemon refuses to start rather than run against an unreadable registry
    assert!(
        serve_err.to_string().contains("could not be moved aside"),
        "{serve_err}"
    );
}

#[tokio::test]
async fn delete_that_fails_to_commit_leaves_project_intact_and_is_retryable() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path());
    let id = create(&app, "Sticky").await["id"]
        .as_str()
        .unwrap()
        .to_string();
    let dir = tmp.path().join(&id);
    let state = tmp.path().join(".orso");
    // Make the registry write fail: its temp file path is occupied by a directory.
    std::fs::create_dir(state.join("projects.json.tmp")).unwrap();

    let uri = format!("/api/projects/{id}?confirm=true");
    let (status, v) = call(&app, req("DELETE", &uri, None)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{v}");
    assert_eq!(v["error"]["code"], "IO");
    // error means nothing was deleted: files, git repo, registry entry all intact
    assert!(dir.join("AGENTS.md").is_file());
    assert_eq!(git(&dir, &["status", "--porcelain"]), "");
    let (_, list) = call(&app, req("GET", "/api/projects", None)).await;
    assert_eq!(list["projects"].as_array().unwrap().len(), 1);
    // and nothing is stranded in the trash
    let trash = state.join("trash");
    assert!(!trash.exists() || std::fs::read_dir(&trash).unwrap().next().is_none());

    // once the fault is gone, the same request succeeds
    std::fs::remove_dir(state.join("projects.json.tmp")).unwrap();
    let (status, _) = call(&app, req("DELETE", &uri, None)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!dir.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn cleanup_failure_after_commit_is_non_fatal() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path());
    let id = create(&app, "Gone").await["id"]
        .as_str()
        .unwrap()
        .to_string();
    // A state dir we cannot fully remove (file inside a read-only subdir).
    let locked = tmp.path().join(".orso").join(&id).join("locked");
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::write(locked.join("f"), "x").unwrap();
    chmod(&locked, 0o500);

    let uri = format!("/api/projects/{id}?confirm=true");
    let (status, v) = call(&app, req("DELETE", &uri, None)).await;
    let leftover = locked.exists();
    chmod(&locked, 0o700);

    assert!(leftover, "test setup: cleanup should have failed");
    assert_eq!(status, StatusCode::OK, "{v}");
    assert!(!tmp.path().join(&id).exists());
    let (_, list) = call(&app, req("GET", "/api/projects", None)).await;
    assert_eq!(list["projects"], json!([]));
    // and the delete is durable across a restart
    let reopened = self::app(tmp.path());
    let (_, list) = call(&reopened, req("GET", "/api/projects", None)).await;
    assert_eq!(list["projects"], json!([]));
}

#[tokio::test]
async fn delete_of_project_whose_dir_vanished_drops_the_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path());
    let id = create(&app, "Ghost").await["id"]
        .as_str()
        .unwrap()
        .to_string();
    std::fs::remove_dir_all(tmp.path().join(&id)).unwrap();
    let (status, _) = call(
        &app,
        req("DELETE", &format!("/api/projects/{id}?confirm=true"), None),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, list) = call(&app, req("GET", "/api/projects", None)).await;
    assert_eq!(list["projects"], json!([]));
}

#[tokio::test]
async fn trash_leftovers_are_purged_on_start() {
    let tmp = tempfile::tempdir().unwrap();
    let leftover = tmp.path().join(".orso/trash/old-1");
    std::fs::create_dir_all(&leftover).unwrap();
    std::fs::write(leftover.join("f"), "x").unwrap();
    let _ = app(tmp.path());
    assert!(!tmp.path().join(".orso/trash").exists());
}

#[tokio::test]
async fn project_endpoints_require_auth() {
    let tmp = tempfile::tempdir().unwrap();
    let app = http::router(AppState::with_scaffold_install(
        config(tmp.path(), false),
        false,
    ));
    for (m, u) in [
        ("GET", "/api/projects"),
        ("POST", "/api/projects"),
        ("DELETE", "/api/projects/x?confirm=true"),
    ] {
        let (status, v) = call(&app, req(m, u, None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{m} {u}");
        assert_eq!(v["error"]["code"], "UNAUTH");
    }
}

/// Full path with a real `npm install` and a real `astro build` of the scaffolded content
/// model. Needs network; run with `cargo test -- --ignored`.
#[tokio::test]
#[ignore = "needs network + npm"]
async fn real_npm_install_and_site_builds() {
    let tmp = tempfile::tempdir().unwrap();
    // Real install, but no dev server: this test is about scaffolding and `astro build`.
    let app = http::router(AppState::with_options(
        config(tmp.path(), true),
        true,
        orso::devserver::DevServerOptions::disabled(),
    ));
    let body = create(&app, "Real Install").await;
    let dir = tmp.path().join(body["id"].as_str().unwrap());
    assert!(dir.join("node_modules/astro").is_dir());
    assert_eq!(
        git(&dir, &["status", "--porcelain"]),
        "",
        "node_modules must be ignored"
    );

    let out = Command::new("npm")
        .args(["run", "build"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dir.join("dist/index.html").is_file());
}

#[tokio::test]
async fn new_projects_carry_a_local_identity_and_it_can_be_replaced() {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::with_scaffold_install(config(tmp.path(), true), false);
    let info = state.projects.create("Site").await.unwrap();
    let dir = tmp.path().join(&info.id);
    assert_eq!(git(&dir, &["config", "--local", "user.name"]), "orso");
    assert_eq!(git(&dir, &["log", "-1", "--format=%an"]), "orso");

    let ada = orso::git::Identity {
        name: "Ada Lovelace".into(),
        email: "42+ada@users.noreply.github.com".into(),
    };
    state.projects.apply_identity(&ada).await;
    assert_eq!(
        git(&dir, &["config", "--local", "user.name"]),
        "Ada Lovelace"
    );
    assert_eq!(
        git(&dir, &["config", "--local", "user.email"]),
        "42+ada@users.noreply.github.com"
    );
}

#[tokio::test]
async fn startup_gives_older_projects_a_local_identity() {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::with_scaffold_install(config(tmp.path(), true), false);
    let info = state.projects.create("Site").await.unwrap();
    let dir = tmp.path().join(&info.id);
    git(&dir, &["config", "--local", "--unset", "user.name"]);
    git(&dir, &["config", "--local", "--unset", "user.email"]);

    state
        .projects
        .ensure_identities(&orso::git::Identity::orso())
        .await;
    assert_eq!(git(&dir, &["config", "--local", "user.name"]), "orso");
}

#[tokio::test]
async fn startup_repairs_a_half_written_identity() {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::with_scaffold_install(config(tmp.path(), true), false);
    let info = state.projects.create("Site").await.unwrap();
    let dir = tmp.path().join(&info.id);
    git(&dir, &["config", "--local", "--unset", "user.email"]);

    state
        .projects
        .ensure_identities(&orso::git::Identity::orso())
        .await;
    assert_eq!(
        git(&dir, &["config", "--local", "user.email"]),
        "orso@localhost"
    );
}

#[tokio::test]
async fn create_from_clone_copies_the_repo_records_it_and_sets_the_identity() {
    use orso::git::Identity;
    use orso::projects::Source;

    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    git(&src, &["init", "-b", "main"]);
    std::fs::write(src.join("package.json"), "{}").unwrap();
    git(&src, &["add", "-A"]);
    git(
        &src,
        &[
            "-c",
            "user.name=Grace",
            "-c",
            "user.email=g@example.com",
            "commit",
            "-m",
            "init",
        ],
    );
    let bare = tmp.path().join("git/acme/site.git");
    std::fs::create_dir_all(bare.parent().unwrap()).unwrap();
    git(
        tmp.path(),
        &[
            "clone",
            "--bare",
            src.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );

    let projects_dir = tmp.path().join("projects");
    let state = AppState::with_scaffold_install(config(&projects_dir, true), false);
    let info = state
        .projects
        .create_from(
            "site",
            Source::Clone {
                repo: "acme/site".into(),
                url: format!("file://{}", bare.display()),
                env: vec![],
            },
            &Identity {
                name: "Ada".into(),
                email: "ada@example.com".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(info.repo.as_deref(), Some("acme/site"));

    let dir = projects_dir.join(&info.id);
    assert!(dir.join("package.json").exists());
    assert_eq!(git(&dir, &["config", "--local", "user.name"]), "Ada");
    assert_eq!(git(&dir, &["log", "-1", "--format=%an"]), "Grace");
    assert!(git(&dir, &["remote", "get-url", "origin"]).ends_with("acme/site.git"));

    // Persisted across a registry reopen.
    let reopened = AppState::with_scaffold_install(config(&projects_dir, true), false);
    let p = reopened.projects.get(&info.id).await.unwrap();
    assert_eq!(p.repo.as_deref(), Some("acme/site"));

    // Template projects serialize `repo: null`.
    state.projects.create("Other").await.unwrap();
    let (status, body) = call(&app(&projects_dir), req("GET", "/api/projects", None)).await;
    assert_eq!(status, StatusCode::OK);
    let other = body["projects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Other")
        .unwrap();
    assert!(other.get("repo").is_some_and(Value::is_null), "{other}");
}

#[tokio::test]
async fn clone_with_invalid_settings_still_opens() {
    use orso::git::Identity;
    use orso::projects::Source;

    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    git(&src, &["init", "-b", "main"]);
    std::fs::write(src.join("package.json"), "{}").unwrap();
    std::fs::write(src.join("orso.json"), r#"{"dev":"pnpm dev"}"#).unwrap();
    git(&src, &["add", "-A"]);
    git(
        &src,
        &[
            "-c",
            "user.name=Grace",
            "-c",
            "user.email=g@example.com",
            "commit",
            "-m",
            "init",
        ],
    );

    let projects_dir = tmp.path().join("projects");
    let state = AppState::with_options(
        config(&projects_dir, true),
        true,
        orso::devserver::DevServerOptions::disabled(),
    );
    let info = state
        .projects
        .create_from(
            "site",
            Source::Clone {
                repo: "acme/site".into(),
                url: format!("file://{}", src.display()),
                env: vec![],
            },
            &Identity::orso(),
        )
        .await
        .unwrap();
    let dir = projects_dir.join(&info.id);
    assert!(dir.join("orso.json").exists());
    assert!(!dir.join("node_modules").exists(), "install was skipped");
}

#[tokio::test]
async fn failed_clone_leaves_nothing_behind() {
    use orso::git::Identity;
    use orso::projects::Source;

    let tmp = tempfile::tempdir().unwrap();
    let projects_dir = tmp.path().join("projects");
    let state = AppState::with_scaffold_install(config(&projects_dir, true), false);
    let err = state
        .projects
        .create_from(
            "site",
            Source::Clone {
                repo: "acme/missing".into(),
                url: format!("file://{}/nope.git", tmp.path().display()),
                env: vec![],
            },
            &Identity::orso(),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code.as_str(), "GIT_ERR");
    assert!(!projects_dir.join("site").exists());
    assert!(state.projects.list().await.is_empty());
    let info = state.projects.create("site").await.unwrap();
    assert_eq!(info.id, "site");
}

#[tokio::test]
async fn protection_hint_dismissal_is_persisted_per_project() {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::with_scaffold_install(config(tmp.path(), true), false);
    let first = http::router(state.clone());
    let a = create(&first, "Site A").await;
    let b = create(&first, "Site B").await;
    let (a, b) = (a["id"].as_str().unwrap(), b["id"].as_str().unwrap());
    assert!(!state.projects.protection_hint_dismissed(a).await);

    let (status, body) = call(
        &first,
        req(
            "POST",
            &format!("/api/projects/{a}/protection-hint/dismiss"),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ "dismissed": true }));
    assert!(state.projects.protection_hint_dismissed(a).await);
    assert!(!state.projects.protection_hint_dismissed(b).await);
    drop(first);
    drop(state);

    let restarted = AppState::with_scaffold_install(config(tmp.path(), true), false);
    assert!(restarted.projects.protection_hint_dismissed(a).await);
    assert!(!restarted.projects.protection_hint_dismissed(b).await);

    let (status, _) = call(
        &http::router(restarted),
        req("POST", "/api/projects/nope/protection-hint/dismiss", None),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn branch_protection_doc_ships_with_the_daemon() {
    let tmp = tempfile::tempdir().unwrap();
    let res = app(tmp.path())
        .oneshot(req("GET", "/api/docs/branch-protection", None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        text.starts_with("# Protect `main` so merges need review"),
        "{text}"
    );
}

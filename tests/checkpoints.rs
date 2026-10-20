use std::path::Path;
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use orso::config::Config;
use orso::http::checkpoints::{finish_turn, TurnOutcome};
use orso::projects::Source;
use orso::state::AppState;
use orso::{git, http};
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

fn state(dir: &Path) -> AppState {
    AppState::with_scaffold_install(config(dir, true), false)
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

async fn raw(app: &axum::Router, r: Request<Body>) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let res = app.clone().oneshot(r).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, headers, bytes)
}

async fn call(app: &axum::Router, r: Request<Body>) -> (StatusCode, Value) {
    let (s, _, b) = raw(app, r).await;
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

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

/// A scaffolded project (git-initialised) plus router over the same state.
async fn project(tmp: &Path) -> (axum::Router, std::path::PathBuf) {
    let st = state(tmp);
    let info = st.projects.create("Site").await.unwrap();
    let dir = tmp.join(&info.id);
    (http::router(st), dir)
}

const WELCOME: &str = "src/content/blog/en/welcome.md";

#[tokio::test]
async fn dirty_tree_creates_checkpoint_and_clean_tree_skips() {
    let tmp = tempfile::tempdir().unwrap();
    let (_app, dir) = project(tmp.path()).await;
    let base = git_cli(&dir, &["rev-parse", "HEAD"]);

    // clean → no commit
    assert_eq!(git::checkpoint(&dir, "noop").await.unwrap(), None);
    assert_eq!(git_cli(&dir, &["rev-list", "--count", "HEAD"]), "1");

    // dirty (modified + new + deleted) → one commit
    std::fs::write(dir.join(WELCOME), "changed\n").unwrap();
    std::fs::write(dir.join("src/new.txt"), "new\n").unwrap();
    std::fs::remove_file(dir.join("AGENTS.md")).unwrap();
    let cp = git::checkpoint(&dir, "  Update hero\nmore detail")
        .await
        .unwrap()
        .expect("checkpoint");
    assert_eq!(cp.summary, "Update hero");
    assert_eq!(cp.author, "orso");
    assert_eq!(cp.sha, git_cli(&dir, &["rev-parse", "HEAD"]));
    assert_ne!(cp.sha, base);
    assert_eq!(git_cli(&dir, &["rev-list", "--count", "HEAD"]), "2");
    assert_eq!(git_cli(&dir, &["status", "--porcelain"]), "");
    assert_eq!(
        git_cli(&dir, &["log", "-1", "--format=%s"]),
        "checkpoint: Update hero"
    );

    orso::git::set_identity(
        &dir,
        &orso::git::Identity {
            name: "Ada".into(),
            email: "ada@example.com".into(),
        },
    )
    .await
    .unwrap();
    std::fs::write(dir.join("src/ada.txt"), "hi\n").unwrap();
    let by_ada = git::checkpoint(&dir, "Ada's edit").await.unwrap().unwrap();
    assert_eq!(by_ada.author, "Ada");

    // clean again → skipped
    assert_eq!(git::checkpoint(&dir, "again").await.unwrap(), None);
    assert_eq!(git_cli(&dir, &["rev-list", "--count", "HEAD"]), "3");
}

#[tokio::test]
async fn no_checkpoint_while_a_merge_is_unresolved() {
    let tmp = tempfile::tempdir().unwrap();
    let (_app, dir) = project(tmp.path()).await;
    git_cli(&dir, &["checkout", "-b", "content/a"]);
    std::fs::write(dir.join(WELCOME), "ours\n").unwrap();
    git::checkpoint(&dir, "ours").await.unwrap().unwrap();
    git_cli(&dir, &["checkout", "main"]);
    std::fs::write(dir.join(WELCOME), "theirs\n").unwrap();
    git::checkpoint(&dir, "theirs").await.unwrap().unwrap();
    git_cli(&dir, &["checkout", "content/a"]);
    let merge = std::process::Command::new("git")
        .args(["merge", "main"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(!merge.status.success(), "expected a conflict");
    let head = git_cli(&dir, &["rev-parse", "HEAD"]);

    assert_eq!(git::checkpoint(&dir, "turn end").await.unwrap(), None);
    assert_eq!(git_cli(&dir, &["rev-parse", "HEAD"]), head);
}

#[tokio::test]
async fn log_and_diff_endpoints_match_git() {
    let tmp = tempfile::tempdir().unwrap();
    let (app, dir) = project(tmp.path()).await;
    std::fs::write(dir.join(WELCOME), "changed\n").unwrap();
    let cp = git::checkpoint(&dir, "Edit hero").await.unwrap().unwrap();

    let (s, body) = call(&app, req("GET", "/api/checkpoints?project=site", None)).await;
    assert_eq!(s, StatusCode::OK);
    let list = body["checkpoints"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0]["sha"], cp.sha);
    assert_eq!(list[0]["summary"], "Edit hero");
    assert_eq!(list[0]["author"], "orso");
    assert!(list[0]["ts"].as_i64().unwrap() > 1_600_000_000);
    assert!(list[1]["summary"]
        .as_str()
        .unwrap()
        .starts_with("Initial commit"));

    let (s, headers, bytes) = raw(
        &app,
        req(
            "GET",
            &format!("/api/checkpoints/{}/diff?project=site", cp.sha),
            None,
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert!(headers[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("text/plain"));
    let diff = String::from_utf8(bytes).unwrap();
    assert!(
        diff.starts_with("diff --git a/src/content/blog/en/welcome.md"),
        "{diff}"
    );
    assert!(diff.contains("+changed"));
    assert_eq!(
        diff.trim_end(),
        git_cli(&dir, &["show", "--format=", "--patch", &cp.sha]).trim_end()
    );

    // abbreviated sha works; the root commit diffs as all-added
    let root = &list[1]["sha"].as_str().unwrap()[..8];
    let (s, _, bytes) = raw(
        &app,
        req(
            "GET",
            &format!("/api/checkpoints/{root}/diff?project=site"),
            None,
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert!(String::from_utf8(bytes).unwrap().contains("new file mode"));
}

#[tokio::test]
async fn restore_resets_files_and_cleans_untracked() {
    let tmp = tempfile::tempdir().unwrap();
    let (app, dir) = project(tmp.path()).await;
    let original = std::fs::read_to_string(dir.join(WELCOME)).unwrap();
    let base = git_cli(&dir, &["rev-parse", "HEAD"]);

    std::fs::write(dir.join(WELCOME), "changed\n").unwrap();
    git::checkpoint(&dir, "Edit hero").await.unwrap().unwrap();

    // more dirt: modified tracked, untracked file + dir, deleted file, plus an ignored file
    std::fs::write(dir.join(WELCOME), "dirty\n").unwrap();
    std::fs::create_dir_all(dir.join("scratch/deep")).unwrap();
    std::fs::write(dir.join("scratch/deep/x.txt"), "x").unwrap();
    std::fs::remove_file(dir.join("AGENTS.md")).unwrap();
    std::fs::create_dir_all(dir.join("node_modules/pkg")).unwrap();
    std::fs::write(dir.join("node_modules/pkg/i.js"), "keep").unwrap();

    let (s, body) = call(
        &app,
        req(
            "POST",
            &format!("/api/checkpoints/{base}/restore"),
            Some(json!({ "project": "site" })),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["restored"], base);
    assert_eq!(git_cli(&dir, &["rev-parse", "HEAD"]), base);
    assert_eq!(
        std::fs::read_to_string(dir.join(WELCOME)).unwrap(),
        original
    );
    assert!(dir.join("AGENTS.md").is_file());
    assert!(!dir.join("scratch").exists());
    assert_eq!(git_cli(&dir, &["status", "--porcelain"]), "");
    assert!(
        dir.join("node_modules/pkg/i.js").is_file(),
        "ignored files survive"
    );

    let (_, body) = call(&app, req("GET", "/api/checkpoints?project=site", None)).await;
    assert_eq!(body["checkpoints"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn checkpoint_errors_use_envelope() {
    let tmp = tempfile::tempdir().unwrap();
    let (app, _dir) = project(tmp.path()).await;
    let bad = [
        ("GET", "/api/checkpoints?project=nope", None, 404, "ENOENT"),
        ("GET", "/api/checkpoints", None, 400, "BADREQ"),
        (
            "GET",
            "/api/checkpoints/zzzz/diff?project=site",
            None,
            400,
            "BADREQ",
        ),
        (
            "GET",
            "/api/checkpoints/--all/diff?project=site",
            None,
            400,
            "BADREQ",
        ),
        (
            "GET",
            "/api/checkpoints/deadbeef/diff?project=site",
            None,
            404,
            "ENOENT",
        ),
        (
            "POST",
            "/api/checkpoints/deadbeef/restore",
            Some(json!({"project": "site"})),
            404,
            "ENOENT",
        ),
        (
            "POST",
            "/api/checkpoints/deadbeef/restore",
            Some(json!({"project": "nope"})),
            404,
            "ENOENT",
        ),
        (
            "POST",
            "/api/checkpoints/deadbeef/restore",
            Some(json!({})),
            400,
            "BADREQ",
        ),
    ];
    for (m, u, b, status, code) in bad {
        let (s, v) = call(&app, req(m, u, b)).await;
        assert_eq!(s.as_u16(), status, "{m} {u}: {v}");
        assert_eq!(v["error"]["code"], code, "{m} {u}");
    }
}

#[tokio::test]
async fn restore_rejects_commits_outside_current_history() {
    let tmp = tempfile::tempdir().unwrap();
    let (app, dir) = project(tmp.path()).await;
    let base = git_cli(&dir, &["rev-parse", "HEAD"]);
    git_cli(&dir, &["checkout", "-q", "-b", "other"]);
    std::fs::write(dir.join("o.txt"), "o").unwrap();
    let other = git::checkpoint(&dir, "other").await.unwrap().unwrap();
    git_cli(&dir, &["checkout", "-q", "main"]);
    assert_eq!(git_cli(&dir, &["rev-parse", "HEAD"]), base);

    let (s, v) = call(
        &app,
        req(
            "POST",
            &format!("/api/checkpoints/{}/restore", other.sha),
            Some(json!({"project": "site"})),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
}

#[tokio::test]
async fn tree_and_file_reads() {
    let tmp = tempfile::tempdir().unwrap();
    let (app, dir) = project(tmp.path()).await;
    std::fs::write(dir.join("blob.bin"), [0xff, 0xfe, 0x00, 0x01]).unwrap();
    std::fs::write(dir.join("pic.svg"), "<svg/>").unwrap();

    let (s, v) = call(&app, req("GET", "/api/tree?project=site", None)).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let names: Vec<_> = v["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap().to_string())
        .collect();
    assert!(!names.contains(&".git".to_string()));
    assert!(names.contains(&"package.json".to_string()));
    let src = names.iter().position(|n| n == "src").unwrap();
    let pkg = names.iter().position(|n| n == "package.json").unwrap();
    assert!(src < pkg, "dirs sort before files: {names:?}");

    let (s, v) = call(
        &app,
        req(
            "GET",
            "/api/tree?project=site&path=src/content/blog/en",
            None,
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let welcome = v["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "welcome.md")
        .unwrap();
    assert_eq!(welcome["kind"], "file");
    assert_eq!(welcome["path"], WELCOME);
    assert!(welcome["size"].as_u64().unwrap() > 0);

    let (s, v) = call(
        &app,
        req(
            "GET",
            &format!("/api/file?project=site&path={WELCOME}"),
            None,
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["binary"], false);
    assert_eq!(
        v["content"].as_str().unwrap(),
        std::fs::read_to_string(dir.join(WELCOME)).unwrap()
    );

    let (_, v) = call(
        &app,
        req("GET", "/api/file?project=site&path=blob.bin", None),
    )
    .await;
    assert_eq!(v["binary"], true);
    assert_eq!(v["content"], Value::Null);

    let (s, h, b) = raw(
        &app,
        req("GET", "/api/file?project=site&path=pic.svg&raw=true", None),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h[header::CONTENT_TYPE], "image/svg+xml");
    assert_eq!(h[header::CONTENT_SECURITY_POLICY], "sandbox");
    assert_eq!(b, b"<svg/>");

    for (u, status, code) in [
        ("/api/file?project=site&path=missing.txt", 404, "ENOENT"),
        ("/api/file?project=site&path=src", 400, "BADREQ"),
        ("/api/tree?project=site&path=package.json", 400, "BADREQ"),
        ("/api/file?project=site", 400, "BADREQ"),
        ("/api/tree?project=nope", 404, "ENOENT"),
    ] {
        let (s, v) = call(&app, req("GET", u, None)).await;
        assert_eq!(s.as_u16(), status, "{u}: {v}");
        assert_eq!(v["error"]["code"], code, "{u}");
    }
}

#[tokio::test]
async fn file_apis_are_sandboxed() {
    let tmp = tempfile::tempdir().unwrap();
    let (app, dir) = project(tmp.path()).await;
    std::fs::write(tmp.path().join("secret.txt"), "top secret").unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("out.txt"), "outside").unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.join("link-dir")).unwrap();
    std::os::unix::fs::symlink(outside.path().join("out.txt"), dir.join("link-file")).unwrap();
    std::os::unix::fs::symlink(dir.join("package.json"), dir.join("link-inside")).unwrap();
    let abs = format!("{}/secret.txt", tmp.path().display());

    let attempts = [
        "path=../secret.txt".to_string(),
        "path=src/../../secret.txt".to_string(),
        "path=%2E%2E%2Fsecret.txt".to_string(),
        "path=..".to_string(),
        format!("path={abs}"),
        "path=%2Fetc%2Fpasswd".to_string(),
        "path=link-dir/out.txt".to_string(),
        "path=link-file".to_string(),
        "path=link-dir".to_string(),
        "path=.git/config".to_string(),
        "path=.git".to_string(),
        "path=src/%00x".to_string(),
    ];
    for a in &attempts {
        for ep in ["file", "tree"] {
            let (s, v) = call(
                &app,
                req("GET", &format!("/api/{ep}?project=site&{a}"), None),
            )
            .await;
            assert!(
                s == StatusCode::FORBIDDEN || s == StatusCode::BAD_REQUEST,
                "{ep} {a} -> {s} {v}"
            );
            assert!(
                matches!(v["error"]["code"].as_str(), Some("SANDBOX" | "BADREQ")),
                "{ep} {a}: {v}"
            );
        }
    }
    // an escaping symlink is denied, not leaked
    let (s, v) = call(
        &app,
        req("GET", "/api/file?project=site&path=link-file", None),
    )
    .await;
    assert_eq!(
        (s, v["error"]["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("SANDBOX"))
    );

    // tree hides escaping links but shows contained ones
    let (_, v) = call(&app, req("GET", "/api/tree?project=site", None)).await;
    let names: Vec<_> = v["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert!(
        !names.contains(&"link-dir") && !names.contains(&"link-file"),
        "{names:?}"
    );
    assert!(names.contains(&"link-inside"));
    let (s, v) = call(
        &app,
        req("GET", "/api/file?project=site&path=link-inside", None),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn endpoints_require_auth_and_are_read_only() {
    let tmp = tempfile::tempdir().unwrap();
    let app = http::router(AppState::with_scaffold_install(
        config(tmp.path(), false),
        false,
    ));
    for (m, u) in [
        ("GET", "/api/checkpoints?project=x"),
        ("GET", "/api/checkpoints/abcd/diff?project=x"),
        ("POST", "/api/checkpoints/abcd/restore"),
        ("GET", "/api/tree?project=x"),
        ("GET", "/api/file?project=x&path=a"),
    ] {
        let (s, v) = call(&app, req(m, u, None)).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{m} {u}");
        assert_eq!(v["error"]["code"], "UNAUTH");
    }

    // no write verbs on the file APIs
    let app = http::router(state(tmp.path()));
    for m in ["POST", "PUT", "DELETE"] {
        let (s, _) = call(&app, req(m, "/api/file?project=x&path=a", None)).await;
        assert_eq!(s, StatusCode::METHOD_NOT_ALLOWED, "{m}");
    }
}

#[tokio::test]
async fn nested_vcs_dirs_are_blocked_and_hidden_at_any_depth() {
    let tmp = tempfile::tempdir().unwrap();
    let (app, dir) = project(tmp.path()).await;
    // nested repo, upper-case variant, other VCS, and links into VCS metadata
    for d in [
        "vendor/lib/.git",
        "vendor/lib/.GIT",
        "vendor/.hg",
        "vendor/.svn",
    ] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
        std::fs::write(dir.join(d).join("config"), "[core]").unwrap();
    }
    std::fs::write(dir.join("vendor/lib/ok.txt"), "ok").unwrap();
    std::os::unix::fs::symlink(".git/config", dir.join("link-to-git-config")).unwrap();
    std::os::unix::fs::symlink("vendor/lib/.git", dir.join("link-to-nested-git")).unwrap();

    for u in [
        "/api/file?project=site&path=vendor/lib/.git/config",
        "/api/file?project=site&path=vendor/lib/.GIT/config",
        "/api/file?project=site&path=vendor/.hg/config",
        "/api/file?project=site&path=vendor/.svn/config",
        "/api/file?project=site&path=link-to-git-config",
        "/api/file?project=site&path=link-to-nested-git/config",
        "/api/tree?project=site&path=vendor/lib/.git",
        "/api/tree?project=site&path=link-to-nested-git",
    ] {
        let (s, v) = call(&app, req("GET", u, None)).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{u}: {v}");
        assert_eq!(v["error"]["code"], "SANDBOX", "{u}");
    }

    let names = |v: &Value| -> Vec<String> {
        v["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["name"].as_str().unwrap().to_string())
            .collect()
    };
    let (_, v) = call(
        &app,
        req("GET", "/api/tree?project=site&path=vendor/lib", None),
    )
    .await;
    assert_eq!(names(&v), ["ok.txt"]);
    let (_, v) = call(&app, req("GET", "/api/tree?project=site&path=vendor", None)).await;
    assert_eq!(names(&v), ["lib"]);
    // links into VCS metadata are hidden from the root listing
    let (_, v) = call(&app, req("GET", "/api/tree?project=site", None)).await;
    let root = names(&v);
    assert!(
        !root.contains(&"link-to-git-config".to_string()),
        "{root:?}"
    );
    assert!(
        !root.contains(&"link-to-nested-git".to_string()),
        "{root:?}"
    );
    assert!(!root.contains(&".git".to_string()));
}

#[tokio::test]
async fn oversized_files_are_rejected_with_413() {
    let tmp = tempfile::tempdir().unwrap();
    let (app, dir) = project(tmp.path()).await;
    let max = orso::fs::MAX_FILE_BYTES as usize;
    std::fs::write(dir.join("ok.bin"), vec![b'a'; max]).unwrap();
    std::fs::write(dir.join("big.bin"), vec![b'a'; max + 1]).unwrap();
    let (s, _, b) = raw(
        &app,
        req("GET", "/api/file?project=site&path=ok.bin&raw=true", None),
    )
    .await;
    assert_eq!((s, b.len()), (StatusCode::OK, max));
    for u in [
        "/api/file?project=site&path=big.bin",
        "/api/file?project=site&path=big.bin&raw=true",
    ] {
        let (s, v) = call(&app, req("GET", u, None)).await;
        assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE, "{u}");
        assert_eq!(v["error"]["code"], "BADREQ");
    }
}

fn seed_bare(tmp: &Path) -> std::path::PathBuf {
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

#[tokio::test]
async fn finish_turn_commits_and_lists_files() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(tmp.path());
    let info = st.projects.create("Site").await.unwrap();
    let dir = tmp.path().join(&info.id);
    orso::changes::create(&dir, "New post: Launch week")
        .await
        .unwrap();
    std::fs::write(dir.join("b.md"), "b\n").unwrap();
    std::fs::write(dir.join("a.md"), "a\n").unwrap();

    match finish_turn(&st, &info.id, Some("Add posts")).await.unwrap() {
        TurnOutcome::Committed { checkpoint, files } => {
            assert_eq!(files, ["a.md", "b.md"]);
            assert_eq!(checkpoint.summary, "Add posts");
            assert_eq!(checkpoint.sha, git_cli(&dir, &["rev-parse", "HEAD"]));
        }
        other => panic!("expected Committed, got {other:?}"),
    }
    assert!(matches!(
        finish_turn(&st, &info.id, Some("Again")).await.unwrap(),
        TurnOutcome::Unchanged
    ));
}

#[tokio::test]
async fn finish_turn_blocked_on_main_with_remote() {
    let tmp = tempfile::tempdir().unwrap();
    let st = state(tmp.path());
    let bare = seed_bare(&tmp.path().join("gh"));
    let gh = st
        .projects
        .create_from(
            "gh-site",
            Source::Clone {
                repo: "acme/site".into(),
                url: format!("file://{}", bare.display()),
                env: vec![],
            },
            &git::Identity::orso(),
        )
        .await
        .unwrap();
    let dir = tmp.path().join(&gh.id);

    // Talk-only turn on main: nothing to block.
    assert!(matches!(
        finish_turn(&st, &gh.id, None).await.unwrap(),
        TurnOutcome::Unchanged
    ));

    std::fs::write(dir.join("package.json"), "{\"edited\": true}\n").unwrap();
    let mut ev = st.events.subscribe();
    let base = git_cli(&dir, &["rev-parse", "HEAD"]);
    let outcome = finish_turn(&st, &gh.id, Some("Edit")).await.unwrap();
    assert!(matches!(outcome, TurnOutcome::Blocked(_)), "{outcome:?}");
    assert_eq!(git_cli(&dir, &["rev-parse", "HEAD"]), base);
    let mut blocked = false;
    while let Ok(text) = ev.try_recv() {
        let v: Value = serde_json::from_str(&text).unwrap();
        blocked |= v["type"] == "checkpoint.blocked" && v["projectId"] == gh.id;
    }
    assert!(blocked);
}

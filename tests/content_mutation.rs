//! Task 5: safe structured content create/update over HTTP.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use orso::config::Config;
use orso::content::integration;
use orso::projects::Source;
use orso::state::AppState;
use orso::{changes, git, http};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

const WELCOME: &str = "src/content/blog/en/welcome.md";

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

struct Env {
    _tmp: tempfile::TempDir,
    state: AppState,
    app: axum::Router,
    id: String,
    dir: PathBuf,
}

/// A template project with its fake dev server ready, optionally on a change branch.
async fn env(change: bool) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::with_options(
        Config {
            port: 0,
            origin: None,
            projects_dir: tmp.path().to_path_buf(),
            no_auth: true,
            token: None,
            agents_config: None,
            ..Default::default()
        },
        false,
        common::fake_devserver(),
    );
    let info = state.projects.create("Site").await.unwrap();
    let dir = tmp.path().join(&info.id);
    common::write_content_manifest(&dir, "");
    common::start_fake(&state, &info.id).await;
    if change {
        changes::create(&dir, "Edit content").await.unwrap();
    }
    Env {
        app: http::router(state.clone()),
        _tmp: tmp,
        state,
        id: info.id,
        dir,
    }
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
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
    let res = app.clone().oneshot(b.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn read(e: &Env, collection: &str, id: &str) -> Value {
    let (s, v) = call(
        &e.app,
        "GET",
        &format!(
            "/api/content/entry?project={}&collection={collection}&id={id}",
            e.id
        ),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v
}

fn post(e: &Env, branch: &str, collection: &str, slug: &str, fields: Value, body: &str) -> Value {
    json!({"project":e.id,"collection":collection,"slug":slug,"branch":branch,"fields":fields,"body":body})
}

fn post_fields() -> Value {
    json!({"title":"Launch week","description":"What shipped","pubDate":"2026-10-12","tags":["launch"]})
}

fn head(dir: &Path) -> String {
    git_cli(dir, &["rev-parse", "HEAD"])
}

#[tokio::test]
async fn local_and_github_projects_reject_writes_on_main() {
    let e = env(false).await;
    let original = std::fs::read(e.dir.join(WELCOME)).unwrap();
    let entry = read(&e, "blog", "welcome").await;
    let (s, v) = call(
        &e.app,
        "PUT",
        "/api/content/entry",
        Some(json!({"project":e.id,"collection":"blog","id":"welcome","branch":"main","hash":entry["hash"],"fields":{"title":"Changed"}})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["code"], "CHANGE_REQUIRED");
    let (s, v) = call(
        &e.app,
        "POST",
        "/api/content/entry",
        Some(post(
            &e,
            "main",
            "blog",
            "new-post",
            post_fields(),
            "Body\n",
        )),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["code"], "CHANGE_REQUIRED");
    assert_eq!(std::fs::read(e.dir.join(WELCOME)).unwrap(), original);
    assert!(!e.dir.join("src/content/blog/en/new-post.md").exists());

    // A GitHub project cloned from the same history is refused on main too.
    let bare = e._tmp.path().join("remote.git");
    git_cli(
        e._tmp.path(),
        &[
            "clone",
            "--bare",
            e.dir.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );
    let gh = e
        .state
        .projects
        .create_from(
            "GitHub site",
            Source::Clone {
                repo: "acme/site".into(),
                url: format!("file://{}", bare.display()),
                env: vec![],
            },
            &git::Identity::orso(),
        )
        .await
        .unwrap();
    let (s, v) = call(
        &e.app,
        "PUT",
        "/api/content/entry",
        Some(json!({"project":gh.id,"collection":"blog","id":"welcome","branch":"main","hash":entry["hash"],"fields":{"title":"Changed"}})),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["code"], "CHANGE_REQUIRED");
}

#[tokio::test]
async fn create_and_update_succeed_on_active_change_with_scoped_checkpoints() {
    let e = env(true).await;
    let branch = git_cli(&e.dir, &["branch", "--show-current"]);
    let mut events = e.state.events.subscribe();
    let mut fields = post_fields();
    fields["coverImage"] = json!("./cover.png");
    fields["coverAlt"] = json!("A cover");
    let (s, created) = call(
        &e.app,
        "POST",
        "/api/content/entry",
        Some(post(
            &e,
            &branch,
            "blog",
            "launch-week",
            fields,
            "## Hello\n",
        )),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{created}");
    assert_eq!(created["path"], "src/content/blog/en/launch-week.md");
    assert_eq!(created["id"], "launch-week");
    assert_eq!(created["fields"]["title"], "Launch week");
    assert_eq!(created["body"], "## Hello\n");
    assert_eq!(created["branch"], branch.as_str());
    // Dates stay strings for every YAML parser (Astro's included).
    assert!(
        created["source"]
            .as_str()
            .unwrap()
            .contains("pubDate: \"2026-10-12\"\n"),
        "{}",
        created["source"]
    );
    assert_eq!(
        git_cli(&e.dir, &["show", "--format=", "--name-only", "HEAD"]),
        "src/content/blog/en/launch-week.md"
    );
    let mut kinds = vec![];
    while let Ok(text) = events.try_recv() {
        let v: Value = serde_json::from_str(&text).unwrap();
        kinds.push(v["type"].as_str().unwrap().to_string());
    }
    assert!(
        kinds.contains(&"checkpoint.created".to_string()),
        "{kinds:?}"
    );
    assert!(kinds.contains(&"fs.change".to_string()), "{kinds:?}");

    // Update one field, then clear optional fields with `unset`; untouched source stays.
    let (s, updated) = call(
        &e.app,
        "PUT",
        "/api/content/entry",
        Some(json!({"project":e.id,"collection":"blog","id":"launch-week","branch":branch,"hash":created["hash"],"fields":{"title":"Launch week recap"}})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{updated}");
    assert_eq!(
        updated["source"],
        created["source"]
            .as_str()
            .unwrap()
            .replace("Launch week", "Launch week recap")
    );
    let (s, cleared) = call(
        &e.app,
        "PUT",
        "/api/content/entry",
        Some(json!({"project":e.id,"collection":"blog","id":"launch-week","branch":branch,"hash":updated["hash"],"fields":{},"unset":["coverImage","coverAlt"]})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{cleared}");
    assert!(cleared["fields"].get("coverImage").is_none());
    assert!(cleared["fields"].get("coverAlt").is_none());
    assert_eq!(cleared["body"], "## Hello\n");
    assert_eq!(git_cli(&e.dir, &["status", "--porcelain"]), "");
}

#[tokio::test]
async fn stale_hash_and_changed_branch_reject_without_writes() {
    let e = env(true).await;
    let branch = git_cli(&e.dir, &["branch", "--show-current"]);
    let entry = read(&e, "blog", "welcome").await;
    let before = std::fs::read(e.dir.join(WELCOME)).unwrap();
    let base = head(&e.dir);
    for (body, reason) in [
        (
            json!({"project":e.id,"collection":"blog","id":"welcome","branch":branch,"hash":"0".repeat(64),"fields":{"title":"X"}}),
            "stale_content",
        ),
        (
            json!({"project":e.id,"collection":"blog","id":"welcome","branch":"content/other","hash":entry["hash"],"fields":{"title":"X"}}),
            "branch_changed",
        ),
    ] {
        let (s, v) = call(&e.app, "PUT", "/api/content/entry", Some(body)).await;
        assert_eq!(s, StatusCode::CONFLICT, "{v}");
        assert_eq!(v["error"]["reason"], reason);
    }
    let (s, v) = call(
        &e.app,
        "POST",
        "/api/content/entry",
        Some(post(
            &e,
            "content/other",
            "blog",
            "fresh",
            post_fields(),
            "",
        )),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["reason"], "branch_changed");
    assert!(!e.dir.join("src/content/blog/en/fresh.md").exists());
    assert_eq!(std::fs::read(e.dir.join(WELCOME)).unwrap(), before);
    assert_eq!(head(&e.dir), base);
}

#[tokio::test]
async fn duplicate_and_invalid_slugs_reject() {
    let e = env(true).await;
    let branch = git_cli(&e.dir, &["branch", "--show-current"]);
    let base = head(&e.dir);
    let (s, v) = call(
        &e.app,
        "POST",
        "/api/content/entry",
        Some(post(&e, &branch, "blog", "welcome", post_fields(), "")),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["reason"], "duplicate_slug");
    for slug in [
        "Bad Slug",
        "../escape",
        "a/b",
        "",
        "-lead",
        "trail-",
        "UPPER",
        "a..b",
        "x\u{0}y",
    ] {
        let (s, v) = call(
            &e.app,
            "POST",
            "/api/content/entry",
            Some(post(&e, &branch, "blog", slug, post_fields(), "")),
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{slug}: {v}");
        assert_eq!(v["error"]["reason"], "invalid_slug", "{slug}");
    }
    let (s, v) = call(
        &e.app,
        "POST",
        "/api/content/entry",
        Some(post(&e, &branch, "nope", "x", json!({}), "")),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["error"]["reason"], "unsupported_collection");
    assert_eq!(head(&e.dir), base);
    assert_eq!(git_cli(&e.dir, &["status", "--porcelain"]), "");
}

#[tokio::test]
async fn schema_errors_contain_field_paths_and_leave_files_untouched() {
    let e = env(true).await;
    let branch = git_cli(&e.dir, &["branch", "--show-current"]);
    let entry = read(&e, "blog", "welcome").await;
    let before = std::fs::read(e.dir.join(WELCOME)).unwrap();
    let (s, v) = call(
        &e.app,
        "PUT",
        "/api/content/entry",
        Some(json!({"project":e.id,"collection":"blog","id":"welcome","branch":branch,"hash":entry["hash"],"fields":{"pubDate":"2026-02-30"}})),
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["error"]["reason"], "invalid_schema");
    assert_eq!(v["error"]["details"][0]["path"], json!(["pubDate"]));
    assert!(v["error"]["details"][0]["message"].is_string());
    assert_eq!(std::fs::read(e.dir.join(WELCOME)).unwrap(), before);

    let (s, v) = call(
        &e.app,
        "POST",
        "/api/content/entry",
        Some(post(
            &e,
            &branch,
            "blog",
            "partial",
            json!({"title":"Only"}),
            "",
        )),
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    let paths: Vec<Value> = v["error"]["details"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["path"].clone())
        .collect();
    assert!(paths.contains(&json!(["description"])), "{paths:?}");
    assert!(!e.dir.join("src/content/blog/en/partial.md").exists());
    assert_eq!(git_cli(&e.dir, &["status", "--porcelain"]), "");
}

#[tokio::test]
async fn symlink_escapes_reject_without_writing_outside() {
    let e = env(true).await;
    let branch = git_cli(&e.dir, &["branch", "--show-current"]);
    let outside = tempfile::tempdir().unwrap();
    let secret = "---\ntitle: T\ndescription: D\npubDate: \"2026-10-11\"\n---\n";
    std::fs::write(outside.path().join("secret.md"), secret).unwrap();
    let hash = format!("{:x}", Sha256::digest(secret.as_bytes()));

    // Entry file symlinked outside the project.
    std::os::unix::fs::symlink(
        outside.path().join("secret.md"),
        e.dir.join("src/content/blog/en/evil.md"),
    )
    .unwrap();
    let (s, v) = call(
        &e.app,
        "PUT",
        "/api/content/entry",
        Some(json!({"project":e.id,"collection":"blog","id":"evil","branch":branch,"hash":hash,"fields":{"title":"Owned"}})),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{v}");
    assert_eq!(v["error"]["reason"], "unsafe_path");

    // Collection directory symlinked outside the project.
    let blog = e.dir.join("src/content/blog/en");
    std::fs::remove_dir_all(&blog).unwrap();
    std::os::unix::fs::symlink(outside.path(), &blog).unwrap();
    let (s, v) = call(
        &e.app,
        "POST",
        "/api/content/entry",
        Some(post(&e, &branch, "blog", "escape", post_fields(), "")),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{v}");
    assert_eq!(v["error"]["reason"], "unsafe_path");
    let (s, v) = call(
        &e.app,
        "PUT",
        "/api/content/entry",
        Some(json!({"project":e.id,"collection":"blog","id":"secret","branch":branch,"hash":hash,"fields":{"title":"Owned"}})),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{v}");

    assert_eq!(
        std::fs::read_to_string(outside.path().join("secret.md")).unwrap(),
        secret
    );
    let names: Vec<_> = std::fs::read_dir(outside.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, vec![std::ffi::OsString::from("secret.md")]);
}

#[tokio::test]
async fn unchanged_save_preserves_mtime_and_creates_no_checkpoint() {
    let e = env(true).await;
    let branch = git_cli(&e.dir, &["branch", "--show-current"]);
    let entry = read(&e, "blog", "welcome").await;
    let mtime = std::fs::metadata(e.dir.join(WELCOME))
        .unwrap()
        .modified()
        .unwrap();
    let base = head(&e.dir);
    let (s, v) = call(
        &e.app,
        "PUT",
        "/api/content/entry",
        Some(json!({"project":e.id,"collection":"blog","id":"welcome","branch":branch,"hash":entry["hash"],"fields":entry["fields"],"body":entry["body"]})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["hash"], entry["hash"]);
    assert_eq!(
        std::fs::metadata(e.dir.join(WELCOME))
            .unwrap()
            .modified()
            .unwrap(),
        mtime
    );
    assert_eq!(head(&e.dir), base);
}

#[tokio::test]
async fn unrelated_staged_and_unstaged_changes_survive_a_save() {
    let e = env(true).await;
    let branch = git_cli(&e.dir, &["branch", "--show-current"]);
    std::fs::write(e.dir.join("README.md"), "staged\n").unwrap();
    git_cli(&e.dir, &["add", "README.md"]);
    std::fs::write(e.dir.join("README.md"), "staged then unstaged\n").unwrap();
    std::fs::write(e.dir.join("astro.config.mjs"), "// unstaged\n").unwrap();
    let staged = git_cli(&e.dir, &["diff", "--cached", "--binary"]);
    let unstaged = git_cli(&e.dir, &["diff", "--binary"]);
    let entry = read(&e, "blog", "welcome").await;
    let (s, v) = call(
        &e.app,
        "PUT",
        "/api/content/entry",
        Some(json!({"project":e.id,"collection":"blog","id":"welcome","branch":branch,"hash":entry["hash"],"fields":{"title":"New title"},"body":"Rewritten\n"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert!(v["source"].as_str().unwrap().ends_with("---\nRewritten\n"));
    assert_eq!(
        git_cli(&e.dir, &["show", "--format=", "--name-only", "HEAD"]),
        WELCOME
    );
    assert_eq!(git_cli(&e.dir, &["diff", "--cached", "--binary"]), staged);
    assert_eq!(git_cli(&e.dir, &["diff", "--binary"]), unstaged);
}

#[tokio::test]
async fn dangling_and_empty_references_are_refused_without_writes() {
    let e = env(true).await;
    let branch = git_cli(&e.dir, &["branch", "--show-current"]);
    let welcome = read(&e, "blog", "welcome").await;
    let before = std::fs::read(e.dir.join(WELCOME)).unwrap();
    for (related, needle) in [
        (json!(["welcome", "missing-post"]), "missing-post"),
        (json!(["welcome", ""]), "Choose"),
    ] {
        let (s, v) = call(
            &e.app,
            "PUT",
            "/api/content/entry",
            Some(json!({"project":e.id,"collection":"blog","id":"welcome","branch":branch,"hash":welcome["hash"],"fields":{"relatedPosts":related}})),
        )
        .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
        assert_eq!(v["error"]["reason"], "invalid_schema");
        assert_eq!(
            v["error"]["details"][0]["path"],
            json!(["relatedPosts", 1]),
            "{v}"
        );
        assert!(
            v["error"]["details"][0]["message"]
                .as_str()
                .unwrap()
                .contains(needle),
            "{v}"
        );
        assert_eq!(std::fs::read(e.dir.join(WELCOME)).unwrap(), before);
    }
    // A new entry is checked too: relatedPosts must name existing posts.
    let mut fields = post_fields();
    fields["relatedPosts"] = json!(["welcome", "missing-post"]);
    let (s, v) = call(
        &e.app,
        "POST",
        "/api/content/entry",
        Some(post(&e, &branch, "blog", "related", fields.clone(), "")),
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert_eq!(v["error"]["details"][0]["path"], json!(["relatedPosts", 1]));
    assert!(!e.dir.join("src/content/blog/en/related.md").exists());
    assert_eq!(git_cli(&e.dir, &["status", "--porcelain"]), "");

    // Existing targets save.
    fields["relatedPosts"] = json!(["welcome"]);
    let (s, v) = call(
        &e.app,
        "POST",
        "/api/content/entry",
        Some(post(&e, &branch, "blog", "related", fields, "")),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let (s, v) = call(
        &e.app,
        "PUT",
        "/api/content/entry",
        Some(json!({"project":e.id,"collection":"blog","id":"welcome","branch":branch,"hash":welcome["hash"],"fields":{"relatedPosts":["related"]}})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn without_a_ready_preview_collections_are_read_only_and_saves_write_nothing() {
    let e = env(true).await;
    let mut readonly =
        common::manifest_collection("pages", "src/content/pages", "*.md", "markdown", json!({}));
    readonly["supportedForEditing"] = json!(false);
    readonly["reason"] = json!("Uses a custom loader");
    let manifest = e.dir.join(".astro/orso-content.json");
    let mut m: Value = serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    m["collections"].as_array_mut().unwrap().push(readonly);
    std::fs::write(&manifest, m.to_string()).unwrap();
    let branch = git_cli(&e.dir, &["branch", "--show-current"]);
    let entry = read(&e, "blog", "welcome").await;
    common::stop_fake(&e.state, &e.id).await;

    let uri = format!("/api/content/collections?project={}", e.id);
    let (s, v) = call(&e.app, "GET", &uri, None).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let collections = v["collections"].as_array().unwrap();
    assert_eq!(collections.len(), 3, "{v}");
    for c in collections {
        assert_eq!(c["editable"], false, "{c}");
        let reason = if c["name"] == "pages" {
            "Uses a custom loader"
        } else {
            integration::PREVIEW_REASON
        };
        assert_eq!(c["reason"], reason, "{c}");
    }

    let before = std::fs::read(e.dir.join(WELCOME)).unwrap();
    let base = head(&e.dir);
    let (s, v) = call(
        &e.app,
        "PUT",
        "/api/content/entry",
        Some(json!({"project":e.id,"collection":"blog","id":"welcome","branch":branch,"hash":entry["hash"],"fields":{"title":"Changed"}})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_GATEWAY, "{v}");
    assert_eq!(v["error"]["code"], "DEVSERVER_ERR");
    assert_eq!(v["error"]["message"], integration::PREVIEW_SAVE_REASON);
    let (s, v) = call(
        &e.app,
        "POST",
        "/api/content/entry",
        Some(post(&e, &branch, "blog", "fresh", post_fields(), "")),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_GATEWAY, "{v}");
    assert_eq!(v["error"]["code"], "DEVSERVER_ERR");
    assert_eq!(std::fs::read(e.dir.join(WELCOME)).unwrap(), before);
    assert!(!e.dir.join("src/content/blog/en/fresh.md").exists());
    assert_eq!(head(&e.dir), base);
    assert_eq!(git_cli(&e.dir, &["status", "--porcelain"]), "");
}

//! Skills CRUD, install, and global-path settings over HTTP.

use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use orso::config::Config;
use orso::state::AppState;
use orso::{changes, http};
use serde_json::{json, Value};
use tower::ServiceExt;

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

fn link_node_modules(dir: &Path) {
    let modules =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/astro-marketing/node_modules");
    std::os::unix::fs::symlink(modules, dir.join("node_modules")).unwrap();
    // `.gitignore` has `node_modules/`, which does not match a symlink.
    let exclude = dir.join(".git/info/exclude");
    let mut text = std::fs::read_to_string(&exclude).unwrap_or_default();
    text.push_str("\n/node_modules\n");
    std::fs::write(exclude, text).unwrap();
}

struct Env {
    _tmp: tempfile::TempDir,
    app: axum::Router,
    id: String,
    dir: PathBuf,
}

impl Env {
    fn projects_dir(&self) -> PathBuf {
        self.dir.parent().unwrap().to_path_buf()
    }
}

/// A template project, optionally on a change branch.
async fn env(change: bool) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::with_scaffold_install(
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
    );
    let info = state.projects.create("Site").await.unwrap();
    let dir = tmp.path().join(&info.id);
    link_node_modules(&dir);
    if change {
        changes::create(&dir, "Edit skills").await.unwrap();
    }
    Env {
        app: http::router(state),
        _tmp: tmp,
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

fn branch(e: &Env) -> String {
    git_cli(&e.dir, &["branch", "--show-current"])
}

async fn list(e: &Env) -> (StatusCode, Value) {
    call(&e.app, "GET", &format!("/api/skills?project={}", e.id), None).await
}

async fn get(e: &Env, id: &str, scope: &str) -> (StatusCode, Value) {
    call(
        &e.app,
        "GET",
        &format!("/api/skills/{id}?project={}&scope={scope}", e.id),
        None,
    )
    .await
}

async fn save(e: &Env, id: &str, body: Value) -> (StatusCode, Value) {
    call(&e.app, "PUT", &format!("/api/skills/{id}"), Some(body)).await
}

async fn install(e: &Env, body: Value) -> (StatusCode, Value) {
    call(&e.app, "POST", "/api/skills/install", Some(body)).await
}

async fn uninstall(e: &Env, id: &str, scope: &str, branch: &str) -> (StatusCode, Value) {
    call(
        &e.app,
        "DELETE",
        &format!(
            "/api/skills/{id}?project={}&scope={scope}&branch={branch}",
            e.id
        ),
        None,
    )
    .await
}

async fn settings_get(e: &Env) -> (StatusCode, Value) {
    call(&e.app, "GET", "/api/skills/settings", None).await
}

async fn settings_put(e: &Env, body: Value) -> (StatusCode, Value) {
    call(&e.app, "PUT", "/api/skills/settings", Some(body)).await
}

#[tokio::test]
async fn list_empty_then_catalog() {
    let e = env(true).await;
    let (s, v) = list(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["installed"], json!([]));
    assert_eq!(v["catalog"].as_array().unwrap().len(), 2);
    assert_eq!(v["catalog"][0]["id"], "seo");
    assert_eq!(v["catalog"][0]["name"], "SEO");
    assert_eq!(v["catalog"][0]["installedProject"], false);
    assert_eq!(v["catalog"][1]["id"], "design");

    let b = branch(&e);
    let (s, v) = install(
        &e,
        json!({"project": e.id, "branch": b, "scope": "project", "source": {"catalog": {"id": "seo"}}}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");

    let (s, v) = list(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["installed"].as_array().unwrap().len(), 1);
    assert_eq!(v["installed"][0]["id"], "seo");
    assert_eq!(v["installed"][0]["scope"], "project");
    assert_eq!(v["catalog"][0]["installedProject"], true);
}

#[tokio::test]
async fn install_catalog_project_checkpoints() {
    let e = env(true).await;
    let b = branch(&e);
    let (s, v) = install(
        &e,
        json!({"project": e.id, "branch": b, "scope": "project", "source": {"catalog": {"id": "seo"}}}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["id"], "seo");
    assert_eq!(v["scope"], "project");
    assert_eq!(v["name"], "SEO");
    assert!(v["description"].as_str().unwrap().contains("optimization"), "{v}");

    let path = e.dir.join(".orso/skills/seo/SKILL.md");
    assert!(path.exists());
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("---\n"), "{text}");
    assert_eq!(
        git_cli(&e.dir, &["log", "-1", "--format=%s"]),
        "checkpoint: Installed skill"
    );
    assert_eq!(
        git_cli(&e.dir, &["status", "--porcelain", ".orso/skills/seo/SKILL.md"]),
        ""
    );
}

#[tokio::test]
async fn install_project_without_change_requires_change() {
    let e = env(false).await;
    let b = branch(&e);
    let (s, v) = install(
        &e,
        json!({"project": e.id, "branch": b, "scope": "project", "source": {"catalog": {"id": "seo"}}}),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["code"], "CHANGE_REQUIRED");
}

#[tokio::test]
async fn install_global_without_change_ok() {
    let e = env(false).await;
    let (s, v) = install(
        &e,
        json!({"project": e.id, "scope": "global", "source": {"catalog": {"id": "design"}}}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["id"], "design");
    assert_eq!(v["scope"], "global");

    let path = e.projects_dir().join(".orso/global-skills/design/SKILL.md");
    assert!(path.exists());
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("Design"));
}

#[tokio::test]
async fn save_project_stale_hash() {
    let e = env(true).await;
    let b = branch(&e);
    install(
        &e,
        json!({"project": e.id, "branch": b, "scope": "project", "source": {"catalog": {"id": "seo"}}}),
    )
    .await;
    let (s, v) = get(&e, "seo", "project").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let hash = v["hash"].as_str().unwrap().to_string();

    let (s, v) = save(
        &e,
        "seo",
        json!({"project": e.id, "branch": b, "scope": "project", "name": "SEO", "description": "Updated", "body": "new body\n", "hash": hash}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["body"], "new body\n");

    // The same (now stale) hash must be refused.
    let (s, v) = save(
        &e,
        "seo",
        json!({"project": e.id, "branch": b, "scope": "project", "name": "SEO", "description": "Updated", "body": "again\n", "hash": hash}),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["reason"], "stale_content");
}

#[tokio::test]
async fn save_global_ok() {
    let e = env(false).await;
    install(
        &e,
        json!({"project": e.id, "scope": "global", "source": {"catalog": {"id": "seo"}}}),
    )
    .await;
    let (s, v) = get(&e, "seo", "global").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let hash = v["hash"].as_str().unwrap().to_string();

    // Omitting name/description keeps the current values; body is replaced.
    let (s, v) = save(
        &e,
        "seo",
        json!({"project": e.id, "scope": "global", "body": "global body\n", "hash": hash}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["name"], "SEO");
    assert_eq!(v["body"], "global body\n");

    let path = e.projects_dir().join(".orso/global-skills/seo/SKILL.md");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("name: SEO"), "{text}");
    assert!(text.ends_with("global body\n"), "{text}");
}

#[tokio::test]
async fn uninstall_project() {
    let e = env(true).await;
    let b = branch(&e);
    install(
        &e,
        json!({"project": e.id, "branch": b, "scope": "project", "source": {"catalog": {"id": "seo"}}}),
    )
    .await;
    let path = e.dir.join(".orso/skills/seo/SKILL.md");
    assert!(path.exists());

    let (s, v) = uninstall(&e, "seo", "project", &b).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert!(!path.exists());
    assert_eq!(
        git_cli(&e.dir, &["log", "-1", "--format=%s"]),
        "checkpoint: Removed skill"
    );

    let (s, v) = get(&e, "seo", "project").await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
}

#[tokio::test]
async fn duplicate_id_conflict() {
    let e = env(true).await;
    let b = branch(&e);
    let body =
        json!({"project": e.id, "branch": b, "scope": "project", "source": {"catalog": {"id": "seo"}}});
    let (s, v) = install(&e, body.clone()).await;
    assert_eq!(s, StatusCode::OK, "{v}");

    let (s, v) = install(&e, body).await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["code"], "CONFLICT");

    // A different scope is a distinct row and installs fine.
    let (s, v) = install(
        &e,
        json!({"project": e.id, "scope": "global", "source": {"catalog": {"id": "seo"}}}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
}

#[tokio::test]
async fn get_invalid_skill_reason() {
    let e = env(false).await;
    let dir = e.dir.join(".orso/skills/broken");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), "---\ndescription: no name\n---\nbody\n").unwrap();

    let (s, v) = get(&e, "broken", "project").await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["error"]["reason"], "invalid_skill");

    // Invalid skills are skipped in the list.
    let (s, v) = list(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["installed"], json!([]));
}

#[tokio::test]
async fn settings_round_trip_and_reset() {
    let e = env(false).await;
    let projects = e.projects_dir();
    let default = projects.join(".orso/global-skills");
    let default_str = default.to_string_lossy().into_owned();

    let (s, v) = settings_get(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["globalPath"], default_str.as_str());
    assert_eq!(v["defaultGlobalPath"], default_str.as_str());

    // Relative paths are rejected.
    let (s, v) = settings_put(&e, json!({"globalPath": "relative/path"})).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");

    let custom = projects.join("custom-skills");
    let custom_str = custom.to_string_lossy().into_owned();
    let (s, v) = settings_put(&e, json!({"globalPath": custom_str})).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["globalPath"], custom_str.as_str());

    let (s, v) = settings_get(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["globalPath"], custom_str.as_str());

    // Empty string resets to the default.
    let (s, v) = settings_put(&e, json!({"globalPath": ""})).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["globalPath"], default_str.as_str());

    let (s, v) = settings_get(&e).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["globalPath"], default_str.as_str());
}

#[tokio::test]
async fn install_global_without_project_ok() {
    let e = env(false).await;
    // No `project` field: global scope resolves its own root.
    let (s, v) = install(
        &e,
        json!({"scope": "global", "source": {"catalog": {"id": "seo"}}}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["id"], "seo");
    assert!(e
        .projects_dir()
        .join(".orso/global-skills/seo/SKILL.md")
        .exists());
}

#[tokio::test]
async fn save_global_stale_hash() {
    let e = env(false).await;
    install(
        &e,
        json!({"scope": "global", "source": {"catalog": {"id": "seo"}}}),
    )
    .await;
    let (s, v) = get(&e, "seo", "global").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let hash = v["hash"].as_str().unwrap().to_string();
    let path = e.projects_dir().join(".orso/global-skills/seo/SKILL.md");
    let before = std::fs::read_to_string(&path).unwrap();

    let (s, v) = save(
        &e,
        "seo",
        json!({"scope": "global", "body": "changed\n", "hash": hash}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let after = std::fs::read_to_string(&path).unwrap();
    assert_ne!(before, after);

    // The same (now stale) hash is refused and the file is left unchanged.
    let (s, v) = save(
        &e,
        "seo",
        json!({"scope": "global", "body": "again\n", "hash": hash}),
    )
    .await;
    assert_eq!(s, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"]["reason"], "stale_content");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), after);
}

#[tokio::test]
async fn install_copy_global_to_project() {
    let e = env(true).await;
    install(
        &e,
        json!({"scope": "global", "source": {"catalog": {"id": "seo"}}}),
    )
    .await;
    let b = branch(&e);
    let (s, v) = install(
        &e,
        json!({"project": e.id, "branch": b, "scope": "project", "source": {"copy": {"id": "seo", "fromScope": "global"}}}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["id"], "seo");
    assert_eq!(v["scope"], "project");
    assert!(e.dir.join(".orso/skills/seo/SKILL.md").exists());
    assert_eq!(
        git_cli(&e.dir, &["log", "-1", "--format=%s"]),
        "checkpoint: Installed skill"
    );
}

#[tokio::test]
async fn import_slugifies_id() {
    let e = env(false).await;
    let skill_md = "---\nname: My Custom Skill\ndescription: Does things\n---\n# Body\n";

    // An explicit id is honored.
    let (s, v) = install(
        &e,
        json!({"scope": "global", "source": {"import": {"skillMd": skill_md, "id": "custom"}}}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["id"], "custom");
    assert!(e
        .projects_dir()
        .join(".orso/global-skills/custom/SKILL.md")
        .exists());

    // An omitted id derives a slug from the front-matter name.
    let (s, v) = install(
        &e,
        json!({"scope": "global", "source": {"import": {"skillMd": skill_md}}}),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["id"], "my-custom-skill");
    assert!(e
        .projects_dir()
        .join(".orso/global-skills/my-custom-skill/SKILL.md")
        .exists());
}

#[tokio::test]
async fn unknown_catalog_404() {
    let e = env(false).await;
    let (s, v) = install(
        &e,
        json!({"scope": "global", "source": {"catalog": {"id": "nope"}}}),
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
    assert_eq!(v["error"]["code"], "ENOENT");
}

#[tokio::test]
async fn save_rejects_empty_name() {
    let e = env(false).await;
    install(
        &e,
        json!({"scope": "global", "source": {"catalog": {"id": "seo"}}}),
    )
    .await;
    let (s, v) = get(&e, "seo", "global").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    let hash = v["hash"].as_str().unwrap().to_string();
    let path = e.projects_dir().join(".orso/global-skills/seo/SKILL.md");
    let before = std::fs::read_to_string(&path).unwrap();

    let (s, v) = save(
        &e,
        "seo",
        json!({"scope": "global", "name": "", "body": "x\n", "hash": hash}),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["error"]["reason"], "invalid_skill");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
}

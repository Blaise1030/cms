use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use workbench::changes::conflict_sessions::ConflictSessions;
use workbench::changes::conflicts::Choice;
use workbench::config::Config;
use workbench::conflict_preview::ConflictPreviewManager;
use workbench::git::Identity;
use workbench::{http, state::AppState};

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn fixture(
    base: &[u8],
    yours: &[u8],
    latest: &[u8],
) -> (tempfile::TempDir, PathBuf, String, String) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("site");
    std::fs::create_dir(&dir).unwrap();
    git(&dir, &["init", "-b", "main"]);
    git(&dir, &["config", "user.name", "Ada"]);
    git(&dir, &["config", "user.email", "ada@example.com"]);
    std::fs::write(dir.join("page.md"), base).unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "base"]);
    git(&dir, &["checkout", "-b", "content/page"]);
    std::fs::write(dir.join("page.md"), yours).unwrap();
    git(&dir, &["commit", "-am", "mine"]);
    let tip = git(&dir, &["rev-parse", "HEAD"]);
    git(&dir, &["checkout", "main"]);
    std::fs::write(dir.join("page.md"), latest).unwrap();
    std::fs::write(dir.join("other.md"), "clean addition\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "latest"]);
    let incoming = git(&dir, &["rev-parse", "HEAD"]);
    git(&dir, &["checkout", "content/page"]);
    (tmp, dir, tip, incoming)
}

#[tokio::test]
async fn session_records_three_versions_without_touching_branch() {
    let (_tmp, dir, tip, incoming) = fixture(b"old\n", b"mine\n", b"latest\n");
    let sessions = ConflictSessions::new();
    let session = sessions
        .start(&dir, "site", "content/page", &incoming)
        .await
        .unwrap();
    assert_eq!(session.tip, tip);
    assert_eq!(session.incoming, incoming);
    assert_eq!(session.files.len(), 1);
    let file = &session.files[0];
    assert_eq!(file.path, "page.md");
    assert!(!file.binary);
    assert_eq!(git(&dir, &["cat-file", "blob", &file.ours_oid]), "mine");
    assert_eq!(git(&dir, &["cat-file", "blob", &file.theirs_oid]), "latest");
    assert_eq!(
        git(
            &dir,
            &["cat-file", "blob", file.base_oid.as_deref().unwrap()]
        ),
        "old"
    );
    assert_eq!(git(&dir, &["rev-parse", "content/page"]), tip);
    assert_eq!(std::fs::read(dir.join("page.md")).unwrap(), b"mine\n");
}

#[tokio::test]
async fn choice_keeps_clean_incoming_edits_and_finish_authors_merge() {
    let (_tmp, dir, tip, incoming) = fixture(b"old\n", b"mine\n", b"latest\n");
    let sessions = ConflictSessions::new();
    let session = sessions
        .start(&dir, "site", "content/page", &incoming)
        .await
        .unwrap();
    sessions
        .choose(&session.id, "page.md", Choice::Yours)
        .await
        .unwrap();
    let merged = sessions
        .finish(
            &session.id,
            &Identity {
                name: "Grace".into(),
                email: "grace@example.com".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(git(&dir, &["rev-parse", "content/page"]), merged);
    assert_eq!(
        git(&dir, &["rev-list", "--parents", "-n", "1", &merged]),
        format!("{merged} {tip} {incoming}")
    );
    assert_eq!(git(&dir, &["show", &format!("{merged}:page.md")]), "mine");
    assert_eq!(
        git(&dir, &["show", &format!("{merged}:other.md")]),
        "clean addition"
    );
    assert_eq!(
        git(&dir, &["show", "-s", "--format=%an <%ae>", &merged]),
        "Grace <grace@example.com>"
    );
}

#[tokio::test]
async fn both_combines_each_conflict_region_once_and_abandon_changes_nothing() {
    let base = b"start\nold a\nseparator 1\nseparator 2\nseparator 3\nseparator 4\nseparator 5\nold b\nend\n";
    let yours = b"start\nmine a\nseparator 1\nseparator 2\nseparator 3\nseparator 4\nseparator 5\nmine b\nend\n";
    let latest = b"start\nlatest a\nseparator 1\nseparator 2\nseparator 3\nseparator 4\nseparator 5\nlatest b\nend\n";
    let (_tmp, dir, tip, incoming) = fixture(base, yours, latest);
    let sessions = ConflictSessions::new();
    let session = sessions
        .start(&dir, "site", "content/page", &incoming)
        .await
        .unwrap();
    let candidate = sessions
        .choose(&session.id, "page.md", Choice::Both)
        .await
        .unwrap();
    let content = git(&dir, &["show", &format!("{}:page.md", candidate.tree)]);
    assert!(content.contains("mine a\nlatest a"), "{content}");
    assert!(content.contains("mine b\nlatest b"), "{content}");
    assert!(content.contains("separator"), "{content}");
    assert_eq!(
        git(&dir, &["show", &format!("{}:other.md", candidate.tree)]),
        "clean addition"
    );
    sessions.abandon(&session.id).await.unwrap();
    assert_eq!(git(&dir, &["rev-parse", "content/page"]), tip);
    assert_eq!(git(&dir, &["rev-parse", "main"]), incoming);
    assert!(git(&dir, &["status", "--porcelain"]).is_empty());
}

#[tokio::test]
async fn binary_conflict_rejects_both_and_stale_tip_rejects_finish() {
    let (_tmp, dir, _tip, incoming) = fixture(b"\0old", b"\0mine", b"\0latest");
    let sessions = ConflictSessions::new();
    let session = sessions
        .start(&dir, "site", "content/page", &incoming)
        .await
        .unwrap();
    assert!(session.files[0].binary);
    assert!(sessions
        .choose(&session.id, "page.md", Choice::Both)
        .await
        .is_err());
    sessions
        .choose(&session.id, "page.md", Choice::Latest)
        .await
        .unwrap();
    let candidate = sessions.get(&session.id).await.unwrap();
    assert_eq!(candidate.choices.get("page.md"), Some(&Choice::Latest));
    std::fs::write(dir.join("new.md"), "later\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-m", "later"]);
    let newer_tip = git(&dir, &["rev-parse", "HEAD"]);
    assert!(sessions
        .finish(&session.id, &Identity::workbench())
        .await
        .is_err());
    assert_eq!(git(&dir, &["rev-parse", "content/page"]), newer_tip);
}

#[tokio::test]
async fn binary_choice_preserves_exact_bytes() {
    let (_tmp, dir, _tip, incoming) = fixture(b"\0old", b"\0mine", b"\0latest");
    let sessions = ConflictSessions::new();
    let session = sessions
        .start(&dir, "site", "content/page", &incoming)
        .await
        .unwrap();
    sessions
        .choose(&session.id, "page.md", Choice::Latest)
        .await
        .unwrap();
    let commit = sessions
        .finish(&session.id, &Identity::workbench())
        .await
        .unwrap();
    let output = Command::new("git")
        .args(["show", &format!("{commit}:page.md")])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"\0latest");
}

#[cfg(unix)]
#[tokio::test]
async fn latest_choice_uses_latest_file_mode() {
    let (_tmp, dir, _tip, incoming) = fixture(b"old\n", b"mine\n", b"latest\n");
    git(&dir, &["update-index", "--chmod=+x", "page.md"]);
    git(&dir, &["commit", "-m", "make executable"]);
    let sessions = ConflictSessions::new();
    let session = sessions
        .start(&dir, "site", "content/page", &incoming)
        .await
        .unwrap();
    let candidate = sessions
        .choose(&session.id, "page.md", Choice::Latest)
        .await
        .unwrap();
    assert!(git(&dir, &["ls-tree", &candidate.tree, "page.md"]).starts_with("100644 "));
}

#[tokio::test]
async fn deletion_conflict_can_keep_deleted_or_modified_file() {
    let (_tmp, dir, _tip, incoming) = fixture(b"old\n", b"mine\n", b"latest\n");
    git(&dir, &["rm", "page.md"]);
    git(&dir, &["commit", "-m", "delete page"]);
    let sessions = ConflictSessions::new();
    let session = sessions
        .start(&dir, "site", "content/page", &incoming)
        .await
        .unwrap();
    assert!(session.files[0].ours_deleted);
    assert!(!session.files[0].theirs_deleted);
    assert!(sessions
        .choose(&session.id, "page.md", Choice::Both)
        .await
        .is_err());
    let deleted = sessions
        .choose(&session.id, "page.md", Choice::Yours)
        .await
        .unwrap();
    assert!(git(&dir, &["ls-tree", &deleted.tree, "page.md"]).is_empty());
    let modified = sessions
        .choose(&session.id, "page.md", Choice::Latest)
        .await
        .unwrap();
    assert!(git(&dir, &["show", &format!("{}:page.md", modified.tree)]).contains("latest"));
    let merged = sessions
        .finish(&session.id, &Identity::workbench())
        .await
        .unwrap();
    assert_eq!(git(&dir, &["show", &format!("{merged}:page.md")]), "latest");
}

#[tokio::test]
async fn sessions_on_different_branches_coexist() {
    let (_tmp, dir, tip, incoming) = fixture(b"old\n", b"mine\n", b"latest\n");
    git(&dir, &["branch", "content/other", &tip]);
    let sessions = ConflictSessions::new();
    let first = sessions
        .start(&dir, "site", "content/page", &incoming)
        .await
        .unwrap();
    let second = sessions
        .start(&dir, "site", "content/other", &incoming)
        .await
        .unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(sessions.list("site").await.len(), 2);
    assert!(sessions
        .start(&dir, "site", "content/page", &incoming)
        .await
        .is_err());
    assert_eq!(sessions.get(&first.id).await.unwrap().tip, tip);
}

#[tokio::test]
async fn finish_checks_result_build_even_when_preview_was_never_opened() {
    let (_tmp, dir, _tip, incoming) = fixture(b"old\n", b"mine\n", b"latest\n");
    std::fs::write(
        dir.join("package.json"),
        r#"{"scripts":{"build":"node -e \"throw new SyntaxError('broken page')\""}}"#,
    )
    .unwrap();
    git(&dir, &["add", "package.json"]);
    git(&dir, &["commit", "-m", "broken build"]);
    let tip = git(&dir, &["rev-parse", "HEAD"]);
    let sessions = ConflictSessions::new();
    let session = sessions
        .start(&dir, "site", "content/page", &incoming)
        .await
        .unwrap();
    sessions
        .choose(&session.id, "page.md", Choice::Yours)
        .await
        .unwrap();
    let previews = ConflictPreviewManager::new();
    let result = sessions
        .finish_validated(&session.id, &Identity::workbench(), &previews)
        .await;
    assert!(result.is_err(), "invalid result must prevent a merge");
    assert_eq!(git(&dir, &["rev-parse", "HEAD"]), tip);
    assert!(
        previews
            .is_invalid(&session.id, workbench::conflict_preview::Source::Result)
            .await
    );
    previews.remove_session(&session.id).await;
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "localhost");
    let body = if let Some(v) = body {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
        Body::from(v.to_string())
    } else {
        Body::empty()
    };
    let response = app
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn conflict_api_lists_choices_and_finishes_only_its_project() {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    std::fs::create_dir(&projects).unwrap();
    let state = AppState::with_scaffold_install(
        Config {
            port: 0,
            projects_dir: projects.clone(),
            no_auth: true,
            ..Default::default()
        },
        false,
    );
    let info = state.projects.create("Site").await.unwrap();
    let dir = projects.join(&info.id);
    git(&dir, &["config", "user.name", "Ada"]);
    git(&dir, &["config", "user.email", "ada@example.com"]);
    std::fs::write(dir.join("page.md"), "old\n").unwrap();
    git(&dir, &["add", "page.md"]);
    git(&dir, &["commit", "-m", "base"]);
    git(&dir, &["checkout", "-b", "content/page"]);
    std::fs::write(dir.join("page.md"), "mine\n").unwrap();
    git(&dir, &["commit", "-am", "mine"]);
    git(&dir, &["checkout", "main"]);
    std::fs::write(dir.join("page.md"), "latest\n").unwrap();
    git(&dir, &["commit", "-am", "latest"]);
    let incoming = git(&dir, &["rev-parse", "HEAD"]);
    git(&dir, &["checkout", "content/page"]);
    let session = state
        .conflicts
        .start(&dir, &info.id, "content/page", &incoming)
        .await
        .unwrap();
    let app = http::router(state);
    let (status, list) = call(
        &app,
        "GET",
        &format!("/api/changes/conflicts?project={}", info.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list["sessions"][0]["id"], session.id);
    let (status, _) = call(
        &app,
        "POST",
        "/api/changes/update",
        Some(json!({"project": info.id})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let detail_url = format!("/api/changes/conflicts/{}?project={}", session.id, info.id);
    let (status, detail) = call(&app, "GET", &detail_url, None).await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["files"][0]["path"], "page.md");
    assert!(detail["files"][0]["patch"]
        .as_str()
        .unwrap()
        .contains("+latest"));
    let (status, wrong) = call(
        &app,
        "GET",
        &format!("/api/changes/conflicts/{}?project=other", session.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{wrong}");
    let (status, invalid) = call(
        &app,
        "POST",
        &format!("/api/changes/conflicts/{}/choice", session.id),
        Some(json!({"project": info.id, "path": "../other.md", "choice": "yours"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid}");
    let (status, choice) = call(
        &app,
        "POST",
        &format!("/api/changes/conflicts/{}/choice", session.id),
        Some(json!({"project": info.id, "path": "page.md", "choice": "yours"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{choice}");
    assert_eq!(choice["complete"], true);
    let (status, reloaded) = call(&app, "GET", &detail_url, None).await;
    assert_eq!(status, StatusCode::OK, "{reloaded}");
    assert_eq!(reloaded["choices"]["page.md"], "yours");
    assert_eq!(reloaded["candidate"]["complete"], true);
    let (status, finished) = call(
        &app,
        "POST",
        &format!("/api/changes/conflicts/{}/finish", session.id),
        Some(json!({"project": info.id})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{finished}");
    assert_eq!(git(&dir, &["show", "HEAD:page.md"]), "mine");
}

use std::path::Path;
use std::process::Command;

use workbench::config::Config;
use workbench::state::AppState;
use workbench::{git, http};

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

const HERO: &str = "src/content/sections/hero.mdx";
const HERO2: &str = "src/content/sections/hero2.mdx";

fn append(dir: &Path, path: &str, text: &str) {
    let file = dir.join(path);
    let mut body = std::fs::read_to_string(&file).unwrap_or_default();
    body.push_str(text);
    std::fs::write(file, body).unwrap();
}

#[tokio::test]
async fn file_log_lists_commits_touching_path_newest_first() {
    let tmp = tempfile::tempdir().unwrap();
    let (_app, dir) = project(tmp.path()).await;
    git_cli(&dir, &["checkout", "-b", "content/x"]);
    append(&dir, HERO, "\nOne.\n");
    git::checkpoint(&dir, "one").await.unwrap().unwrap();
    append(&dir, "README.md", "\nOther.\n");
    git::checkpoint(&dir, "other").await.unwrap().unwrap();
    append(&dir, HERO, "\nTwo.\n");
    git::checkpoint(&dir, "two").await.unwrap().unwrap();

    let log = git::file_log(&dir, HERO).await.unwrap();
    assert_eq!(log[0].summary, "two");
    assert_eq!(log[1].summary, "one");
    assert!(log.iter().all(|v| v.summary != "other"));
    assert!(log.iter().all(|v| v.path == HERO));
}

#[tokio::test]
async fn file_log_marks_main_commits_published() {
    let tmp = tempfile::tempdir().unwrap();
    let (_app, dir) = project(tmp.path()).await;
    let scaffold = git_cli(&dir, &["rev-parse", "main"]);
    git_cli(&dir, &["checkout", "-b", "content/x"]);
    append(&dir, HERO, "\nDraft.\n");
    let draft = git::checkpoint(&dir, "draft").await.unwrap().unwrap();

    let log = git::file_log(&dir, HERO).await.unwrap();
    let find = |sha: &str| log.iter().find(|v| v.sha == sha).unwrap();
    assert!(!find(&draft.sha).published);
    assert!(find(&scaffold).published);
}

#[tokio::test]
async fn file_log_follows_renames_and_file_at_reads_old_path() {
    let tmp = tempfile::tempdir().unwrap();
    let (_app, dir) = project(tmp.path()).await;
    append(&dir, HERO, "\nBefore the move.\n");
    git::checkpoint(&dir, "before").await.unwrap().unwrap();
    git_cli(&dir, &["mv", HERO, HERO2]);
    git::checkpoint(&dir, "moved").await.unwrap().unwrap();

    let log = git::file_log(&dir, HERO2).await.unwrap();
    assert_eq!(log[0].summary, "moved");
    assert_eq!(log[0].path, HERO2);
    let old = log.iter().find(|v| v.summary == "before").unwrap();
    assert_eq!(old.path, HERO);
    assert!(git::file_at(&dir, &old.sha, &old.path)
        .await
        .unwrap()
        .contains("Before the move."));
}

#[tokio::test]
async fn file_log_is_empty_for_untracked_file() {
    let tmp = tempfile::tempdir().unwrap();
    let (_app, dir) = project(tmp.path()).await;
    std::fs::write(dir.join("src/content/sections/new.mdx"), "new\n").unwrap();
    assert_eq!(
        git::file_log(&dir, "src/content/sections/new.mdx")
            .await
            .unwrap(),
        vec![]
    );
}

#[tokio::test]
async fn file_at_rejects_non_hex_sha() {
    let tmp = tempfile::tempdir().unwrap();
    let (_app, dir) = project(tmp.path()).await;
    let err = git::file_at(&dir, "HEAD~1", HERO).await.unwrap_err();
    assert_eq!(err.http_status(), 400);
}

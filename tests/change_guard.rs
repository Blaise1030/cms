//! Slice 3: GitHub projects on `main` require a change before terminal edits / checkpoints.

use std::path::{Path, PathBuf};
use std::process::Command;

use workbench::{changes, git};

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

#[tokio::test]
async fn require_change_blocks_github_on_main_allows_local_and_branches() {
    let tmp = tempfile::tempdir().unwrap();
    let work = tmp.path().join("work");
    let bare = seed_bare(tmp.path());
    git_cli(tmp.path(), &["clone", bare.to_str().unwrap(), "work"]);
    git_cli(&work, &["config", "user.name", "Ada"]);
    git_cli(&work, &["config", "user.email", "ada@example.com"]);

    let err = changes::require_change(&work, Some("acme/site"))
        .await
        .unwrap_err();
    assert_eq!(err.code.as_str(), "CHANGE_REQUIRED");

    changes::require_change(&work, None).await.unwrap();

    changes::create(&work, "New post: Launch week")
        .await
        .unwrap();
    changes::require_change(&work, Some("acme/site"))
        .await
        .unwrap();
}

#[tokio::test]
async fn checkpoint_guard_blocks_main_for_github_via_require_change() {
    // Hook turn-end checkpoints call the same guard; assert the code it would see.
    let tmp = tempfile::tempdir().unwrap();
    let bare = seed_bare(tmp.path());
    let work = tmp.path().join("work");
    git_cli(tmp.path(), &["clone", bare.to_str().unwrap(), "work"]);
    git_cli(&work, &["config", "user.name", "Ada"]);
    git_cli(&work, &["config", "user.email", "ada@example.com"]);
    std::fs::write(work.join("notes.txt"), "dirty\n").unwrap();

    let err = changes::require_change(&work, Some("acme/site"))
        .await
        .unwrap_err();
    assert_eq!(err.code.as_str(), "CHANGE_REQUIRED");

    // Local-only: checkpoint itself may proceed (guard allows).
    changes::require_change(&work, None).await.unwrap();
    let cp = git::checkpoint(&work, "local edit").await.unwrap();
    assert!(cp.is_some());
}

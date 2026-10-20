use std::{
    path::{Path, PathBuf},
    process::Command,
};
use orso::git;

fn cli(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .trim_end()
        .to_string()
}
fn repo() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    cli(tmp.path(), &["init", "-q"]);
    cli(tmp.path(), &["config", "user.name", "Ada"]);
    cli(tmp.path(), &["config", "user.email", "ada@example.com"]);
    for name in ["content.mdx", "other.txt", "unstaged.txt"] {
        std::fs::write(tmp.path().join(name), "original\n").unwrap();
    }
    cli(tmp.path(), &["add", "."]);
    cli(
        tmp.path(),
        &["-c", "commit.gpgsign=false", "commit", "-qm", "initial"],
    );
    tmp
}

#[tokio::test]
async fn commits_only_selected_content_preserving_unrelated_index_and_worktree() {
    let tmp = repo();
    let dir = tmp.path();
    std::fs::write(dir.join("other.txt"), "staged\n").unwrap();
    cli(dir, &["add", "other.txt"]);
    std::fs::write(dir.join("other.txt"), "unstaged after staging\n").unwrap();
    std::fs::write(dir.join("unstaged.txt"), "only unstaged\n").unwrap();
    let staged = cli(dir, &["diff", "--cached", "--binary"]);
    let unstaged = cli(dir, &["diff", "--binary"]);
    std::fs::write(dir.join("content.mdx"), "updated\n").unwrap();
    let cp = git::checkpoint_paths(dir, "  Update content\nignored", &["content.mdx".into()])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cp.summary, "Update content");
    assert_eq!(cp.author, "Ada");
    assert!(cp.ts > 0);
    assert_eq!(cp.sha, cli(dir, &["rev-parse", "HEAD"]));
    assert_eq!(
        cli(dir, &["show", "--format=", "--name-only", "HEAD"]),
        "content.mdx"
    );
    assert_eq!(cli(dir, &["diff", "--cached", "--binary"]), staged);
    assert_eq!(cli(dir, &["diff", "--binary"]), unstaged);
    assert_eq!(cli(dir, &["show", ":other.txt"]), "staged");
    assert_eq!(
        std::fs::read_to_string(dir.join("other.txt")).unwrap(),
        "unstaged after staging\n"
    );
}

#[tokio::test]
async fn creates_and_deletes_selected_files_with_literal_names() {
    let tmp = repo();
    let dir = tmp.path();
    std::fs::remove_file(dir.join("content.mdx")).unwrap();
    std::fs::write(dir.join("[new].mdx"), "new\n").unwrap();
    std::fs::write(dir.join("n.mdx"), "unrelated\n").unwrap();
    git::checkpoint_paths(dir, "Files", &["content.mdx".into(), "[new].mdx".into()])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cli(dir, &["show", "HEAD:[new].mdx"]), "new");
    assert!(!cli(dir, &["ls-tree", "--name-only", "HEAD"]).contains("content.mdx"));
    assert_eq!(cli(dir, &["status", "--porcelain"]), "?? n.mdx");
}

#[tokio::test]
async fn unchanged_or_empty_selection_does_not_commit_or_touch_index() {
    let tmp = repo();
    let dir = tmp.path();
    std::fs::write(dir.join("other.txt"), "staged\n").unwrap();
    cli(dir, &["add", "other.txt"]);
    let head = cli(dir, &["rev-parse", "HEAD"]);
    let index = std::fs::read(dir.join(".git/index")).unwrap();
    for paths in [
        vec![],
        vec![PathBuf::from("content.mdx")],
        vec![PathBuf::from("missing.mdx")],
    ] {
        assert_eq!(
            git::checkpoint_paths(dir, "noop", &paths).await.unwrap(),
            None
        );
    }
    assert_eq!(cli(dir, &["rev-parse", "HEAD"]), head);
    assert_eq!(std::fs::read(dir.join(".git/index")).unwrap(), index);
}

#[tokio::test]
async fn rejects_escaping_metadata_and_directory_paths_before_mutation() {
    let tmp = repo();
    let dir = tmp.path();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), "secret").unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.join("escape")).unwrap();
    let head = cli(dir, &["rev-parse", "HEAD"]);
    let index = std::fs::read(dir.join(".git/index")).unwrap();
    for path in [
        "../secret",
        "/etc/passwd",
        ".git/config",
        "escape/secret",
        ".",
        "",
    ] {
        assert!(
            git::checkpoint_paths(dir, "bad", &[path.into()])
                .await
                .is_err(),
            "{path}"
        );
    }
    assert_eq!(cli(dir, &["rev-parse", "HEAD"]), head);
    assert_eq!(std::fs::read(dir.join(".git/index")).unwrap(), index);
}

async fn reject_directory_selection(replace_with_file: bool, committed: bool) {
    let tmp = repo();
    let dir = tmp.path();
    std::fs::create_dir(dir.join("content")).unwrap();
    for name in ["a.mdx", "b.mdx"] {
        std::fs::write(dir.join("content").join(name), "original\n").unwrap();
    }
    cli(dir, &["add", "content"]);
    if committed {
        cli(
            dir,
            &["-c", "commit.gpgsign=false", "commit", "-qm", "content"],
        );
    }
    std::fs::write(dir.join("content/b.mdx"), "staged unrelated\n").unwrap();
    cli(dir, &["add", "content/b.mdx"]);
    std::fs::remove_dir_all(dir.join("content")).unwrap();
    if replace_with_file {
        std::fs::write(dir.join("content"), "replacement\n").unwrap();
    }
    let head = cli(dir, &["rev-parse", "HEAD"]);
    let index = std::fs::read(dir.join(".git/index")).unwrap();
    let result = git::checkpoint_paths(dir, "Must not expand selection", &["content".into()]).await;
    assert!(
        result.is_err(),
        "directory selection must be rejected: {result:?}"
    );
    assert_eq!(cli(dir, &["rev-parse", "HEAD"]), head);
    assert_eq!(std::fs::read(dir.join(".git/index")).unwrap(), index);
    if replace_with_file {
        assert_eq!(
            std::fs::read_to_string(dir.join("content")).unwrap(),
            "replacement\n"
        );
    } else {
        assert!(!dir.join("content").exists());
    }
}

#[tokio::test]
async fn rejects_removed_directory_from_head() {
    reject_directory_selection(false, true).await;
}
#[tokio::test]
async fn rejects_removed_directory_from_index() {
    reject_directory_selection(false, false).await;
}
#[tokio::test]
async fn rejects_directory_replaced_with_file_from_head() {
    reject_directory_selection(true, true).await;
}
#[tokio::test]
async fn rejects_directory_replaced_with_file_from_index() {
    reject_directory_selection(true, false).await;
}

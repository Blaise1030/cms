use std::fs;
use std::path::Path;
use std::process::Command;

use axum::body::to_bytes;
use orso::conflict_preview::{ConflictPreviewManager, PreviewState, Source};

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
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn commit(dir: &Path, text: &str) -> String {
    fs::write(dir.join("index.html"), text).unwrap();
    git(dir, &["add", "."]);
    git(
        dir,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=t@example.com",
            "commit",
            "-m",
            "page",
        ],
    );
    git(dir, &["rev-parse", "HEAD"])
}

#[tokio::test]
async fn distinct_snapshots_assets_invalid_build_and_cleanup() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("site");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    fs::write(repo.join("package.json"), r#"{"scripts":{"build":"mkdir -p dist/assets && cp index.html dist/index.html && cp site.css dist/assets/site.css"}}"#).unwrap();
    fs::write(repo.join("site.css"), "body { color: red }\n").unwrap();
    let yours = commit(&repo, "<h1>Yours</h1>\n");
    let latest = commit(&repo, "<h1>Latest</h1>\n");
    let result = commit(&repo, "<h1>Result</h1>\n");
    fs::create_dir(repo.join("node_modules")).unwrap();
    let manager = ConflictPreviewManager::new();
    for (source, rev, expected) in [
        (Source::Yours, &yours, "Yours"),
        (Source::Latest, &latest, "Latest"),
        (Source::Result, &result, "Result"),
    ] {
        let state = manager.prepare(&repo, "site", "session", source, rev).await;
        assert!(matches!(state, PreviewState::Ready { .. }), "{state:?}");
        let response = manager
            .serve("session", source, "index.html", None)
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(
            String::from_utf8_lossy(&body),
            format!("<h1>{expected}</h1>\n")
        );
    }
    let css = manager
        .serve("session", Source::Latest, "assets/site.css", None)
        .await
        .unwrap();
    assert_eq!(css.headers()["content-type"], "text/css; charset=utf-8");
    let body = to_bytes(css.into_body(), 1024).await.unwrap();
    assert_eq!(&body[..], b"body { color: red }\n");
    let highlighted = manager
        .serve("session", Source::Yours, "index.html", Some("hero"))
        .await
        .unwrap();
    let highlighted = to_bytes(highlighted.into_body(), 4096).await.unwrap();
    assert!(!String::from_utf8_lossy(&highlighted).contains("data-conflict-highlight"));
    let unsafe_highlight = manager
        .serve(
            "session",
            Source::Yours,
            "index.html",
            Some("hero');alert(1)//"),
        )
        .await
        .unwrap();
    let unsafe_highlight = to_bytes(unsafe_highlight.into_body(), 4096).await.unwrap();
    assert!(!String::from_utf8_lossy(&unsafe_highlight).contains("alert(1)"));
    assert!(manager
        .serve("session", Source::Result, "../secret", None)
        .await
        .is_err());
    assert_eq!(
        fs::read_to_string(repo.join("index.html")).unwrap(),
        "<h1>Result</h1>\n"
    );

    fs::write(
        repo.join("package.json"),
        r#"{"scripts":{"build":"node -e \"throw new SyntaxError('broken page')\""}}"#,
    )
    .unwrap();
    let broken = commit(&repo, "<h1>Broken</h1>\n");
    let state = manager
        .prepare(&repo, "site", "session", Source::Result, &broken)
        .await;
    assert!(matches!(state, PreviewState::Invalid { .. }), "{state:?}");
    assert!(manager.is_invalid("session", Source::Result).await);
    fs::write(
        repo.join("package.json"),
        r#"{"scripts":{"build":"mkdir -p dist"}}"#,
    )
    .unwrap();
    let no_page = commit(&repo, "<h1>No build output</h1>\n");
    let state = manager
        .prepare(&repo, "site", "session", Source::Result, &no_page)
        .await;
    assert!(
        matches!(state, PreviewState::Unavailable { .. }),
        "{state:?}"
    );
    manager.remove_session("session").await;
    assert!(!manager.is_invalid("session", Source::Result).await);
    assert!(manager
        .serve("session", Source::Yours, "", None)
        .await
        .is_err());
}

#[tokio::test]
async fn missing_dependencies_keep_text_flow_available() {
    let temp = tempfile::tempdir().unwrap();
    let manager = ConflictPreviewManager::new();
    let state = manager
        .prepare(temp.path(), "site", "s", Source::Yours, "abc")
        .await;
    assert!(matches!(state, PreviewState::Unavailable { .. }));
}

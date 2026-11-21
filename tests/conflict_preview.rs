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
        let state = manager.prepare(&repo, "session", source, rev).await;
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
        .prepare(&repo, "session", Source::Result, &broken)
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
        .prepare(&repo, "session", Source::Result, &no_page)
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
        .prepare(temp.path(), "s", Source::Yours, "abc")
        .await;
    assert!(matches!(state, PreviewState::Unavailable { .. }));
}

/// One request to a snapshot origin: `(status, head, body)`.
async fn get(port: u16, path: &str, extra: &str) -> (u16, String, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    s.write_all(
        format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n{extra}\r\n")
            .as_bytes(),
    )
    .await
    .unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    let (head, body) = out.split_once("\r\n\r\n").unwrap();
    let status = head.split_whitespace().nth(1).unwrap().parse().unwrap();
    (status, head.to_string(), body.to_string())
}

#[tokio::test]
async fn each_snapshot_is_served_at_the_root_of_its_own_origin() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("site");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    fs::write(
        repo.join("package.json"),
        r#"{"scripts":{"build":"mkdir -p dist && cp index.html dist/index.html"}}"#,
    )
    .unwrap();
    let rev = commit(
        &repo,
        "<html><head></head><body><section id=\"hero\"><a href=\"/pricing\">Hi</a></section></body></html>",
    );
    fs::create_dir(repo.join("node_modules")).unwrap();
    let config = orso::config::Config {
        port: 7800,
        token: Some("daemon-token".into()),
        ..Default::default()
    };
    let manager = ConflictPreviewManager::new().with_config(std::sync::Arc::new(config));
    let state = manager.prepare(&repo, "s", Source::Yours, &rev).await;
    let PreviewState::Ready { port, token } = state else {
        panic!("{state:?}")
    };
    let token = token.expect("an origin token under auth");

    assert_eq!(
        get(port, "/", "").await.0,
        401,
        "the origin needs its token"
    );
    let (status, head, _) = get(port, &format!("/?section=hero&t={token}"), "").await;
    assert_eq!(status, 302);
    assert!(head.contains("location: /?section=hero"), "{head}");
    let cookie = format!("Cookie: orso_preview_{port}={token}\r\n");

    let (status, _, body) = get(port, "/?section=hero", &cookie).await;
    assert_eq!(status, 200);
    assert!(
        body.contains("<a href=\"/pricing\">"),
        "links stay plain: {body}"
    );
    assert!(body.contains("data-conflict-highlight"), "{body}");
    // The page tells the app (and only the app's origins) that it exists.
    assert!(
        body.contains(r#"type:"orso:snapshot",found:true"#),
        "{body}"
    );
    assert!(body.contains("\"http://127.0.0.1:7800\""), "{body}");
    assert!(!body.contains("\"*\""), "{body}");

    let (status, _, body) = get(port, "/missing/", &cookie).await;
    assert_eq!(status, 404);
    assert!(body.contains("found:false"), "{body}");
    assert_eq!(get(port, "/missing.css", &cookie).await.0, 404);

    manager.remove_session("s").await;
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_err(),
        "the snapshot's port is freed with it"
    );
}

use std::path::Path;
use orso::content::catalog::{list_collections, list_entries, read_entry};

fn template() -> &'static Path {
    Path::new("templates/astro-marketing")
}
#[tokio::test]
async fn template_catalog_and_raw_entries() {
    let collections = list_collections(template()).await.unwrap();
    assert_eq!(
        collections
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["blog", "helpDocs"]
    );
    assert!(collections.iter().all(|c| c.editable));
    assert_eq!(
        serde_json::to_value(&collections).unwrap()[0]["fields"]["properties"]["pubDate"]["type"],
        "date"
    );
    for (collection, id, path) in [
        ("blog", "welcome", "src/content/blog/en/welcome.md"),
        (
            "helpDocs",
            "getting-started",
            "src/content/helpDocs/en/getting-started.md",
        ),
    ] {
        let entries = list_entries(template(), collection).await.unwrap();
        assert!(entries.iter().any(|e| e.id == id && e.path == path));
        let entry = read_entry(template(), collection, id).await.unwrap();
        assert_eq!(
            entry.source,
            std::fs::read_to_string(template().join(path)).unwrap()
        );
        assert_eq!(entry.hash.len(), 64);
        assert!(entry.fields.is_object());
        assert!(!entry.id.starts_with("en/"));
    }
    assert!(read_entry(template(), "blog", "../site").await.is_err());
}
fn copy_template(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if matches!(name.to_str(), Some("node_modules" | "dist" | ".git")) {
            continue;
        }
        let to = dst.join(&name);
        if entry.file_type().unwrap().is_dir() {
            copy_template(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), &to).unwrap();
        }
    }
}

#[tokio::test]
async fn catalog_under_subfolder_root() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let site = root.join("site");
    copy_template(template(), &site);
    std::os::unix::fs::symlink(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/astro-marketing/node_modules"),
        site.join("node_modules"),
    )
    .unwrap();
    std::fs::write(root.join("orso.json"), r#"{"root":"site"}"#).unwrap();
    std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(root)
        .output()
        .unwrap();

    let collections = list_collections(root).await.unwrap();
    assert!(collections.iter().all(|c| c.editable), "{collections:?}");
    let entries = list_entries(root, "blog").await.unwrap();
    assert!(!entries.is_empty());
    assert!(
        entries
            .iter()
            .all(|e| e.path.starts_with("site/src/content/blog/en/")),
        "{entries:?}"
    );
    let entry = read_entry(root, "blog", "welcome").await.unwrap();
    assert_eq!(entry.path, "site/src/content/blog/en/welcome.md");
    assert_eq!(
        entry.source,
        std::fs::read_to_string(site.join("src/content/blog/en/welcome.md")).unwrap()
    );
}

#[tokio::test]
async fn helper_failure_is_read_only_and_isolated() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("src/content/blog/en")).unwrap();
    std::fs::write(
        temp.path().join("src/content/blog/en/other.md"),
        "---\ntitle: Other\n---\nBody\n",
    )
    .unwrap();
    let collections = list_collections(temp.path()).await.unwrap();
    assert!(collections
        .iter()
        .all(|c| !c.editable && c.reason.as_deref().unwrap().contains("helper")));
    assert_eq!(
        list_entries(temp.path(), "blog").await.unwrap()[0].id,
        "other"
    );
    assert!(read_entry(temp.path(), "blog", "welcome").await.is_err());
}
#[tokio::test]
async fn unsupported_loader_is_visible_with_reason() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("scripts")).unwrap();
    std::fs::write(temp.path().join("scripts/orso-content.mjs"), "console.log(JSON.stringify({collections:[{name:'remote',loader:{type:'remote'},kind:'json'}]}))").unwrap();
    let collections = list_collections(temp.path()).await.unwrap();
    assert_eq!(collections[0].name, "remote");
    assert!(!collections[0].editable);
    assert!(collections[0]
        .reason
        .as_deref()
        .unwrap()
        .contains("Unsupported"));
    assert!(list_entries(temp.path(), "remote")
        .await
        .unwrap()
        .is_empty());
}
#[tokio::test]
async fn raw_read_keeps_crlf_body_and_hashes_exact_bytes() {
    use sha2::{Digest, Sha256};
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("src/content/blog/en")).unwrap();
    let source = "---\r\ntitle: 'Quoted' # comment\r\ntags: [one, two]\r\n---\r\n\r\nBody\r\n";
    std::fs::write(temp.path().join("src/content/blog/en/post.md"), source).unwrap();
    std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(temp.path())
        .output()
        .unwrap();
    let entry = read_entry(temp.path(), "blog", "post").await.unwrap();
    assert_eq!(entry.branch, "main");
    assert_eq!(entry.fields["title"], "Quoted");
    assert_eq!(entry.fields["tags"], serde_json::json!(["one", "two"]));
    assert_eq!(entry.body, "\r\nBody\r\n");
    assert_eq!(entry.source, source);
    assert_eq!(
        entry.hash,
        format!("{:x}", Sha256::digest(source.as_bytes()))
    );
}
#[tokio::test]
async fn escaping_symlinks_are_not_readable() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.md"), "secret").unwrap();
    std::fs::create_dir_all(temp.path().join("src/content/blog/en")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.md"),
        temp.path().join("src/content/blog/en/secret.md"),
    )
    .unwrap();
    assert!(list_entries(temp.path(), "blog").await.unwrap().is_empty());
    assert!(read_entry(temp.path(), "blog", "secret").await.is_err());
}
#[tokio::test]
async fn content_routes_require_project_and_share_auth_boundary() {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    let temp = tempfile::tempdir().unwrap();
    let state = orso::state::AppState::new(orso::config::Config {
        projects_dir: temp.path().into(),
        no_auth: true,
        ..Default::default()
    });
    let app = orso::http::router(state);
    for endpoint in ["collections", "entries", "entry"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/content/{endpoint}?project=unknown"))
                    .header("Host", "localhost:7800")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 404);
    }
    let state = orso::state::AppState::new(orso::config::Config {
        projects_dir: temp.path().into(),
        token: Some("token".into()),
        no_auth: false,
        ..Default::default()
    });
    let response = orso::http::router(state)
        .oneshot(
            Request::builder()
                .uri("/api/content/collections?project=p")
                .header("Host", "localhost:7800")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}
#[tokio::test]
async fn unknown_helper_collection_is_read_only_even_with_a_supported_glob() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("scripts")).unwrap();
    std::fs::create_dir_all(temp.path().join("posts")).unwrap();
    std::fs::write(
        temp.path().join("posts/custom.md"),
        "---\ntitle: Custom\n---\nBody",
    )
    .unwrap();
    std::fs::write(temp.path().join("scripts/orso-content.mjs"),"console.log(JSON.stringify({collections:[{name:'custom',loader:{base:'posts',pattern:'*.md'},kind:'markdown'}]}))").unwrap();
    let collections = list_collections(temp.path()).await.unwrap();
    assert!(!collections[0].editable);
    assert!(collections[0]
        .reason
        .as_deref()
        .unwrap()
        .contains("Unsupported"));
    assert_eq!(
        read_entry(temp.path(), "custom", "custom")
            .await
            .unwrap()
            .body,
        "Body"
    );
}

#[tokio::test]
async fn required_unsupported_schema_is_read_only() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("scripts")).unwrap();
    std::fs::write(temp.path().join("scripts/orso-content.mjs"), r#"console.log(JSON.stringify({collections:[{name:'blog',loader:{base:'posts',pattern:'*.md'},kind:'markdown',supportedForEditing:true,fields:{type:'object',properties:{special:{type:'unsupported',required:true,reason:'Unknown transform'}}}}]}))"#).unwrap();
    let collections = list_collections(temp.path()).await.unwrap();
    assert!(!collections[0].editable);
    assert_eq!(
        collections[0].fields["properties"]["special"]["type"],
        "unsupported"
    );
    assert!(collections[0].reason.as_deref().unwrap().contains("schema"));
}

/// A fake helper that logs each run and reports one collection named `name`.
fn counting_helper(root: &Path, name: &str) {
    std::fs::create_dir_all(root.join("scripts")).unwrap();
    std::fs::write(
        root.join("scripts/orso-content.mjs"),
        format!("import {{appendFileSync}} from 'node:fs'; appendFileSync('runs.log','run\\n'); console.log(JSON.stringify({{collections:[{{name:'{name}',loader:{{base:'posts',pattern:'*.md'}},kind:'markdown'}}]}}))"),
    )
    .unwrap();
}
fn runs(root: &Path) -> usize {
    std::fs::read_to_string(root.join("runs.log"))
        .map(|log| log.lines().count())
        .unwrap_or(0)
}
fn set_mtime(path: &Path, time: std::time::SystemTime) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}
#[tokio::test]
async fn collections_use_one_helper_run_and_are_cached_until_schema_inputs_change() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let script = root.join("scripts/orso-content.mjs");
    let schemas = root.join("src/content-schemas.mjs");
    let config = root.join("src/content.config.ts");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(&schemas, "export {}").unwrap();
    counting_helper(root, "first");
    let written = std::fs::metadata(&script).unwrap().modified().unwrap();

    assert_eq!(list_collections(root).await.unwrap()[0].name, "first");
    assert_eq!(runs(root), 1, "list and describe share one helper run");
    // Cache hit: same mtimes, so a rewritten helper is not consulted.
    counting_helper(root, "second");
    set_mtime(&script, written);
    assert_eq!(list_collections(root).await.unwrap()[0].name, "first");
    assert_eq!(runs(root), 1);

    // Each schema input invalidates the cache when its mtime changes.
    let later = written + std::time::Duration::from_secs(5);
    set_mtime(&schemas, later);
    assert_eq!(list_collections(root).await.unwrap()[0].name, "second");
    assert_eq!(runs(root), 2);
    std::fs::write(&config, "export const collections = {}").unwrap();
    assert_eq!(list_collections(root).await.unwrap()[0].name, "second");
    assert_eq!(runs(root), 3, "a new content.config.ts invalidates");
    counting_helper(root, "third");
    set_mtime(&script, later + std::time::Duration::from_secs(5));
    assert_eq!(list_collections(root).await.unwrap()[0].name, "third");
    assert_eq!(runs(root), 4);
    assert_eq!(list_collections(root).await.unwrap()[0].name, "third");
    assert_eq!(runs(root), 4);
}

use std::path::Path;
use workbench::content::catalog::{list_collections, list_entries, read_entry};

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
        ["site", "pages", "sections", "blog", "helpDocs"]
    );
    assert!(collections.iter().all(|c| c.editable));
    assert_eq!(
        serde_json::to_value(&collections).unwrap()[3]["fields"]["properties"]["pubDate"]["type"],
        "date"
    );
    for (collection, id, path) in [
        ("site", "site", "src/content/site.json"),
        ("pages", "home", "src/content/pages/home.json"),
        ("sections", "hero", "src/content/sections/hero.mdx"),
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
    std::fs::write(temp.path().join("scripts/workbench-content.mjs"), "console.log(JSON.stringify({collections:[{name:'remote',loader:{type:'remote'},kind:'json'}]}))").unwrap();
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
    let state = workbench::state::AppState::new(workbench::config::Config {
        projects_dir: temp.path().into(),
        no_auth: true,
        ..Default::default()
    });
    let app = workbench::http::router(state);
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
    let state = workbench::state::AppState::new(workbench::config::Config {
        projects_dir: temp.path().into(),
        token: Some("token".into()),
        no_auth: false,
        ..Default::default()
    });
    let response = workbench::http::router(state)
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
    std::fs::write(temp.path().join("scripts/workbench-content.mjs"),"console.log(JSON.stringify({collections:[{name:'custom',loader:{base:'posts',pattern:'*.md'},kind:'markdown'}]}))").unwrap();
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
    std::fs::write(temp.path().join("scripts/workbench-content.mjs"), r#"console.log(JSON.stringify({collections:[{name:'blog',loader:{base:'posts',pattern:'*.md'},kind:'markdown',supportedForEditing:true,fields:{type:'object',properties:{special:{type:'unsupported',required:true,reason:'Unknown transform'}}}}]}))"#).unwrap();
    let collections = list_collections(temp.path()).await.unwrap();
    assert!(!collections[0].editable);
    assert_eq!(
        collections[0].fields["properties"]["special"]["type"],
        "unsupported"
    );
    assert!(collections[0].reason.as_deref().unwrap().contains("schema"));
}

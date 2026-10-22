mod common;

use orso::content::catalog::{list_collections, list_entries, read_entry};
use serde_json::{json, Value};
use std::path::Path;

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}
/// Writes `<site>/.astro/orso-content.json` with the given collections.
fn manifest(site: &Path, collections: Vec<Value>) {
    write(
        &site.join(".astro/orso-content.json"),
        &json!({ "version": 1, "collections": collections }).to_string(),
    );
}
fn git_init(root: &Path) {
    std::process::Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(root)
        .output()
        .unwrap();
}
fn blog_fields() -> Value {
    json!({
        "type": "object",
        "properties": {
            "title": { "type": "string", "required": true },
            "pubDate": { "type": "date", "required": true },
            "coverImage": { "type": "image", "required": false },
        },
    })
}

#[tokio::test]
async fn manifest_catalog_and_raw_entries() {
    use sha2::{Digest, Sha256};
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    manifest(
        root,
        vec![
            common::manifest_collection(
                "blog",
                "src/content/blog/en",
                "*.md",
                "markdown",
                blog_fields(),
            ),
            common::manifest_collection(
                "helpDocs",
                "src/content/helpDocs/en",
                "*.md",
                "markdown",
                json!({ "type": "object", "properties": {} }),
            ),
        ],
    );
    let source = "---\r\ntitle: 'Quoted' # comment\r\ntags: [one, two]\r\n---\r\n\r\nBody\r\n";
    write(&root.join("src/content/blog/en/post.md"), source);
    write(
        &root.join("src/content/helpDocs/en/getting-started.md"),
        "---\ntitle: Start\n---\nBody\n",
    );
    git_init(root);

    let collections = list_collections(root).await.unwrap();
    assert_eq!(
        collections
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["blog", "helpDocs"]
    );
    assert!(collections.iter().all(|c| c.editable && c.reason.is_none()));
    assert_eq!(collections[0].loader["base"], "src/content/blog/en");
    let json = serde_json::to_value(&collections).unwrap();
    assert_eq!(json[0]["supportedForEditing"], true);
    assert_eq!(
        json[0]["fields"]["properties"]["pubDate"],
        json!({ "type": "date", "required": true })
    );
    assert_eq!(
        json[0]["fields"]["properties"]["coverImage"],
        json!({ "type": "image", "required": false })
    );

    let entries = list_entries(root, "blog").await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "src/content/blog/en/post.md");
    let entry = read_entry(root, "blog", "post").await.unwrap();
    assert_eq!(entry.branch, "main");
    assert_eq!(entry.fields["title"], "Quoted");
    assert_eq!(entry.fields["tags"], json!(["one", "two"]));
    assert_eq!(entry.body, "\r\nBody\r\n");
    assert_eq!(entry.source, source);
    assert_eq!(
        entry.hash,
        format!("{:x}", Sha256::digest(source.as_bytes()))
    );
    let help = read_entry(root, "helpDocs", "getting-started")
        .await
        .unwrap();
    assert_eq!(help.path, "src/content/helpDocs/en/getting-started.md");
    assert!(read_entry(root, "blog", "../site").await.is_err());
}

#[tokio::test]
async fn catalog_under_subfolder_root() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let site = root.join("site");
    write(&site.join("astro.config.mjs"), "export default {}");
    write(&root.join("orso.json"), r#"{"root":"site"}"#);
    common::write_content_manifest(root, "site/");
    let source = "---\ntitle: Welcome\n---\nBody\n";
    write(&site.join("src/content/blog/en/welcome.md"), source);
    git_init(root);

    let collections = list_collections(root).await.unwrap();
    assert!(collections.iter().all(|c| c.editable), "{collections:?}");
    assert_eq!(collections[0].loader["base"], "site/src/content/blog/en");
    assert_eq!(
        collections[1].loader["base"],
        "site/src/content/helpDocs/en"
    );
    let entries = list_entries(root, "blog").await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, "site/src/content/blog/en/welcome.md");
    let entry = read_entry(root, "blog", "welcome").await.unwrap();
    assert_eq!(entry.path, "site/src/content/blog/en/welcome.md");
    assert_eq!(entry.source, source);
}

#[tokio::test]
async fn missing_manifest_lists_nothing() {
    let temp = tempfile::tempdir().unwrap();
    write(
        &temp.path().join("src/content/blog/en/other.md"),
        "---\ntitle: Other\n---\nBody\n",
    );
    assert!(list_collections(temp.path()).await.unwrap().is_empty());
    assert!(list_entries(temp.path(), "blog").await.is_err());
    assert!(read_entry(temp.path(), "blog", "other").await.is_err());
}

#[tokio::test]
async fn invalid_manifests_list_nothing() {
    let blog = common::manifest_collection(
        "blog",
        "src/content/blog/en",
        "*.md",
        "markdown",
        blog_fields(),
    );
    for contents in [
        "{not json".to_string(),
        json!({ "version": 2, "collections": [blog] }).to_string(),
        json!({ "collections": [blog] }).to_string(),
        json!({ "version": 1 }).to_string(),
    ] {
        let temp = tempfile::tempdir().unwrap();
        write(&temp.path().join(".astro/orso-content.json"), &contents);
        assert!(
            list_collections(temp.path()).await.unwrap().is_empty(),
            "{contents}"
        );
    }
}

#[tokio::test]
async fn unsupported_loader_is_visible_with_its_reason() {
    let temp = tempfile::tempdir().unwrap();
    let mut remote = common::manifest_collection("remote", "", "", "json", Value::Null);
    remote["loader"] = Value::Null;
    remote["supportedForEditing"] = json!(false);
    remote["reason"] = json!("Collection does not use orsoGlob");
    manifest(temp.path(), vec![remote]);

    let collections = list_collections(temp.path()).await.unwrap();
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].name, "remote");
    assert!(!collections[0].editable);
    assert!(!collections[0].supported_for_editing);
    assert_eq!(
        collections[0].reason.as_deref(),
        Some("Collection does not use orsoGlob")
    );
    assert!(list_entries(temp.path(), "remote")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn unsupported_pattern_without_reason_gets_the_default_reason() {
    let temp = tempfile::tempdir().unwrap();
    write(
        &temp.path().join("posts/custom.txt"),
        "---\ntitle: Custom\n---\nBody",
    );
    manifest(
        temp.path(),
        vec![common::manifest_collection(
            "custom",
            "posts",
            "*.txt",
            "markdown",
            json!({ "type": "object", "properties": {} }),
        )],
    );
    let collections = list_collections(temp.path()).await.unwrap();
    assert!(!collections[0].editable);
    assert_eq!(
        collections[0].reason.as_deref(),
        Some("Unsupported collection loader or schema; content is read-only")
    );
}

#[tokio::test]
async fn required_unsupported_schema_is_read_only() {
    let temp = tempfile::tempdir().unwrap();
    manifest(
        temp.path(),
        vec![common::manifest_collection(
            "blog",
            "posts",
            "*.md",
            "markdown",
            json!({
                "type": "object",
                "properties": {
                    "special": { "type": "unsupported", "required": true, "reason": "Unknown transform" },
                },
            }),
        )],
    );
    let collections = list_collections(temp.path()).await.unwrap();
    assert!(!collections[0].editable);
    assert_eq!(
        collections[0].fields["properties"]["special"],
        json!({ "type": "unsupported", "required": true, "reason": "Unknown transform" })
    );
    assert!(collections[0].reason.as_deref().unwrap().contains("schema"));
}

#[tokio::test]
async fn catalog_cache_follows_the_manifest_and_root() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let fields = json!({ "type": "object", "properties": {} });
    let collection =
        |name: &str| common::manifest_collection(name, "posts", "*.md", "markdown", fields.clone());
    manifest(root, vec![collection("a")]);
    assert_eq!(list_collections(root).await.unwrap()[0].name, "a");
    assert_eq!(list_collections(root).await.unwrap()[0].name, "a");

    manifest(root, vec![collection("bb")]);
    let collections = list_collections(root).await.unwrap();
    assert_eq!(collections[0].name, "bb");
    assert_eq!(collections[0].loader["base"], "posts");

    let site = root.join("site");
    write(&site.join("astro.config.mjs"), "export default {}");
    manifest(&site, vec![collection("bb")]);
    write(&root.join("orso.json"), r#"{"root":"site"}"#);
    assert_eq!(
        list_collections(root).await.unwrap()[0].loader["base"],
        "site/posts"
    );

    std::fs::remove_file(site.join(".astro/orso-content.json")).unwrap();
    assert!(list_collections(root).await.unwrap().is_empty());
}

#[tokio::test]
async fn escaping_symlinks_are_not_readable() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.md"), "secret").unwrap();
    common::write_content_manifest(temp.path(), "");
    std::fs::create_dir_all(temp.path().join("src/content/blog/en")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.md"),
        temp.path().join("src/content/blog/en/secret.md"),
    )
    .unwrap();
    assert!(list_collections(temp.path())
        .await
        .unwrap()
        .iter()
        .any(|c| c.name == "blog"));
    assert!(list_entries(temp.path(), "blog").await.unwrap().is_empty());
    assert!(read_entry(temp.path(), "blog", "secret").await.is_err());
}

#[tokio::test]
async fn escaping_manifest_symlink_is_ignored() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    common::write_content_manifest(outside.path(), "");
    std::fs::create_dir_all(temp.path().join(".astro")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join(".astro/orso-content.json"),
        temp.path().join(".astro/orso-content.json"),
    )
    .unwrap();
    assert!(list_collections(temp.path()).await.unwrap().is_empty());
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

//! The daemon's save path against the fake dev server serving the real `POST /__orso`.

mod common;

use orso::config::Config;
use orso::content::integration;
use orso::state::AppState;
use serde_json::{json, Map, Value};

const SOURCE: &str =
    "---\ntitle: Old\ndescription: Desc\npubDate: 2024-01-02\n---\n\nOriginal body.\n";

fn fields(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

#[tokio::test]
async fn fake_dev_server_patches_through_the_integration_endpoint() {
    let tmp = tempfile::tempdir().unwrap();
    let config = Config {
        port: 0,
        projects_dir: tmp.path().to_path_buf(),
        no_auth: true,
        ..Default::default()
    };
    let state = AppState::with_options(config, false, common::fake_devserver());
    let id = state.projects.create("Site").await.unwrap().id;
    let dir = state.projects.get(&id).await.unwrap().dir;
    common::write_content_manifest(&dir, "");

    common::start_fake(&state, &id).await;
    let endpoint = state.content_endpoint(&id).await.unwrap().unwrap();

    let source = integration::patch(
        &endpoint,
        "blog",
        SOURCE,
        &fields(json!({ "title": "New" })),
        &[],
        None,
    )
    .await
    .unwrap();
    assert!(source.contains("title: New"), "{source}");
    assert!(!source.contains("title: Old"), "{source}");
    assert!(source.contains("\nOriginal body.\n"), "{source}");

    let err = integration::patch(
        &endpoint,
        "blog",
        SOURCE,
        &fields(json!({ "pubDate": "2024-02-30" })),
        &[],
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(err.status, Some(422), "{err:?}");
    assert_eq!(err.reason, Some("invalid_schema"), "{err:?}");
    let issues = err.details.clone().unwrap();
    assert_eq!(issues[0]["path"], json!(["pubDate"]), "{issues}");

    common::stop_fake(&state, &id).await;
    assert!(state.content_endpoint(&id).await.unwrap().is_none());
}

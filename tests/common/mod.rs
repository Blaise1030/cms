#![allow(dead_code)]

use std::path::Path;
use std::time::{Duration, Instant};

use orso::devserver::DevServerOptions;
use orso::projects::DevserverStatus;
use orso::state::AppState;
use serde_json::{json, Value};
use tokio::process::Command;

const READY_POLL: Duration = Duration::from_millis(10);
const READY_TIMEOUT: Duration = Duration::from_secs(5);

/// Supervise `packages/astro/test/fixtures/fake-devserver.mjs`: the real `POST /__orso`
/// endpoint with the fixture `blog`/`helpDocs` schemas. Needs `npm --prefix packages/astro ci`.
pub fn fake_devserver() -> DevServerOptions {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("packages/astro/test/fixtures/fake-devserver.mjs");
    DevServerOptions::custom(move |spec| {
        let mut command = Command::new("node");
        command.arg(&script).arg(spec.port.to_string());
        command
    })
}

/// Wait until the project's dev server reports ready; panics with the crash reason otherwise.
pub async fn wait_ready(state: &AppState, id: &str) {
    let start = Instant::now();
    loop {
        let status = state.projects.get(id).await.unwrap().devserver;
        if status == DevserverStatus::Ready {
            return;
        }
        assert!(
            start.elapsed() < READY_TIMEOUT,
            "dev server for {id} not ready ({status:?}): {:?}",
            state.devservers.crash_message(id)
        );
        tokio::time::sleep(READY_POLL).await;
    }
}

pub async fn start_fake(state: &AppState, id: &str) {
    state.devservers.start(id).await.unwrap();
    wait_ready(state, id).await;
}

pub async fn stop_fake(state: &AppState, id: &str) {
    state.devservers.stop(id).await;
}

/// One collection entry of `.astro/orso-content.json`.
pub fn manifest_collection(
    name: &str,
    base: &str,
    pattern: &str,
    kind: &str,
    fields: Value,
) -> Value {
    json!({
        "name": name,
        "loader": { "base": base, "pattern": pattern },
        "kind": kind,
        "singleton": null,
        "supportedForEditing": true,
        "reason": null,
        "fields": fields,
    })
}

/// Writes the content manifest the orso Astro integration would emit for the template's
/// `blog` and `helpDocs` collections. `site_prefix` is `""` or a site root like `"site/"`.
pub fn write_content_manifest(project: &Path, site_prefix: &str) {
    let string = json!({ "type": "string", "required": true });
    let blog = json!({
        "type": "object",
        "required": true,
        "properties": {
            "title": string,
            "description": string,
            "pubDate": { "type": "date", "required": true },
            "coverImage": { "type": "image", "required": false },
            "coverAlt": { "type": "string", "required": false },
            "tags": { "type": "array", "items": string, "required": false, "default": [] },
            "relatedPosts": {
                "type": "array",
                "items": { "type": "reference", "collection": "blog", "required": true },
                "required": false,
                "default": [],
            },
        },
    });
    let help_docs = json!({
        "type": "object",
        "required": true,
        "properties": {
            "title": string,
            "description": string,
            "section": string,
            "order": { "type": "number", "required": true },
        },
    });
    let manifest = json!({
        "version": 1,
        "collections": [
            manifest_collection("blog", "src/content/blog/en", "*.md", "markdown", blog),
            manifest_collection("helpDocs", "src/content/helpDocs/en", "*.md", "markdown", help_docs),
        ],
    });
    let dir = project.join(format!("{site_prefix}.astro"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("orso-content.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

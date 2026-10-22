#![allow(dead_code)]

use std::path::Path;

use serde_json::{json, Value};

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

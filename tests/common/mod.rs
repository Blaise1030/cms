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
    let fields = json!({ "type": "object", "properties": {} });
    let manifest = json!({
        "version": 1,
        "collections": [
            manifest_collection("blog", "src/content/blog/en", "*.md", "markdown", fields.clone()),
            manifest_collection("helpDocs", "src/content/helpDocs/en", "*.md", "markdown", fields),
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

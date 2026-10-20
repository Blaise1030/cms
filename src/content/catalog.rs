//! Project-scoped read-only content inventory. Editable paths come from the project helper.
use crate::{error::ApiError, fs};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentCollection {
    pub name: String,
    #[serde(default)]
    pub loader: Value,
    pub kind: String,
    #[serde(default)]
    pub singleton: Option<String>,
    #[serde(default)]
    pub editable: bool,
    #[serde(default, rename = "supportedForEditing")]
    pub supported_for_editing: bool,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub fields: Value,
}
#[derive(Debug, Clone, Serialize)]
pub struct ContentEntrySummary {
    pub collection: String,
    pub id: String,
    pub path: String,
    pub kind: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct ContentEntry {
    pub collection: String,
    pub id: String,
    pub path: String,
    pub kind: String,
    pub source: String,
    pub fields: Value,
    pub body: String,
    pub branch: String,
    pub hash: String,
}
async fn helper(root: &Path, op: &str) -> Result<Vec<ContentCollection>, ApiError> {
    let response = super::helper::run(root, &serde_json::json!({ "op": op }))
        .await?
        .map_err(|e| ApiError::io(format!("content helper: {e}")))?;
    #[derive(Deserialize)]
    struct Response {
        collections: Vec<ContentCollection>,
    }
    let prefix = crate::settings::site_prefix(root);
    serde_json::from_value::<Response>(response)
        .map(|r| {
            let mut collections = r.collections;
            // Bases are relative to the Astro root; make them repo-relative
            // so every caller keeps resolving against the project root.
            for c in &mut collections {
                if let Some(base) = c.loader.get("base").and_then(Value::as_str) {
                    let base = format!("{prefix}{base}");
                    if let Some(loader) = c.loader.as_object_mut() {
                        loader.insert("base".into(), Value::String(base));
                    }
                }
            }
            collections
        })
        .map_err(|e| ApiError::io(format!("content helper invalid response: {e}")))
}
pub(crate) fn supported(c: &ContentCollection) -> bool {
    let Some(base) = c.loader.get("base").and_then(Value::as_str) else {
        return false;
    };
    let Some(pattern) = c.loader.get("pattern").and_then(Value::as_str) else {
        return false;
    };
    let path = Path::new(base);
    !base.contains('\0')
        && !path.is_absolute()
        && !path
            .components()
            .any(|v| matches!(v, std::path::Component::ParentDir))
        && match (c.kind.as_str(), pattern) {
            ("json", p) => p == "*.json" || (!p.contains('*') && p.ends_with(".json")),
            ("markdown", p) => p == "*.md",
            ("mdx", p) => p == "*.mdx",
            _ => false,
        }
}
/// Known bundled paths are used only to expose raw, read-only files after helper failure.
fn fallback(reason: String, prefix: &str) -> Vec<ContentCollection> {
    let blog = format!("{prefix}src/content/blog/en");
    let help = format!("{prefix}src/content/helpDocs/en");
    [
        ("blog", blog.as_str(), "*.md", "markdown", None),
        ("helpDocs", help.as_str(), "*.md", "markdown", None),
    ]
    .into_iter()
    .map(|(name, base, pattern, kind, singleton)| ContentCollection {
        name: name.into(),
        loader: serde_json::json!({"base":base,"pattern":pattern}),
        kind: kind.into(),
        singleton: singleton.map(str::to_string),
        editable: false,
        supported_for_editing: false,
        reason: Some(reason.clone()),
        fields: Value::Null,
    })
    .collect()
}
fn required_unsupported(field: &Value) -> bool {
    if field.get("required").and_then(Value::as_bool) == Some(false) {
        return false;
    }
    field.get("type").and_then(Value::as_str) == Some("unsupported")
        || field
            .get("properties")
            .and_then(Value::as_object)
            .is_some_and(|p| p.values().any(required_unsupported))
        || field.get("items").is_some_and(required_unsupported)
        || field
            .get("variants")
            .and_then(Value::as_array)
            .is_some_and(|v| v.iter().any(required_unsupported))
}
/// Apply editability and the read-only reason from the helper's descriptions.
fn finish(
    mut collections: Vec<ContentCollection>,
    describe_error: Option<&ApiError>,
) -> Vec<ContentCollection> {
    for c in &mut collections {
        c.editable = matches!(c.name.as_str(), "blog" | "helpDocs")
            && c.supported_for_editing
            && supported(c)
            && !c.fields.is_null()
            && !required_unsupported(&c.fields);
        c.reason = if c.editable {
            None
        } else {
            Some(match describe_error {
                Some(error) => format!("Content helper describe unavailable: {error}"),
                None => "Unsupported collection loader or schema; content is read-only".into(),
            })
        };
    }
    collections
}
/// Files whose modification times decide whether a cached catalog is still current.
const CATALOG_INPUTS: [&str; 3] = [
    "src/content-schemas.mjs",
    "src/content.config.ts",
    super::helper::SCRIPT,
];
type CatalogKey = (String, [Option<std::time::SystemTime>; 3]);
type CatalogCache = HashMap<PathBuf, (CatalogKey, Vec<ContentCollection>)>;
static CATALOG: LazyLock<Mutex<CatalogCache>> = LazyLock::new(Default::default);
fn catalog_key(root: &Path) -> CatalogKey {
    let prefix = crate::settings::site_prefix(root);
    // symlink_metadata: replacing an input with a link also changes the key.
    let mtimes = CATALOG_INPUTS.map(|path| {
        std::fs::symlink_metadata(root.join(format!("{prefix}{path}")))
            .and_then(|m| m.modified())
            .ok()
    });
    (prefix, mtimes)
}
/// The catalog with field descriptors. One `describe` helper run serves both the
/// collection list and the fields; complete results are cached per project until the site
/// prefix or a schema input's mtime changes, so file events do not spawn Node for an
/// unchanged catalog.
pub async fn list_collections(project_root: &Path) -> Result<Vec<ContentCollection>, ApiError> {
    let key = catalog_key(project_root);
    let cached = CATALOG
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(project_root)
        .filter(|(k, _)| *k == key)
        .map(|(_, collections)| collections.clone());
    if let Some(collections) = cached {
        return Ok(collections);
    }
    let collections = match helper(project_root, "describe").await {
        Ok(described) => {
            let collections = finish(described, None);
            CATALOG
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .insert(project_root.to_path_buf(), (key, collections.clone()));
            collections
        }
        // Not cached: a helper without `describe`, or a transient failure, lists read-only.
        Err(describe_error) => match helper(project_root, "list").await {
            Ok(mut listed) => {
                listed.iter_mut().for_each(|c| c.fields = Value::Null);
                finish(listed, Some(&describe_error))
            }
            Err(e) => fallback(
                format!("Content helper unavailable: {e}"),
                &crate::settings::site_prefix(project_root),
            ),
        },
    };
    Ok(collections)
}
pub(crate) async fn collection(root: &Path, name: &str) -> Result<ContentCollection, ApiError> {
    list_collections(root)
        .await?
        .into_iter()
        .find(|c| c.name == name)
        .ok_or_else(|| ApiError::enoent("no such content collection"))
}
pub(crate) async fn entries(
    root: &Path,
    c: &ContentCollection,
) -> Result<Vec<ContentEntrySummary>, ApiError> {
    if !supported(c) {
        return Ok(vec![]);
    }
    let base = c.loader["base"].as_str().unwrap();
    let pattern = c.loader["pattern"].as_str().unwrap();
    let tree = match fs::tree(root, base).await {
        Ok(v) => v,
        Err(e) if e.code == crate::error::ErrorCode::Enoent => return Ok(vec![]),
        Err(e) => return Err(e),
    };
    Ok(tree
        .into_iter()
        .filter(|e| {
            e.kind == fs::EntryKind::File
                && if let Some(suffix) = pattern.strip_prefix('*') {
                    e.name.ends_with(suffix)
                } else {
                    e.name == pattern
                }
        })
        .map(|e| ContentEntrySummary {
            collection: c.name.clone(),
            id: Path::new(&e.name)
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            path: e.path,
            kind: c.kind.clone(),
        })
        .collect())
}
pub async fn list_entries(root: &Path, name: &str) -> Result<Vec<ContentEntrySummary>, ApiError> {
    entries(root, &collection(root, name).await?).await
}
pub async fn read_entry(
    project_root: &Path,
    collection_name: &str,
    id: &str,
) -> Result<ContentEntry, ApiError> {
    let c = collection(project_root, collection_name).await?;
    let e = entries(project_root, &c)
        .await?
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| ApiError::enoent("no such content entry"))?;
    let source = fs::read_file(project_root, &e.path)
        .await?
        .content
        .ok_or_else(|| ApiError::badreq("content entry is not UTF-8"))?;
    let branch = current_branch(project_root).await?;
    Ok(build_entry(
        e.collection,
        e.id,
        e.path,
        &c.kind,
        source,
        branch,
    ))
}
/// The checked-out branch name (empty when detached).
pub(crate) async fn current_branch(project_root: &Path) -> Result<String, ApiError> {
    let output = tokio::process::Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(project_root)
        .output()
        .await?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
pub(crate) fn hash(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}
pub(crate) fn build_entry(
    collection: String,
    id: String,
    path: String,
    kind: &str,
    source: String,
    branch: String,
) -> ContentEntry {
    let (fields, body) = if kind == "json" {
        (
            serde_json::from_str(&source).unwrap_or(Value::Null),
            String::new(),
        )
    } else {
        parse_frontmatter(&source)
    };
    ContentEntry {
        collection,
        id,
        path,
        kind: kind.to_string(),
        hash: hash(&source),
        source,
        fields,
        body,
        branch,
    }
}
fn parse_frontmatter(source: &str) -> (Value, String) {
    let mut offset = 0;
    let mut lines = source.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return (Value::Null, source.into());
    };
    if first.trim_end() != "---" {
        return (Value::Null, source.into());
    }
    offset += first.len();
    let start = offset;
    for line in lines {
        if line.trim_end() == "---" {
            return (
                serde_yaml::from_str::<Value>(&source[start..offset]).unwrap_or(Value::Null),
                source[offset + line.len()..].into(),
            );
        }
        offset += line.len();
    }
    (Value::Null, source.into())
}

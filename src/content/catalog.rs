//! Project-scoped read-only content inventory. Collection metadata comes from the
//! `.astro/orso-content.json` manifest the orso Astro integration writes.
use crate::{error::ApiError, fs};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

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
/// Written by the orso Astro integration during `astro dev` and `astro sync`.
const MANIFEST: &str = ".astro/orso-content.json";
#[derive(Deserialize)]
struct Manifest {
    version: u32,
    collections: Vec<ContentCollection>,
}
/// Collections from the site's manifest; empty when it is missing, unreadable or invalid.
async fn manifest(site: &Path, prefix: &str) -> Vec<ContentCollection> {
    let Ok(Some(text)) = fs::read_file(site, MANIFEST).await.map(|f| f.content) else {
        return vec![];
    };
    let Ok(manifest) = serde_json::from_str::<Manifest>(&text) else {
        return vec![];
    };
    if manifest.version != 1 {
        return vec![];
    }
    let mut collections = manifest.collections;
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
/// Apply editability, keeping the manifest's read-only reason when it gives one.
fn finish(mut collections: Vec<ContentCollection>) -> Vec<ContentCollection> {
    for c in &mut collections {
        c.editable = c.supported_for_editing
            && supported(c)
            && !c.fields.is_null()
            && !required_unsupported(&c.fields);
        c.reason = if c.editable {
            None
        } else {
            Some(c.reason.take().unwrap_or_else(|| {
                "Unsupported collection loader or schema; content is read-only".into()
            }))
        };
    }
    collections
}
/// Site prefix plus the manifest's mtime and length; the length catches rewrites within
/// the filesystem's mtime granularity.
type CatalogKey = (String, Option<(SystemTime, u64)>);
type CatalogCache = HashMap<PathBuf, (CatalogKey, Vec<ContentCollection>)>;
static CATALOG: LazyLock<Mutex<CatalogCache>> = LazyLock::new(Default::default);
/// The catalog with field descriptors, read from the site's integration manifest. Results
/// are cached per project until the site prefix or the manifest's stamp changes.
pub async fn list_collections(project_root: &Path) -> Result<Vec<ContentCollection>, ApiError> {
    let settings = crate::settings::load(project_root).ok();
    let prefix = settings
        .as_ref()
        .map(|s| s.site_prefix())
        .unwrap_or_default();
    let site = settings.map_or_else(|| project_root.to_path_buf(), |s| s.site_dir(project_root));
    // symlink_metadata: replacing the manifest with a link also changes the key.
    let stamp = std::fs::symlink_metadata(site.join(MANIFEST))
        .and_then(|m| Ok((m.modified()?, m.len())))
        .ok();
    let key = (prefix, stamp);
    let cached = CATALOG
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(project_root)
        .filter(|(k, _)| *k == key)
        .map(|(_, collections)| collections.clone());
    if let Some(collections) = cached {
        return Ok(collections);
    }
    let collections = finish(manifest(&site, &key.0).await);
    CATALOG
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(project_root.to_path_buf(), (key, collections.clone()));
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

//! Safe structured content create/update.
//!
//! Under a per-project lock: require an active change, check the opened branch and hash,
//! resolve only catalog-declared paths without following symlinks, validate the complete
//! candidate with the template helper, write through a same-directory temporary file and
//! rename, then checkpoint only the entry path.
use super::{catalog, helper};
use crate::{changes, error::ApiError, git, state::AppState};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use tokio::sync::Mutex;

pub enum SaveTarget {
    Create { slug: String },
    Update { id: String, hash: String },
}

pub struct SaveEntry {
    pub project: String,
    pub collection: String,
    /// Branch the editor opened the entry on.
    pub branch: String,
    pub target: SaveTarget,
    /// Partial top-level update (complete starting fields for create).
    pub fields: Map<String, Value>,
    /// Markdown/MDX body; `None` keeps the current body on update.
    pub body: Option<String>,
    /// Optional top-level fields to remove.
    pub unset: Vec<String>,
}

/// Page slugs owned by template routes.
const RESERVED_PAGES: &[&str] = &["blog", "help"];

fn lock_for(dir: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<StdMutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let mut map = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    map.entry(dir.to_path_buf()).or_default().clone()
}

fn unsafe_path(message: &str) -> ApiError {
    ApiError::sandbox(message).with_reason("unsafe_path")
}

fn invalid_slug(message: &str) -> ApiError {
    ApiError::badreq(message).with_reason("invalid_slug")
}

fn stale() -> ApiError {
    ApiError::conflict("The entry changed since it was opened; reload to compare")
        .with_reason("stale_content")
}

/// Content edits always need an active change, for GitHub and local-only projects alike.
/// Returns the current branch, which must be the one the editor opened.
async fn require_content_change(root: &Path, opened: &str) -> Result<String, ApiError> {
    let Some(active) = changes::active(root).await? else {
        return Err(ApiError::change_required(
            "Start a change first — content cannot be edited on main",
        ));
    };
    if active.branch != opened {
        return Err(ApiError::conflict(format!(
            "The project is now on {}; the entry was opened on {opened}",
            active.branch
        ))
        .with_reason("branch_changed"));
    }
    Ok(active.branch)
}

fn validate_slug(collection: &str, slug: &str) -> Result<(), ApiError> {
    let valid = !slug.is_empty()
        && slug.len() <= 100
        && slug.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        });
    if !valid {
        return Err(invalid_slug(
            "Slugs use lowercase letters, digits, and single hyphens",
        ));
    }
    if collection == "pages" && RESERVED_PAGES.contains(&slug) {
        return Err(invalid_slug(&format!("\"{slug}\" is a reserved page slug")));
    }
    Ok(())
}

fn validate_id(id: &str) -> Result<(), ApiError> {
    if id.is_empty() || id.starts_with('.') || id.contains(['/', '\\', '\0']) {
        return Err(invalid_slug("invalid entry id"));
    }
    Ok(())
}

/// Project-relative path of `id` under the collection's declared loader.
fn entry_path(c: &catalog::ContentCollection, id: &str) -> Result<String, ApiError> {
    let base = c.loader["base"].as_str().unwrap_or_default();
    let pattern = c.loader["pattern"].as_str().unwrap_or_default();
    let name = match pattern.strip_prefix('*') {
        Some(ext) => format!("{id}{ext}"),
        None if Path::new(pattern).file_stem().and_then(|s| s.to_str()) == Some(id) => {
            pattern.to_string()
        }
        None => return Err(ApiError::enoent("no such content entry")),
    };
    let mut parts = vec![];
    for part in Path::new(base).join(&name).components() {
        match part {
            Component::CurDir => {}
            Component::Normal(p) => parts.push(
                p.to_str()
                    .ok_or_else(|| unsafe_path("content path is not UTF-8"))?
                    .to_string(),
            ),
            _ => return Err(unsafe_path("content path escapes the project")),
        }
    }
    if parts.iter().any(|p| {
        [".git", ".hg", ".svn", ".bzr", "_darcs", ".fossil", ".jj"]
            .iter()
            .any(|v| p.eq_ignore_ascii_case(v))
    }) {
        return Err(unsafe_path("content path enters version-control metadata"));
    }
    Ok(parts.join("/"))
}

/// Resolve `rel` under the canonical root, refusing any symlink or non-directory parent
/// and a final component that is a symlink or non-file. Missing components are allowed.
fn resolve(root: &Path, rel: &str) -> Result<PathBuf, ApiError> {
    let mut target = std::fs::canonicalize(root)?;
    let parts: Vec<_> = Path::new(rel).components().collect();
    for (i, part) in parts.iter().enumerate() {
        target.push(part);
        match std::fs::symlink_metadata(&target) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(unsafe_path("content paths cannot pass through symlinks"))
            }
            Ok(meta) if i + 1 < parts.len() && !meta.is_dir() => {
                return Err(unsafe_path("content parent is not a directory"))
            }
            Ok(meta) if i + 1 == parts.len() && !meta.is_file() => {
                return Err(unsafe_path("content entry is not a regular file"))
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(target)
}

/// Create missing parents one level at a time, re-checking each for symlinks.
fn create_parents(root: &Path, rel: &str) -> Result<PathBuf, ApiError> {
    let target = resolve(root, rel)?;
    let parent = target.parent().unwrap();
    let canon_root = std::fs::canonicalize(root)?;
    let mut dir = canon_root.clone();
    for part in parent.strip_prefix(&canon_root).unwrap().components() {
        dir.push(part);
        match std::fs::create_dir(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
    }
    // Containment after creation: the parent's real path must be the lexical one.
    if std::fs::canonicalize(parent)? != parent {
        return Err(unsafe_path("content parent escapes the project"));
    }
    resolve(root, rel)
}

/// Write `bytes` through a same-directory temporary file and atomic rename.
fn write_atomic(target: &Path, bytes: &[u8], mode: u32) -> Result<(), ApiError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let name = target.file_name().unwrap().to_string_lossy();
    let tmp = target.with_file_name(format!(
        ".{name}.{}-{}.workbench-tmp",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&tmp, target)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.map_err(ApiError::from)
}

/// Initial source for a new entry; the helper validates exactly these bytes.
fn render(kind: &str, fields: &Map<String, Value>, body: &str) -> Result<String, ApiError> {
    if kind == "json" {
        if !body.is_empty() {
            return Err(ApiError::badreq("JSON entries have no body"));
        }
        let text =
            serde_json::to_string_pretty(fields).map_err(|e| ApiError::badreq(e.to_string()))?;
        return Ok(format!("{text}\n"));
    }
    // JSON flow values are valid YAML and keep strings quoted, so YAML 1.1 parsers
    // (e.g. Astro's) never reinterpret values such as dates.
    let mut yaml = String::new();
    for (key, value) in fields {
        let plain = key
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        let key = if plain {
            key.clone()
        } else {
            Value::from(key.as_str()).to_string()
        };
        yaml.push_str(&format!("{key}: {value}\n"));
    }
    Ok(format!("---\n{yaml}---\n{body}"))
}

fn read_current(target: &Path) -> Result<String, ApiError> {
    let bytes = std::fs::read(target)?;
    if bytes.len() as u64 > crate::fs::MAX_FILE_BYTES {
        return Err(ApiError::badreq("content entry is too large"));
    }
    String::from_utf8(bytes).map_err(|_| ApiError::badreq("content entry is not UTF-8"))
}

pub async fn save_entry(
    state: &AppState,
    request: SaveEntry,
) -> Result<catalog::ContentEntry, ApiError> {
    let root = state
        .projects
        .get(&request.project)
        .await
        .map(|p| p.dir)
        .ok_or_else(|| ApiError::enoent(format!("no such project: {}", request.project)))?;
    let lock = lock_for(&root);
    let _guard = lock.lock().await;
    let branch = require_content_change(&root, &request.branch).await?;

    let unsupported =
        |message: String| ApiError::badreq(message).with_reason("unsupported_collection");
    let c = match catalog::collection(&root, &request.collection).await {
        Ok(c) => c,
        Err(e) if e.code == crate::error::ErrorCode::Enoent => {
            return Err(unsupported(format!(
                "no such content collection: {}",
                request.collection
            )))
        }
        Err(e) => return Err(e),
    };
    if !c.editable || !catalog::supported(&c) {
        return Err(unsupported(
            c.reason
                .clone()
                .unwrap_or_else(|| "collection is read-only".into()),
        ));
    }
    if request.body.is_some() && c.kind == "json" && request.body.as_deref() != Some("") {
        return Err(ApiError::badreq("JSON entries have no body"));
    }

    let (id, created) = match &request.target {
        SaveTarget::Create { slug } => {
            if c.singleton.is_some() {
                return Err(ApiError::badreq(format!("{} is update-only", c.name)));
            }
            validate_slug(&c.name, slug)?;
            (slug.clone(), true)
        }
        SaveTarget::Update { id, .. } => {
            validate_id(id)?;
            (id.clone(), false)
        }
    };
    let rel = entry_path(&c, &id)?;
    let target = resolve(&root, &rel)?;

    let (old, mode, source) = if created {
        if std::fs::symlink_metadata(&target).is_ok() {
            return Err(
                ApiError::conflict(format!("{} already has an entry named {id}", c.name))
                    .with_reason("duplicate_slug"),
            );
        }
        let mut fields = request.fields.clone();
        for key in &request.unset {
            fields.remove(key);
        }
        let initial = render(&c.kind, &fields, request.body.as_deref().unwrap_or(""))?;
        let source = helper::patch(&root, &c.name, &initial, &Map::new(), &[], None).await?;
        (None, 0o644, source)
    } else {
        let SaveTarget::Update { hash, .. } = &request.target else {
            unreachable!()
        };
        let old = read_current(&target)?;
        if &catalog::hash(&old) != hash {
            return Err(stale());
        }
        let mode = std::fs::metadata(&target)?.permissions().mode() & 0o7777;
        let body = if c.kind == "json" {
            None
        } else {
            request.body.as_deref()
        };
        let source =
            helper::patch(&root, &c.name, &old, &request.fields, &request.unset, body).await?;
        if source == old {
            return Ok(catalog::build_entry(c.name, id, rel, &c.kind, old, branch));
        }
        (Some(old), mode, source)
    };

    // Re-verify immediately before mutating: the helper ran outside git's view.
    require_content_change(&root, &request.branch).await?;
    let target = if created {
        if std::fs::symlink_metadata(resolve(&root, &rel)?).is_ok() {
            return Err(
                ApiError::conflict(format!("{} already has an entry named {id}", c.name))
                    .with_reason("duplicate_slug"),
            );
        }
        create_parents(&root, &rel)?
    } else {
        let target = resolve(&root, &rel)?;
        if read_current(&target)? != *old.as_ref().unwrap() {
            return Err(stale());
        }
        target
    };
    write_atomic(&target, source.as_bytes(), mode)?;

    let summary = format!(
        "{} {}/{id}",
        if created { "Create" } else { "Update" },
        c.name
    );
    let checkpoint = match git::checkpoint_paths(&root, &summary, &[PathBuf::from(&rel)]).await {
        Ok(cp) => cp,
        Err(e) => {
            // Roll back the entry so a failed save leaves no partial update.
            let _ = match &old {
                Some(old) => write_atomic(&target, old.as_bytes(), mode),
                None => std::fs::remove_file(&target).map_err(ApiError::from),
            };
            return Err(e);
        }
    };
    state.events.emit(
        "fs.change",
        json!({ "projectId": request.project, "path": rel, "kind": if created { "create" } else { "modify" } }),
    );
    if let Some(cp) = checkpoint {
        state.events.emit(
            "checkpoint.created",
            json!({
                "projectId": request.project,
                "sha": cp.sha,
                "summary": cp.summary,
                "author": cp.author,
                "ts": cp.ts,
            }),
        );
    }
    Ok(catalog::build_entry(
        c.name, id, rel, &c.kind, source, branch,
    ))
}

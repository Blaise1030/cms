//! Safe structured content create/update; rename/delete live in `lifecycle`.
//!
//! Under a per-project lock: require an active change, check the opened branch and hash,
//! resolve only catalog-declared paths without following symlinks, validate the complete
//! candidate with the template helper, write through a same-directory temporary file and
//! rename, then checkpoint only the entry path (plus any images attached through `images`).
use super::lifecycle::{Journal, Undo};
use super::{catalog, helper, images};
use crate::{changes, error::ApiError, git, state::AppState};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use tokio::sync::Mutex;

pub use super::lifecycle::{
    delete_entry, inspect_impact, rename_entry, ContentImpact, DeleteEntry, RenameEntry,
};

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
    /// Images named by `workbench-image:<clientId>` tokens in `fields` or `body`.
    pub images: Vec<images::Upload>,
}

/// A saved entry and, for each attached image, where it was placed.
#[derive(Debug, Serialize)]
pub struct SavedEntry {
    #[serde(flatten)]
    pub entry: catalog::ContentEntry,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub images: BTreeMap<String, images::SavedImage>,
}

/// Page slugs owned by template routes.
const RESERVED_PAGES: &[&str] = &["blog", "help"];

pub(super) fn lock_for(dir: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<StdMutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let mut map = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    map.entry(dir.to_path_buf()).or_default().clone()
}

pub(super) fn unsafe_path(message: &str) -> ApiError {
    ApiError::sandbox(message).with_reason("unsafe_path")
}

pub(super) fn invalid_slug(message: &str) -> ApiError {
    ApiError::badreq(message).with_reason("invalid_slug")
}

pub(super) fn stale() -> ApiError {
    ApiError::conflict("The entry changed since it was opened; reload to compare")
        .with_reason("stale_content")
}

/// Content edits always need an active change, for GitHub and local-only projects alike.
/// Returns the current branch, which must be the one the editor opened.
pub(super) async fn require_content_change(root: &Path, opened: &str) -> Result<String, ApiError> {
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

pub(super) fn validate_slug(collection: &str, slug: &str) -> Result<(), ApiError> {
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

pub(super) fn validate_id(id: &str) -> Result<(), ApiError> {
    if id.is_empty() || id.starts_with('.') || id.contains(['/', '\\', '\0']) {
        return Err(invalid_slug("invalid entry id"));
    }
    Ok(())
}

/// Project-relative path of `id` under the collection's declared loader.
pub(super) fn entry_path(c: &catalog::ContentCollection, id: &str) -> Result<String, ApiError> {
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

/// What the final component of a resolved path must be when it exists.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Leaf {
    File,
    Dir,
}

/// Resolve `rel` under the canonical root, refusing any symlink or non-directory parent
/// and a final component that is a symlink or not a regular file. Missing components are
/// allowed.
pub(super) fn resolve(root: &Path, rel: &str) -> Result<PathBuf, ApiError> {
    resolve_as(root, rel, Leaf::File)
}

/// [`resolve`] with the expected kind of the final component.
pub(super) fn resolve_as(root: &Path, rel: &str, leaf: Leaf) -> Result<PathBuf, ApiError> {
    let mut target = std::fs::canonicalize(root)?;
    let parts: Vec<_> = Path::new(rel).components().collect();
    for (i, part) in parts.iter().enumerate() {
        if !matches!(part, Component::Normal(_)) {
            return Err(unsafe_path("content path escapes the project"));
        }
        target.push(part);
        let last = i + 1 == parts.len();
        match std::fs::symlink_metadata(&target) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(unsafe_path("content paths cannot pass through symlinks"))
            }
            Ok(meta) if !last && !meta.is_dir() => {
                return Err(unsafe_path("content parent is not a directory"))
            }
            Ok(meta) if last && leaf == Leaf::File && !meta.is_file() => {
                return Err(unsafe_path("content entry is not a regular file"))
            }
            Ok(meta) if last && leaf == Leaf::Dir && !meta.is_dir() => {
                return Err(unsafe_path("content asset path is not a directory"))
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(target)
}

/// Create the missing directories of `rel_dir` one level at a time, re-checking each for
/// symlinks. Directories it created are appended to `created`, outermost first, even when
/// it fails part-way.
pub(super) fn create_dirs(
    root: &Path,
    rel_dir: &str,
    created: &mut Vec<PathBuf>,
) -> Result<PathBuf, ApiError> {
    let target = resolve_as(root, rel_dir, Leaf::Dir)?;
    let canon_root = std::fs::canonicalize(root)?;
    let mut dir = canon_root.clone();
    for part in target.strip_prefix(&canon_root).unwrap().components() {
        dir.push(part);
        match std::fs::create_dir(&dir) {
            Ok(()) => created.push(dir.clone()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
    }
    // Containment after creation: the directory's real path must be the lexical one.
    if std::fs::canonicalize(&target)? != target {
        return Err(unsafe_path("content parent escapes the project"));
    }
    resolve_as(root, rel_dir, Leaf::Dir)
}

/// Create the missing parents of the file at `rel`; see [`create_dirs`].
fn create_parents(root: &Path, rel: &str, created: &mut Vec<PathBuf>) -> Result<PathBuf, ApiError> {
    let parent = Path::new(rel).parent().and_then(Path::to_str).unwrap_or("");
    create_dirs(root, parent, created)?;
    resolve(root, rel)
}

/// Write `bytes` through a same-directory temporary file and atomic rename.
pub(super) fn write_atomic(target: &Path, bytes: &[u8], mode: u32) -> Result<(), ApiError> {
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

pub(super) fn read_current(target: &Path) -> Result<String, ApiError> {
    let bytes = std::fs::read(target)?;
    if bytes.len() as u64 > crate::fs::MAX_FILE_BYTES {
        return Err(ApiError::badreq("content entry is too large"));
    }
    String::from_utf8(bytes).map_err(|_| ApiError::badreq("content entry is not UTF-8"))
}

/// The registered project's directory.
pub(super) async fn project_root(state: &AppState, project: &str) -> Result<PathBuf, ApiError> {
    state
        .projects
        .get(project)
        .await
        .map(|p| p.dir)
        .ok_or_else(|| ApiError::enoent(format!("no such project: {project}")))
}

/// A catalog collection whose loader and schema support structured editing.
pub(super) async fn editable_collection(
    root: &Path,
    name: &str,
) -> Result<catalog::ContentCollection, ApiError> {
    editable_in(&catalog::list_collections(root).await?, name)
}

/// [`editable_collection`] from an already-listed catalog.
pub(super) fn editable_in(
    collections: &[catalog::ContentCollection],
    name: &str,
) -> Result<catalog::ContentCollection, ApiError> {
    let unsupported =
        |message: String| ApiError::badreq(message).with_reason("unsupported_collection");
    let c = collections
        .iter()
        .find(|c| c.name == name)
        .cloned()
        .ok_or_else(|| unsupported(format!("no such content collection: {name}")))?;
    if !c.editable || !catalog::supported(&c) {
        return Err(unsupported(
            c.reason
                .clone()
                .unwrap_or_else(|| "collection is read-only".into()),
        ));
    }
    Ok(c)
}

pub(super) fn duplicate_slug(c: &catalog::ContentCollection, id: &str) -> ApiError {
    ApiError::conflict(format!("{} already has an entry named {id}", c.name))
        .with_reason("duplicate_slug")
}

/// Refusal for the singleton `site` and the required `pages/home` entry.
pub(super) fn protected(message: String) -> ApiError {
    ApiError::badreq(message).with_reason("protected_entry")
}

/// Announce a created checkpoint so History refreshes.
pub(super) fn emit_checkpoint(state: &AppState, project: &str, cp: Option<git::Checkpoint>) {
    if let Some(cp) = cp {
        state.events.emit(
            "checkpoint.created",
            json!({
                "projectId": project,
                "sha": cp.sha,
                "summary": cp.summary,
                "author": cp.author,
                "ts": cp.ts,
            }),
        );
    }
}

pub async fn save_entry(state: &AppState, request: SaveEntry) -> Result<SavedEntry, ApiError> {
    let root = project_root(state, &request.project).await?;
    let lock = lock_for(&root);
    let _guard = lock.lock().await;
    images::cleanup_orphans(&root);
    let branch = require_content_change(&root, &request.branch).await?;
    let c = editable_collection(&root, &request.collection).await?;
    if request.body.is_some() && c.kind == "json" && request.body.as_deref() != Some("") {
        return Err(ApiError::badreq("JSON entries have no body"));
    }

    let (id, created) = match &request.target {
        SaveTarget::Create { slug } => {
            if c.singleton.is_some() {
                return Err(protected(format!("{} is update-only", c.name)));
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
    if created && std::fs::symlink_metadata(&target).is_ok() {
        return Err(duplicate_slug(&c, &id));
    }
    // Validate and stage attached images, turning their tokens into relative paths.
    let (mut fields, mut body) = (request.fields, request.body);
    let prepared = images::prepare(
        &root,
        &c.name,
        &id,
        &rel,
        &request.images,
        &mut fields,
        &mut body,
    )?;

    let (old, mode, source) = if created {
        let mut fields = fields.clone();
        for key in &request.unset {
            fields.remove(key);
        }
        let initial = render(&c.kind, &fields, body.as_deref().unwrap_or(""))?;
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
            body.as_deref()
        };
        let source = helper::patch(&root, &c.name, &old, &fields, &request.unset, body).await?;
        if source == old {
            return Ok(SavedEntry {
                entry: catalog::build_entry(c.name, id, rel, &c.kind, old, branch),
                images: BTreeMap::new(),
            });
        }
        (Some(old), mode, source)
    };

    // Every reference in the saved entry must name an existing entry of its target.
    let saved = catalog::build_entry(
        c.name.clone(),
        id.clone(),
        rel.clone(),
        &c.kind,
        source.clone(),
        String::new(),
    );
    super::lifecycle::check_references(&root, &c, &id, &saved.fields).await?;

    // Re-verify immediately before mutating: the helper ran outside git's view.
    require_content_change(&root, &request.branch).await?;
    if created {
        if std::fs::symlink_metadata(resolve(&root, &rel)?).is_ok() {
            return Err(duplicate_slug(&c, &id));
        }
    } else if read_current(&resolve(&root, &rel)?)? != *old.as_ref().unwrap() {
        return Err(stale());
    }

    // Apply under an undo journal so a failed write or checkpoint leaves no partial update.
    let mut journal = Journal::default();
    let applied = (|| {
        let assets = match &prepared {
            Some(p) => p.place(&root, &mut journal)?,
            None => vec![],
        };
        let target = if created {
            let mut dirs = vec![];
            let target = create_parents(&root, &rel, &mut dirs);
            journal.0.extend(dirs.into_iter().map(Undo::RemoveDir));
            target?
        } else {
            resolve(&root, &rel)?
        };
        write_atomic(&target, source.as_bytes(), mode)?;
        journal.0.push(match &old {
            Some(old) => Undo::Write {
                path: target,
                bytes: old.clone(),
                mode,
            },
            None => Undo::Remove(target),
        });
        Ok::<_, ApiError>(assets)
    })();
    let assets = match applied {
        Ok(assets) => assets,
        Err(e) => return Err(journal.fail(e)),
    };

    let summary = format!(
        "{} {}/{id}",
        if created { "Create" } else { "Update" },
        c.name
    );
    let mut paths = vec![PathBuf::from(&rel)];
    paths.extend(assets.iter().map(PathBuf::from));
    let checkpoint = match git::checkpoint_paths(&root, &summary, &paths).await {
        Ok(cp) => cp,
        Err(e) => return Err(journal.fail(e)),
    };
    state.events.emit(
        "fs.change",
        json!({ "projectId": request.project, "path": rel, "kind": if created { "create" } else { "modify" } }),
    );
    for asset in &assets {
        state.events.emit(
            "fs.change",
            json!({ "projectId": request.project, "path": asset, "kind": "create" }),
        );
    }
    emit_checkpoint(state, &request.project, checkpoint);
    Ok(SavedEntry {
        entry: catalog::build_entry(c.name, id, rel, &c.kind, source, branch),
        images: prepared.as_ref().map(|p| p.saved()).unwrap_or_default(),
    })
}

//! Rename and delete content entries with their schema references and owned assets.
//!
//! Both operations reuse the save guards in [`super::mutation`]: per-project lock, active
//! `content/*` branch, opened hash, symlink-free paths, and helper validation. Every new
//! source is computed and validated in memory before the first write; the writes are then
//! applied with an undo journal so a failed write or checkpoint leaves the files as they
//! were. A rename or delete is one scoped checkpoint covering the old and new entry paths,
//! the owned asset files (`src/assets/<collection>/<id>/`), and every rewritten reference.
use super::mutation::{self as m, Leaf};
use super::{catalog, helper};
use crate::{error::ApiError, git, state::AppState};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct RenameEntry {
    pub project: String,
    pub collection: String,
    pub id: String,
    pub slug: String,
    /// Branch and content hash the editor opened the entry with.
    pub branch: String,
    pub hash: String,
}

pub struct DeleteEntry {
    pub project: String,
    pub collection: String,
    pub id: String,
    pub branch: String,
    pub hash: String,
}

/// A schema reference field in another entry that names the inspected entry.
#[derive(Debug, Clone, Serialize)]
pub struct Reference {
    pub collection: String,
    pub id: String,
    pub path: String,
    /// Dotted field path without array indices, e.g. `sections`.
    pub field: String,
}

/// A local Markdown link to the entry's public URL; reported, never rewritten.
#[derive(Debug, Clone, Serialize)]
pub struct ProseLink {
    pub collection: String,
    pub id: String,
    pub path: String,
    pub url: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentImpact {
    pub references: Vec<Reference>,
    /// Project-relative files in the entry's owned asset directory.
    pub assets: Vec<String>,
    pub prose_links: Vec<ProseLink>,
}

/// One entry read for analysis.
struct Scanned {
    collection: String,
    descriptor: Value,
    kind: String,
    id: String,
    path: String,
    source: String,
    fields: Value,
}

/// A reference holder's pending rewrite: top-level fields to patch.
struct Rewrite {
    entry: Scanned,
    fields: Map<String, Value>,
}

struct Analysis {
    impact: ContentImpact,
    rewrites: Vec<Rewrite>,
}

fn asset_dir(collection: &str, id: &str) -> String {
    format!("src/assets/{collection}/{id}")
}

/// Public URL of an entry under the bundled template routes, when it has one.
fn public_url(collection: &str, id: &str) -> Option<String> {
    match collection {
        "pages" if id != "home" => Some(format!("/{id}")),
        "blog" => Some(format!("/blog/{id}")),
        "helpDocs" => Some(format!("/help/{id}")),
        _ => None,
    }
}

/// `to` relative to the directory `from` (both project-relative, `/`-separated).
fn relative(from: &str, to: &str) -> String {
    let a: Vec<_> = from.split('/').filter(|p| !p.is_empty()).collect();
    let b: Vec<_> = to.split('/').collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts = vec![".."; a.len() - common];
    parts.extend(&b[common..]);
    parts.join("/")
}

/// Leaf visitor: `(descriptor, value, dotted field path)` → optional replacement.
type Visit<'a> = dyn FnMut(&Value, &Value, &[String]) -> Option<Value> + 'a;

/// Walk `value` along its field descriptor, calling `visit` on every leaf with the dotted
/// field path. Returns the rewritten value when `visit` replaced any leaf.
fn rewrite(
    desc: &Value,
    value: &Value,
    path: &mut Vec<String>,
    visit: &mut Visit,
) -> Option<Value> {
    match desc["type"].as_str() {
        Some("object") => {
            let (obj, props) = (value.as_object()?, desc["properties"].as_object()?);
            let mut out: Option<Map<String, Value>> = None;
            for (key, field) in props {
                let Some(item) = obj.get(key) else { continue };
                path.push(key.clone());
                if let Some(next) = rewrite(field, item, path, visit) {
                    out.get_or_insert_with(|| obj.clone())
                        .insert(key.clone(), next);
                }
                path.pop();
            }
            out.map(Value::Object)
        }
        Some("array") => {
            let items = value.as_array()?;
            let mut out: Option<Vec<Value>> = None;
            for (i, item) in items.iter().enumerate() {
                if let Some(next) = rewrite(&desc["items"], item, path, visit) {
                    out.get_or_insert_with(|| items.clone())[i] = next;
                }
            }
            out.map(Value::Array)
        }
        Some("union") => {
            let tag = value.get(desc["discriminator"].as_str()?)?;
            let variant = desc["variants"].as_array()?.iter().find(|v| {
                v["properties"][desc["discriminator"].as_str().unwrap()]["values"]
                    .as_array()
                    .is_some_and(|values| values.contains(tag))
            })?;
            rewrite(variant, value, path, visit)
        }
        _ => visit(desc, value, path),
    }
}

/// Top-level fields whose value differs after a rewrite.
fn changed_fields(old: &Value, new: Option<Value>) -> Map<String, Value> {
    let Some(Value::Object(new)) = new else {
        return Map::new();
    };
    new.into_iter()
        .filter(|(k, v)| old.get(k) != Some(v))
        .collect()
}

/// Local Markdown links (`](url)`) whose target is `url`, ignoring a trailing slash,
/// fragment, or query.
fn prose_links(source: &str, url: &str) -> Vec<(usize, String)> {
    let mut found = vec![];
    for (n, line) in source.lines().enumerate() {
        let mut rest = line;
        while let Some(at) = rest.find("](") {
            rest = &rest[at + 2..];
            let end = rest
                .find(|c: char| c == ')' || c.is_whitespace())
                .unwrap_or(rest.len());
            let target = &rest[..end];
            let bare = target.split(['#', '?']).next().unwrap_or_default();
            if bare.trim_end_matches('/') == url {
                found.push((n + 1, target.to_string()));
            }
        }
    }
    found
}

/// Files in an owned asset directory; refuses symlinks anywhere on or under the path.
fn owned_assets(root: &Path, rel: &str) -> Result<Vec<String>, ApiError> {
    fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) -> Result<(), ApiError> {
        let mut names: Vec<_> = std::fs::read_dir(dir)?
            .map(|e| e.map(|e| e.file_name()))
            .collect::<Result<_, _>>()?;
        names.sort();
        for name in names {
            let name = name
                .to_str()
                .ok_or_else(|| m::unsafe_path("asset path is not UTF-8"))?
                .to_string();
            let path = dir.join(&name);
            let child = format!("{rel}/{name}");
            let meta = std::fs::symlink_metadata(&path)?;
            if meta.file_type().is_symlink() {
                return Err(m::unsafe_path(
                    "owned asset directories cannot contain symlinks",
                ));
            } else if meta.is_dir() {
                walk(&path, &child, out)?;
            } else if meta.is_file() {
                out.push(child);
            } else {
                return Err(m::unsafe_path("owned assets must be regular files"));
            }
        }
        Ok(())
    }
    let dir = m::resolve_as(root, rel, Leaf::Dir)?;
    let mut out = vec![];
    if dir.exists() {
        walk(&dir, rel, &mut out)?;
    }
    Ok(out)
}

/// Read every entry of every structured collection (symlinked entries are refused).
async fn scan(
    root: &Path,
    collections: &[catalog::ContentCollection],
) -> Result<Vec<Scanned>, ApiError> {
    let mut out = vec![];
    for c in collections.iter().filter(|c| c.editable) {
        for e in catalog::entries(root, c).await? {
            let source = m::read_current(&m::resolve(root, &e.path)?)?;
            let parsed = catalog::build_entry(
                c.name.clone(),
                e.id.clone(),
                e.path.clone(),
                &c.kind,
                source.clone(),
                String::new(),
            );
            out.push(Scanned {
                collection: c.name.clone(),
                descriptor: c.fields.clone(),
                kind: c.kind.clone(),
                id: e.id,
                path: e.path,
                source,
                fields: parsed.fields,
            });
        }
    }
    Ok(out)
}

/// References to `collection/id` from other entries, their rewrites to `new_id` when
/// renaming, owned assets, and prose links to the entry's public URL.
async fn analyze(
    root: &Path,
    collections: &[catalog::ContentCollection],
    collection: &str,
    id: &str,
    new_id: Option<&str>,
) -> Result<Analysis, ApiError> {
    let url = public_url(collection, id);
    let mut impact = ContentImpact {
        references: vec![],
        assets: owned_assets(root, &asset_dir(collection, id))?,
        prose_links: vec![],
    };
    let mut rewrites = vec![];
    for entry in scan(root, collections).await? {
        if let (Some(url), true) = (&url, entry.kind != "json") {
            for (line, target) in prose_links(&entry.source, url) {
                impact.prose_links.push(ProseLink {
                    collection: entry.collection.clone(),
                    id: entry.id.clone(),
                    path: entry.path.clone(),
                    url: target,
                    line,
                });
            }
        }
        if entry.collection == collection && entry.id == id {
            continue; // Self-references move with the entry.
        }
        let mut hits: Vec<String> = vec![];
        let next = rewrite(
            &entry.descriptor,
            &entry.fields,
            &mut vec![],
            &mut |desc, value, path| {
                let hit = desc["type"] == "reference"
                    && desc["collection"] == collection
                    && value.as_str() == Some(id);
                if !hit {
                    return None;
                }
                let field = path.join(".");
                if !hits.contains(&field) {
                    hits.push(field);
                }
                new_id.map(Value::from)
            },
        );
        for field in hits {
            impact.references.push(Reference {
                collection: entry.collection.clone(),
                id: entry.id.clone(),
                path: entry.path.clone(),
                field,
            });
        }
        let fields = changed_fields(&entry.fields, next);
        if !fields.is_empty() {
            rewrites.push(Rewrite { entry, fields });
        }
    }
    Ok(Analysis { impact, rewrites })
}

/// `mutation::inspect_impact`: the dry run shown before a rename or delete is confirmed.
pub async fn inspect_impact(
    project_root: &Path,
    collection: &str,
    id: &str,
) -> Result<ContentImpact, ApiError> {
    let collections = catalog::list_collections(project_root).await?;
    let c = m::editable_in(&collections, collection)?;
    m::validate_id(id)?;
    m::read_current(&m::resolve(project_root, &m::entry_path(&c, id)?)?)?;
    Ok(analyze(project_root, &collections, &c.name, id, None)
        .await?
        .impact)
}

/// The singleton and the required home page cannot be renamed or deleted.
fn guard_lifecycle(c: &catalog::ContentCollection, id: &str, verb: &str) -> Result<(), ApiError> {
    if c.singleton.is_some() {
        return Err(m::protected(format!("{} cannot be {verb}", c.name)));
    }
    if c.name == "pages" && id == "home" {
        return Err(m::protected(format!("pages/home cannot be {verb}")));
    }
    Ok(())
}

/// The new slug's asset folder exists without an entry; never merge into it.
fn assets_taken(rel: &str) -> ApiError {
    ApiError::conflict(format!("{rel} already exists; choose another slug"))
        .with_reason("duplicate_slug")
}

/// Name the entry whose candidate failed helper validation.
fn in_entry(mut e: ApiError, collection: &str, id: &str) -> ApiError {
    e.message = format!("{collection}/{id}: {}", e.message);
    e
}

fn mode_of(path: &Path) -> Result<u32, ApiError> {
    Ok(std::fs::metadata(path)?.permissions().mode() & 0o7777)
}

/// Undo log for applied file-system steps, replayed in reverse on failure.
#[derive(Default)]
struct Journal(Vec<Undo>);

enum Undo {
    /// Restore `bytes` at `path` (a file that was overwritten or removed).
    Write {
        path: PathBuf,
        bytes: String,
        mode: u32,
    },
    /// Remove a file that was created.
    Remove(PathBuf),
    /// A directory moved from `to` to `from`; move it back.
    Move { from: PathBuf, to: PathBuf },
}

impl Journal {
    /// Roll back and return `error`, extended with every rollback step that failed.
    fn fail(self, error: ApiError) -> ApiError {
        let mut failures = vec![];
        for step in self.0.into_iter().rev() {
            let (path, result) = match step {
                Undo::Write { path, bytes, mode } => {
                    let r = m::write_atomic(&path, bytes.as_bytes(), mode);
                    (path, r)
                }
                Undo::Remove(path) => {
                    let r = std::fs::remove_file(&path).map_err(ApiError::from);
                    (path, r)
                }
                Undo::Move { from, to } => {
                    let r = std::fs::rename(&from, &to).map_err(ApiError::from);
                    (to, r)
                }
            };
            if let Err(e) = result {
                failures.push(format!("{}: {}", path.display(), e.message));
            }
        }
        if failures.is_empty() {
            return error;
        }
        ApiError::io(format!(
            "{}; rollback failed, check these paths: {}",
            error.message,
            failures.join("; ")
        ))
        .with_reason("rollback_failed")
        .with_details(json!(failures))
    }
}

fn emit_paths(state: &AppState, project: &str, root: &Path, paths: &[PathBuf]) {
    for path in paths {
        let kind = if root.join(path).exists() {
            "modify"
        } else {
            "remove"
        };
        state.events.emit(
            "fs.change",
            json!({ "projectId": project, "path": path, "kind": kind }),
        );
    }
}

/// Fail unless every file still has the bytes it was analyzed with.
fn verify_unchanged(root: &Path, files: &[(&str, &str)]) -> Result<(), ApiError> {
    for (rel, source) in files {
        if m::read_current(&m::resolve(root, rel)?)? != *source {
            return Err(m::stale());
        }
    }
    Ok(())
}

pub async fn rename_entry(
    state: &AppState,
    request: RenameEntry,
) -> Result<catalog::ContentEntry, ApiError> {
    let root = m::project_root(state, &request.project).await?;
    let lock = m::lock_for(&root);
    let _guard = lock.lock().await;
    let branch = m::require_content_change(&root, &request.branch).await?;
    let collections = catalog::list_collections(&root).await?;
    let c = m::editable_in(&collections, &request.collection)?;
    let (id, slug) = (request.id.as_str(), request.slug.as_str());
    guard_lifecycle(&c, id, "renamed")?;
    m::validate_id(id)?;
    m::validate_slug(&c.name, slug)?;
    if slug == id {
        return Err(m::invalid_slug("Choose a different slug"));
    }
    let (old_rel, new_rel) = (m::entry_path(&c, id)?, m::entry_path(&c, slug)?);
    let source = m::read_current(&m::resolve(&root, &old_rel)?)?;
    if catalog::hash(&source) != request.hash {
        return Err(m::stale());
    }
    if std::fs::symlink_metadata(m::resolve(&root, &new_rel)?).is_ok() {
        return Err(m::duplicate_slug(&c, slug));
    }
    let (old_assets, new_assets) = (asset_dir(&c.name, id), asset_dir(&c.name, slug));
    if std::fs::symlink_metadata(m::resolve_as(&root, &new_assets, Leaf::Dir)?).is_ok() {
        return Err(assets_taken(&new_assets));
    }
    let analysis = analyze(&root, &collections, &c.name, id, Some(slug)).await?;

    // Stage: the renamed entry's owned image paths and self-references, then every
    // reference holder, each validated by the helper before anything is written.
    let entry_dir = Path::new(&old_rel).parent().unwrap().to_str().unwrap();
    let old_prefix = format!("{}/", relative(entry_dir, &old_assets));
    let new_prefix = format!("{}/", relative(entry_dir, &new_assets));
    let parsed = catalog::build_entry(
        c.name.clone(),
        id.into(),
        old_rel.clone(),
        &c.kind,
        source.clone(),
        branch.clone(),
    );
    let moved = rewrite(
        &c.fields,
        &parsed.fields,
        &mut vec![],
        &mut |desc, value, _| {
            let text = value.as_str()?;
            if desc["type"] == "image" {
                return text
                    .strip_prefix(&old_prefix)
                    .map(|rest| Value::from(format!("{new_prefix}{rest}")));
            }
            (desc["type"] == "reference" && desc["collection"] == c.name.as_str() && text == id)
                .then(|| Value::from(slug))
        },
    );
    let fields = changed_fields(&parsed.fields, moved);
    let body = parsed
        .body
        .replace(&format!("]({old_prefix}"), &format!("]({new_prefix}"));
    let body = (c.kind != "json" && body != parsed.body).then_some(body);
    let new_source = helper::patch(&root, &c.name, &source, &fields, &[], body.as_deref())
        .await
        .map_err(|e| in_entry(e, &c.name, id))?;
    let mut staged = vec![];
    for r in &analysis.rewrites {
        let e = &r.entry;
        let next = helper::patch(&root, &e.collection, &e.source, &r.fields, &[], None)
            .await
            .map_err(|err| in_entry(err, &e.collection, &e.id))?;
        staged.push((e, next));
    }

    // Re-verify immediately before mutating: the helper ran outside git's view.
    m::require_content_change(&root, &request.branch).await?;
    let mut expected = vec![(old_rel.as_str(), source.as_str())];
    expected.extend(
        staged
            .iter()
            .map(|(e, _)| (e.path.as_str(), e.source.as_str())),
    );
    verify_unchanged(&root, &expected)?;
    if owned_assets(&root, &old_assets)? != analysis.impact.assets {
        return Err(m::stale());
    }
    let new_target = m::resolve(&root, &new_rel)?;
    let new_assets_dir = m::resolve_as(&root, &new_assets, Leaf::Dir)?;
    if std::fs::symlink_metadata(&new_target).is_ok() {
        return Err(m::duplicate_slug(&c, slug));
    }
    if std::fs::symlink_metadata(&new_assets_dir).is_ok() {
        return Err(assets_taken(&new_assets));
    }

    let old_target = m::resolve(&root, &old_rel)?;
    let mode = mode_of(&old_target)?;
    let mut journal = Journal::default();
    let applied = (|| {
        m::write_atomic(&new_target, new_source.as_bytes(), mode)?;
        journal.0.push(Undo::Remove(new_target.clone()));
        let from = m::resolve_as(&root, &old_assets, Leaf::Dir)?;
        if from.exists() {
            std::fs::rename(&from, &new_assets_dir)?;
            journal.0.push(Undo::Move {
                from: new_assets_dir.clone(),
                to: from,
            });
        }
        for (e, next) in &staged {
            let target = m::resolve(&root, &e.path)?;
            let mode = mode_of(&target)?;
            m::write_atomic(&target, next.as_bytes(), mode)?;
            journal.0.push(Undo::Write {
                path: target,
                bytes: e.source.clone(),
                mode,
            });
        }
        std::fs::remove_file(&old_target)?;
        journal.0.push(Undo::Write {
            path: old_target.clone(),
            bytes: source.clone(),
            mode,
        });
        Ok::<_, ApiError>(())
    })();
    if let Err(e) = applied {
        return Err(journal.fail(e));
    }

    let mut paths = vec![PathBuf::from(&old_rel), PathBuf::from(&new_rel)];
    for asset in &analysis.impact.assets {
        paths.push(PathBuf::from(asset));
        paths.push(PathBuf::from(format!(
            "{new_assets}{}",
            &asset[old_assets.len()..]
        )));
    }
    paths.extend(staged.iter().map(|(e, _)| PathBuf::from(&e.path)));
    let summary = format!("Rename {}/{id} to {slug}", c.name);
    let checkpoint = match git::checkpoint_paths(&root, &summary, &paths).await {
        Ok(cp) => cp,
        Err(e) => return Err(journal.fail(e)),
    };
    emit_paths(state, &request.project, &root, &paths);
    m::emit_checkpoint(state, &request.project, checkpoint);
    Ok(catalog::build_entry(
        c.name,
        slug.into(),
        new_rel,
        &c.kind,
        new_source,
        branch,
    ))
}

pub async fn delete_entry(state: &AppState, request: DeleteEntry) -> Result<(), ApiError> {
    let root = m::project_root(state, &request.project).await?;
    let lock = m::lock_for(&root);
    let _guard = lock.lock().await;
    m::require_content_change(&root, &request.branch).await?;
    let collections = catalog::list_collections(&root).await?;
    let c = m::editable_in(&collections, &request.collection)?;
    let id = request.id.as_str();
    guard_lifecycle(&c, id, "deleted")?;
    m::validate_id(id)?;
    let rel = m::entry_path(&c, id)?;
    let source = m::read_current(&m::resolve(&root, &rel)?)?;
    if catalog::hash(&source) != request.hash {
        return Err(m::stale());
    }
    let assets_rel = asset_dir(&c.name, id);
    let analysis = analyze(&root, &collections, &c.name, id, None).await?;
    if !analysis.impact.references.is_empty() {
        return Err(ApiError::conflict(format!(
            "{}/{id} is still referenced; remove those references first",
            c.name
        ))
        .with_reason("entry_referenced")
        .with_details(json!(analysis.impact.references)));
    }

    m::require_content_change(&root, &request.branch).await?;
    verify_unchanged(&root, &[(&rel, &source)])?;
    if owned_assets(&root, &assets_rel)? != analysis.impact.assets {
        return Err(m::stale());
    }

    let target = m::resolve(&root, &rel)?;
    let mode = mode_of(&target)?;
    let mut journal = Journal::default();
    let mut trash = None;
    let applied = (|| {
        let dir = m::resolve_as(&root, &assets_rel, Leaf::Dir)?;
        if dir.exists() {
            // Move aside rather than delete so a failed checkpoint can put it back.
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let aside = dir.with_file_name(format!(
                ".{id}.{}-{}.workbench-trash",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::rename(&dir, &aside)?;
            journal.0.push(Undo::Move {
                from: aside.clone(),
                to: dir,
            });
            trash = Some(aside);
        }
        std::fs::remove_file(&target)?;
        journal.0.push(Undo::Write {
            path: target.clone(),
            bytes: source.clone(),
            mode,
        });
        Ok::<_, ApiError>(())
    })();
    if let Err(e) = applied {
        return Err(journal.fail(e));
    }

    let mut paths = vec![PathBuf::from(&rel)];
    paths.extend(analysis.impact.assets.iter().map(PathBuf::from));
    let summary = format!("Delete {}/{id}", c.name);
    let checkpoint = match git::checkpoint_paths(&root, &summary, &paths).await {
        Ok(cp) => cp,
        Err(e) => return Err(journal.fail(e)),
    };
    if let Some(trash) = trash {
        if let Err(e) = std::fs::remove_dir_all(&trash) {
            // The delete is committed; only the set-aside copy remains on disk.
            eprintln!(
                "workbench: deleted {}/{id} but could not remove {}: {e}",
                c.name,
                trash.display()
            );
        }
    }
    emit_paths(state, &request.project, &root, &paths);
    m::emit_checkpoint(state, &request.project, checkpoint);
    Ok(())
}

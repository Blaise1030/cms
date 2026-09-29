//! Sandboxed, read-only file access for the UI (tree, file, raw blob).
//!
//! Every path is client-supplied and relative to the project dir. It is joined, canonicalised
//! (resolving symlinks and `..`), and must still live under the canonical project dir.
//! `.git` is never exposed.

use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::error::ApiError;

/// Largest file `read_file`/`read_raw` will return.
pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    File,
    Dir,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TreeEntry {
    pub name: String,
    /// Project-relative, `/`-separated.
    pub path: String,
    pub kind: EntryKind,
    /// Bytes; `None` for directories.
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileContent {
    pub path: String,
    pub size: u64,
    /// `None` when the file is not valid UTF-8 (`binary: true`).
    pub content: Option<String>,
    pub binary: bool,
}

/// Resolve `rel` inside `root`, returning `(canonical_root, canonical_target)`.
///
/// Errors: `BADREQ` for malformed paths, `SANDBOX` for escapes / `.git`, `ENOENT` when
/// missing.
async fn resolve(root: &Path, rel: &str) -> Result<(PathBuf, PathBuf), ApiError> {
    if rel.contains('\0') {
        return Err(ApiError::badreq("invalid path"));
    }
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() || rel_path.has_root() {
        return Err(ApiError::sandbox("absolute paths are not allowed"));
    }
    // Reject `..` lexically as well; the canonical check below is the real guard.
    if rel_path.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir
        )
    }) {
        return Err(ApiError::sandbox("path escapes the project"));
    }

    let canon_root = tokio::fs::canonicalize(root).await?;
    let canon = tokio::fs::canonicalize(canon_root.join(rel_path))
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ApiError::enoent(format!("no such path: {rel}"))
            } else {
                e.into()
            }
        })?;
    let inside = canon
        .strip_prefix(&canon_root)
        .map_err(|_| ApiError::sandbox("path escapes the project"))?;
    if inside.components().next().is_some_and(is_git_dir) {
        return Err(ApiError::sandbox(".git is not accessible"));
    }
    Ok((canon_root, canon))
}

fn is_git_dir(c: Component<'_>) -> bool {
    c.as_os_str() == ".git"
}

fn rel_string(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// List a directory (`""` = project root). Dirs first, then files, each sorted by name.
/// Symlinks are shown only if they resolve inside the project.
pub async fn tree(root: &Path, rel: &str) -> Result<Vec<TreeEntry>, ApiError> {
    let (canon_root, dir) = resolve(root, rel).await?;
    if !tokio::fs::metadata(&dir).await?.is_dir() {
        return Err(ApiError::badreq(format!("not a directory: {rel}")));
    }
    let at_root = dir == canon_root;
    let mut rd = tokio::fs::read_dir(&dir).await?;
    let mut entries = Vec::new();
    while let Some(ent) = rd.next_entry().await? {
        let name = ent.file_name().to_string_lossy().into_owned();
        if at_root && name == ".git" {
            continue;
        }
        // Follow symlinks, but only to targets inside the project.
        let Ok(target) = tokio::fs::canonicalize(ent.path()).await else {
            continue; // dangling link
        };
        if !target.starts_with(&canon_root) {
            continue;
        }
        let Ok(meta) = tokio::fs::metadata(&target).await else {
            continue;
        };
        let (kind, size) = if meta.is_dir() {
            (EntryKind::Dir, None)
        } else if meta.is_file() {
            (EntryKind::File, Some(meta.len()))
        } else {
            continue;
        };
        entries.push(TreeEntry {
            path: rel_string(&canon_root, &dir.join(&name)),
            name,
            kind,
            size,
        });
    }
    entries.sort_by(|a, b| {
        (a.kind == EntryKind::File, &a.name).cmp(&(b.kind == EntryKind::File, &b.name))
    });
    Ok(entries)
}

async fn open_file(root: &Path, rel: &str) -> Result<(PathBuf, u64), ApiError> {
    let (_, path) = resolve(root, rel).await?;
    let meta = tokio::fs::metadata(&path).await?;
    if !meta.is_file() {
        return Err(ApiError::badreq(format!("not a file: {rel}")));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(ApiError::badreq(format!(
            "file too large ({} bytes, max {MAX_FILE_BYTES})",
            meta.len()
        ))
        .with_status(413));
    }
    Ok((path, meta.len()))
}

/// Read a file as UTF-8 text (or flag it binary).
pub async fn read_file(root: &Path, rel: &str) -> Result<FileContent, ApiError> {
    let (path, size) = open_file(root, rel).await?;
    let bytes = tokio::fs::read(&path).await?;
    let content = String::from_utf8(bytes).ok();
    Ok(FileContent {
        path: rel.trim_matches('/').to_string(),
        size,
        binary: content.is_none(),
        content,
    })
}

/// Read a file's raw bytes plus a best-effort content type.
pub async fn read_raw(root: &Path, rel: &str) -> Result<(Vec<u8>, &'static str), ApiError> {
    let (path, _) = open_file(root, rel).await?;
    let bytes = tokio::fs::read(&path).await?;
    Ok((bytes, content_type(&path)))
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("pdf") => "application/pdf",
        Some("json") => "application/json",
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("md" | "mdx" | "txt" | "astro" | "ts" | "js" | "mjs" | "toml" | "yaml" | "yml") => {
            "text/plain; charset=utf-8"
        }
        _ => "application/octet-stream",
    }
}

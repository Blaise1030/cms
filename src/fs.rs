//! Sandboxed, read-only file access for the UI (tree, file, raw blob).
//!
//! Every path is client-supplied and relative to the project dir. Defence in depth:
//!
//! 1. **Lexical check** — no absolute paths, `..`, NULs, or VCS-metadata components.
//! 2. **Canonicalise-and-contain** — the joined path is canonicalised (resolving symlinks)
//!    and must still live under the canonical project dir.
//! 3. **Verify what was actually opened** — the path is then opened with `O_NOFOLLOW`, and
//!    the *open file descriptor's* real path is re-checked for containment. A symlink swapped
//!    in between steps 2 and 3 is therefore either refused (`ELOOP`) or caught by the fd check.
//!    All metadata (`is_file`, size) comes from the fd, and reads are capped at
//!    [`MAX_FILE_BYTES`] regardless of what `fstat` said, so growth races can't exceed it.
//!
//! VCS metadata directories (`.git`, `.hg`, `.svn`, …) are refused at *any* depth, so nested
//! repositories are not exposed, and they are hidden from listings.

use std::ffi::{CStr, CString, OsStr};
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::error::ApiError;

/// Largest file `read_file`/`read_raw` will return.
pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;

/// Version-control metadata directory names, blocked at every depth (case-insensitive).
const VCS_DIRS: &[&str] = &[".git", ".hg", ".svn", ".bzr", "_darcs", ".fossil", ".jj"];

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

fn is_vcs_name(name: &OsStr) -> bool {
    let b = name.as_bytes();
    VCS_DIRS
        .iter()
        .any(|v| v.as_bytes().eq_ignore_ascii_case(b))
}

fn has_vcs_component(p: &Path) -> bool {
    p.components().any(|c| is_vcs_name(c.as_os_str()))
}

fn io_err(e: std::io::Error, what: &str) -> ApiError {
    match e.raw_os_error() {
        // O_NOFOLLOW on a symlink: the target changed under us or is a link we refuse to open.
        Some(libc::ELOOP) => ApiError::sandbox("path escapes the project"),
        Some(libc::ENOTDIR) => ApiError::badreq(format!("not a directory: {what}")),
        _ if e.kind() == std::io::ErrorKind::NotFound => {
            ApiError::enoent(format!("no such path: {what}"))
        }
        _ => e.into(),
    }
}

/// Real filesystem path of an open descriptor (as the kernel sees it right now).
fn fd_path(fd: &impl AsRawFd) -> std::io::Result<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_link(format!("/proc/self/fd/{}", fd.as_raw_fd()))
    }
    #[cfg(target_os = "macos")]
    {
        let mut buf = [0u8; libc::PATH_MAX as usize];
        // SAFETY: F_GETPATH writes a NUL-terminated path of at most PATH_MAX bytes into `buf`.
        let rc = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETPATH, buf.as_mut_ptr()) };
        if rc == -1 {
            return Err(std::io::Error::last_os_error());
        }
        let cstr = CStr::from_bytes_until_nul(&buf)
            .map_err(|_| std::io::Error::other("unterminated F_GETPATH result"))?;
        Ok(PathBuf::from(OsStr::from_bytes(cstr.to_bytes())))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = fd;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "cannot resolve fd path on this platform",
        ))
    }
}

/// Assert that an open descriptor really lives inside `canon_root` (and outside VCS dirs).
/// Returns its root-relative path.
fn verify_fd(fd: &impl AsRawFd, canon_root: &Path) -> Result<PathBuf, ApiError> {
    let real = fd_path(fd).map_err(|e| ApiError::io(format!("cannot verify opened path: {e}")))?;
    let rel = real
        .strip_prefix(canon_root)
        .map_err(|_| ApiError::sandbox("path escapes the project"))?;
    if has_vcs_component(rel) {
        return Err(ApiError::sandbox(
            "version-control metadata is not accessible",
        ));
    }
    Ok(rel.to_path_buf())
}

/// Lexical validation + canonicalisation. Returns `(canonical_root, canonical_target)`.
fn resolve(root: &Path, rel: &str) -> Result<(PathBuf, PathBuf), ApiError> {
    if rel.contains('\0') {
        return Err(ApiError::badreq("invalid path"));
    }
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() || rel_path.has_root() {
        return Err(ApiError::sandbox("absolute paths are not allowed"));
    }
    if rel_path.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir
        )
    }) {
        return Err(ApiError::sandbox("path escapes the project"));
    }
    if has_vcs_component(rel_path) {
        return Err(ApiError::sandbox(
            "version-control metadata is not accessible",
        ));
    }

    let canon_root = std::fs::canonicalize(root).map_err(|e| io_err(e, "project"))?;
    let canon = std::fs::canonicalize(canon_root.join(rel_path)).map_err(|e| io_err(e, rel))?;
    let inside = canon
        .strip_prefix(&canon_root)
        .map_err(|_| ApiError::sandbox("path escapes the project"))?;
    // A contained symlink may still point into a VCS dir.
    if has_vcs_component(inside) {
        return Err(ApiError::sandbox(
            "version-control metadata is not accessible",
        ));
    }
    Ok((canon_root, canon))
}

/// Open `path` (already canonical) without following a final symlink, then verify the
/// descriptor's real location. `dir` selects `O_DIRECTORY`.
fn open_verified(canon_root: &Path, path: &Path, dir: bool) -> Result<(File, PathBuf), ApiError> {
    let mut flags = libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC;
    if dir {
        flags |= libc::O_DIRECTORY;
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(flags)
        .open(path)
        .map_err(|e| {
            // macOS reports ENOTDIR (not ELOOP) for O_DIRECTORY|O_NOFOLLOW on a symlink.
            let is_link = std::fs::symlink_metadata(path).is_ok_and(|m| m.is_symlink());
            if is_link {
                ApiError::sandbox("path escapes the project")
            } else {
                io_err(e, &path.display().to_string())
            }
        })?;
    let rel = verify_fd(&file, canon_root)?;
    Ok((file, rel))
}

fn rel_string(p: &Path) -> String {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// List a directory (`""` = project root). Dirs first, then files, each sorted by name.
/// Symlinks are shown only if they resolve inside the project; VCS dirs are never shown.
pub async fn tree(root: &Path, rel: &str) -> Result<Vec<TreeEntry>, ApiError> {
    let (root, rel) = (root.to_path_buf(), rel.to_string());
    blocking(move || tree_sync(&root, &rel)).await
}

fn tree_sync(root: &Path, rel: &str) -> Result<Vec<TreeEntry>, ApiError> {
    let (canon_root, dir) = resolve(root, rel)?;
    let (dir_file, dir_rel) = open_verified(&canon_root, &dir, true)?;
    let dirfd = dir_file.as_raw_fd();

    // fdopendir takes ownership of the fd it is given, so hand it a duplicate.
    // SAFETY: `dirfd` is a valid open descriptor for the duration of this call.
    let dup = unsafe { libc::fcntl(dirfd, libc::F_DUPFD_CLOEXEC, 0) };
    if dup == -1 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: `dup` is a fresh descriptor we own; on fdopendir success DIR owns it.
    let dirp = unsafe { libc::fdopendir(dup) };
    if dirp.is_null() {
        let e = std::io::Error::last_os_error();
        // SAFETY: fdopendir failed, so `dup` is still ours to close.
        drop(unsafe { OwnedFd::from_raw_fd(dup) });
        return Err(e.into());
    }

    let mut names: Vec<std::ffi::OsString> = Vec::new();
    loop {
        // SAFETY: `dirp` is a valid DIR*; readdir returns NULL at end (or on error).
        let ent = unsafe { libc::readdir(dirp) };
        if ent.is_null() {
            break;
        }
        // SAFETY: d_name is a NUL-terminated C string within the dirent.
        let name = unsafe { CStr::from_ptr((*ent).d_name.as_ptr()) }.to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        names.push(OsStr::from_bytes(name).to_os_string());
    }
    // SAFETY: `dirp` is valid and closed exactly once (also closes `dup`).
    unsafe { libc::closedir(dirp) };

    let mut entries = Vec::new();
    for name in names {
        if is_vcs_name(&name) {
            continue;
        }
        let Some((kind, size)) = classify(dirfd, &name, &canon_root) else {
            continue;
        };
        let name_s = name.to_string_lossy().into_owned();
        let path = rel_string(&dir_rel.join(&name));
        entries.push(TreeEntry {
            name: name_s,
            path,
            kind,
            size,
        });
    }
    entries.sort_by(|a, b| {
        (a.kind == EntryKind::File, &a.name).cmp(&(b.kind == EntryKind::File, &b.name))
    });
    Ok(entries)
}

/// Kind/size of `name` inside `dirfd`. Plain entries use `fstatat(NOFOLLOW)`; symlinks are
/// opened and verified via their descriptor. `None` = hide the entry.
fn classify(dirfd: i32, name: &OsStr, canon_root: &Path) -> Option<(EntryKind, Option<u64>)> {
    let cname = CString::new(name.as_bytes()).ok()?;
    // SAFETY: zeroed `stat` is a valid out-parameter; `cname` is NUL-terminated.
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::fstatat(dirfd, cname.as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) };
    if rc != 0 {
        return None;
    }
    let fmt = st.st_mode & libc::S_IFMT;
    if fmt == libc::S_IFLNK {
        // SAFETY: valid dirfd + NUL-terminated name; the new fd is wrapped immediately.
        let fd = unsafe {
            libc::openat(
                dirfd,
                cname.as_ptr(),
                libc::O_RDONLY | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };
        if fd == -1 {
            return None; // dangling
        }
        // SAFETY: `fd` was just returned by openat and is owned here.
        let file = File::from(unsafe { OwnedFd::from_raw_fd(fd) });
        verify_fd(&file, canon_root).ok()?;
        let meta = file.metadata().ok()?;
        return if meta.is_dir() {
            Some((EntryKind::Dir, None))
        } else if meta.is_file() {
            Some((EntryKind::File, Some(meta.len())))
        } else {
            None
        };
    }
    match fmt {
        libc::S_IFDIR => Some((EntryKind::Dir, None)),
        libc::S_IFREG => Some((EntryKind::File, Some(st.st_size as u64))),
        _ => None,
    }
}

/// Open a regular file safely and return it with its verified size. Rejects >5 MB.
fn open_file(root: &Path, rel: &str) -> Result<(File, u64), ApiError> {
    let (canon_root, path) = resolve(root, rel)?;
    let (file, _) = open_verified(&canon_root, &path, false)?;
    // fstat on the descriptor we will read from — not a second path lookup.
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(ApiError::badreq(format!("not a file: {rel}")));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(too_large(meta.len()));
    }
    Ok((file, meta.len()))
}

fn too_large(size: u64) -> ApiError {
    ApiError::badreq(format!(
        "file too large ({size} bytes, max {MAX_FILE_BYTES})"
    ))
    .with_status(413)
}

/// Read at most `max` bytes; more than that (e.g. the file grew after `fstat`) is an error.
fn read_limited(mut r: impl Read, max: u64) -> Result<Vec<u8>, ApiError> {
    let mut buf = Vec::new();
    r.by_ref().take(max + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > max {
        return Err(too_large(buf.len() as u64));
    }
    Ok(buf)
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| ApiError::io(format!("file task failed: {e}")))?
}

/// Read a file as UTF-8 text (or flag it binary).
pub async fn read_file(root: &Path, rel: &str) -> Result<FileContent, ApiError> {
    let (root, rel) = (root.to_path_buf(), rel.to_string());
    blocking(move || {
        let (file, _) = open_file(&root, &rel)?;
        let bytes = read_limited(file, MAX_FILE_BYTES)?;
        let size = bytes.len() as u64;
        let content = String::from_utf8(bytes).ok();
        Ok(FileContent {
            path: rel.trim_matches('/').to_string(),
            size,
            binary: content.is_none(),
            content,
        })
    })
    .await
}

/// Read a file's raw bytes plus a best-effort content type.
pub async fn read_raw(root: &Path, rel: &str) -> Result<(Vec<u8>, &'static str), ApiError> {
    let (root, rel) = (root.to_path_buf(), rel.to_string());
    blocking(move || {
        let (file, _) = open_file(&root, &rel)?;
        let bytes = read_limited(file, MAX_FILE_BYTES)?;
        Ok((bytes, content_type(Path::new(&rel))))
    })
    .await
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn vcs_names_match_case_insensitively() {
        for n in [".git", ".GIT", ".Hg", ".svn", ".bzr", "_darcs", ".jj"] {
            assert!(is_vcs_name(OsStr::new(n)), "{n}");
        }
        for n in ["git", ".gitignore", ".github", "x.git", ""] {
            assert!(!is_vcs_name(OsStr::new(n)), "{n}");
        }
        assert!(has_vcs_component(Path::new("a/b/.git/config")));
        assert!(!has_vcs_component(Path::new("a/.github/x")));
    }

    /// Simulates a symlink swapped in *after* canonicalisation: `open_verified` is handed a
    /// path whose final component is now a link out of the sandbox.
    #[test]
    fn swapped_final_symlink_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("s.txt"), "secret").unwrap();
        let canon_root = root.path().canonicalize().unwrap();
        let link = canon_root.join("f.txt");
        symlink(outside.path().join("s.txt"), &link).unwrap();
        let err = open_verified(&canon_root, &link, false).unwrap_err();
        assert_eq!(err.code.as_str(), "SANDBOX", "{err}");
    }

    /// Same, but the swap happened on an *intermediate* directory (which `O_NOFOLLOW` does not
    /// cover): the descriptor's real path is what catches it.
    #[test]
    fn swapped_intermediate_symlink_is_caught_by_fd_check() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("s.txt"), "secret").unwrap();
        let canon_root = root.path().canonicalize().unwrap();
        symlink(outside.path(), canon_root.join("d")).unwrap();
        let err = open_verified(&canon_root, &canon_root.join("d/s.txt"), false).unwrap_err();
        assert_eq!(err.code.as_str(), "SANDBOX", "{err}");
        let err = open_verified(&canon_root, &canon_root.join("d"), true).unwrap_err();
        assert_eq!(err.code.as_str(), "SANDBOX", "{err}");
    }

    #[test]
    fn swap_into_vcs_dir_is_caught_by_fd_check() {
        let root = tempfile::tempdir().unwrap();
        let canon_root = root.path().canonicalize().unwrap();
        std::fs::create_dir_all(canon_root.join("sub/.git")).unwrap();
        std::fs::write(canon_root.join("sub/.git/config"), "x").unwrap();
        let err =
            open_verified(&canon_root, &canon_root.join("sub/.git/config"), false).unwrap_err();
        assert_eq!(err.code.as_str(), "SANDBOX", "{err}");
    }

    #[test]
    fn reads_are_capped_even_if_the_file_grew_after_fstat() {
        let data = [7u8; 11];
        assert_eq!(read_limited(&data[..], 11).unwrap().len(), 11);
        let err = read_limited(&data[..], 10).unwrap_err();
        assert_eq!(err.http_status(), 413);
    }

    #[test]
    fn fifo_does_not_block_and_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let fifo = root.path().join("p");
        let c = CString::new(fifo.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        let err = open_file(root.path(), "p").unwrap_err();
        assert_eq!(err.code.as_str(), "BADREQ", "{err}");
    }
}

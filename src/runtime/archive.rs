//! Safe extraction for the archives the runtime lock pins.
//!
//! Every entry is checked before anything is written: absolute paths, `..`,
//! links, devices, duplicate paths, unexpected roots and oversized trees are
//! rejected. Only the paths listed in [`ExtractOptions::executables`] are made
//! executable; every other regular file gets `0o644`.

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};

use super::{Result, RuntimeError};

/// Default maximum number of extracted files.
pub const DEFAULT_MAX_FILES: usize = 20_000;
/// Default maximum total uncompressed bytes (2 GiB).
pub const DEFAULT_MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Default maximum size of one extracted file (512 MiB).
pub const DEFAULT_MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;

/// Archive container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    TarGz,
    Zip,
}

impl ArchiveFormat {
    /// Guess from a file name or URL path.
    pub fn from_path(path: &Path) -> Self {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if name.ends_with(".zip") {
            Self::Zip
        } else {
            Self::TarGz
        }
    }

    /// Parse the lock file's `format` value.
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "tar.gz" | "tgz" | "tar" => Some(Self::TarGz),
            "zip" => Some(Self::Zip),
            _ => None,
        }
    }
}

/// What to do with symbolic link entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymlinkPolicy {
    /// Reject the archive (the default).
    Reject,
    /// Ignore the entry. Used for Node's `bin/npm` and `bin/npx` links, which
    /// the descriptor replaces with real wrapper scripts.
    Skip,
}

/// Size and entry limits for one extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    pub max_files: usize,
    pub max_total_bytes: u64,
    pub max_file_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_files: DEFAULT_MAX_FILES,
            max_total_bytes: DEFAULT_MAX_TOTAL_BYTES,
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
        }
    }
}

/// How to unpack one runtime archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractOptions {
    /// The only top-level directory the archive may contain.
    pub expected_root: Option<String>,
    /// Leading path components to remove after checking `expected_root`.
    pub strip_components: usize,
    /// Paths (relative to the destination root) that get `0o755`.
    pub executables: Vec<String>,
    /// Link handling; defaults to [`SymlinkPolicy::Reject`].
    pub symlinks: SymlinkPolicy,
    /// Size and entry limits.
    pub limits: Limits,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            expected_root: None,
            strip_components: 0,
            executables: Vec::new(),
            symlinks: SymlinkPolicy::Reject,
            limits: Limits::default(),
        }
    }
}

impl ExtractOptions {
    /// Options for a normal runtime archive with one documented executable.
    pub fn new(expected_root: Option<impl Into<String>>, executable: impl Into<String>) -> Self {
        let has_root = expected_root.is_some();
        Self {
            expected_root: expected_root.map(Into::into),
            strip_components: usize::from(has_root),
            executables: vec![executable.into()],
            ..Self::default()
        }
    }
}

/// Unpack `archive` into `dest`, guessing the container from its name.
pub fn extract(archive: &Path, dest: &Path, options: &ExtractOptions) -> Result<()> {
    extract_format(archive, dest, options, ArchiveFormat::from_path(archive))
}

/// Unpack `archive` into `dest` with an explicit container format. Runtime
/// downloads keep a `.partial` suffix, so the format cannot be guessed from
/// the file name there.
pub fn extract_format(
    archive: &Path,
    dest: &Path,
    options: &ExtractOptions,
    format: ArchiveFormat,
) -> Result<()> {
    match format {
        ArchiveFormat::TarGz => extract_tar_gz(archive, dest, options),
        ArchiveFormat::Zip => extract_zip(archive, dest, options),
    }
}

/// Unpack a gzipped tar archive.
pub fn extract_tar_gz(archive: &Path, dest: &Path, options: &ExtractOptions) -> Result<()> {
    let file = File::open(archive)?;
    let decoder = GzDecoder::new(file);
    let mut tar = tar::Archive::new(decoder);
    let mut budget = Budget::new(options);
    std::fs::create_dir_all(dest)?;

    for entry in tar.entries()? {
        let mut entry = entry?;
        let header = entry.header();
        let raw = entry.path()?.into_owned();
        let entry_type = header.entry_type();
        if entry_type.is_symlink() {
            match options.symlinks {
                SymlinkPolicy::Skip => continue,
                SymlinkPolicy::Reject => {
                    return Err(RuntimeError::Archive(format!(
                        "symbolic link is not allowed: {}",
                        raw.display()
                    )))
                }
            }
        }
        if entry_type.is_hard_link() {
            return Err(RuntimeError::Archive(format!(
                "hard link is not allowed: {}",
                raw.display()
            )));
        }
        if entry_type.is_dir() {
            if let Some(relative) = budget.normalize(&raw)? {
                std::fs::create_dir_all(dest.join(relative))?;
            }
            continue;
        }
        if !entry_type.is_file() {
            return Err(RuntimeError::Archive(format!(
                "special file is not allowed: {}",
                raw.display()
            )));
        }
        let Some(relative) = budget.normalize(&raw)? else {
            continue;
        };
        budget.reserve(&relative, header.size()?)?;
        write_entry(&mut entry, dest, &relative, &budget)?;
    }

    budget.finish(dest, options)
}

/// Unpack a zip archive.
pub fn extract_zip(archive: &Path, dest: &Path, options: &ExtractOptions) -> Result<()> {
    let file = File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|error| RuntimeError::Archive(format!("invalid zip: {error}")))?;
    let mut budget = Budget::new(options);
    std::fs::create_dir_all(dest)?;

    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|error| RuntimeError::Archive(format!("invalid zip entry: {error}")))?;
        let raw = entry
            .enclosed_name()
            .ok_or_else(|| {
                RuntimeError::Archive(format!(
                    "unsafe zip path: {}",
                    entry
                        .name()
                        .map(|name| name.to_string())
                        .unwrap_or_default()
                ))
            })?
            .to_path_buf();
        let mode = entry.unix_mode();
        if let Some(mode) = mode {
            let kind = mode & 0o170000;
            if kind == 0o120000 {
                match options.symlinks {
                    SymlinkPolicy::Skip => continue,
                    SymlinkPolicy::Reject => {
                        return Err(RuntimeError::Archive(format!(
                            "symbolic link is not allowed: {}",
                            raw.display()
                        )))
                    }
                }
            }
            if kind != 0 && kind != 0o040000 && kind != 0o100000 {
                return Err(RuntimeError::Archive(format!(
                    "special file is not allowed: {}",
                    raw.display()
                )));
            }
        }
        if entry.is_dir() {
            if let Some(relative) = budget.normalize(&raw)? {
                std::fs::create_dir_all(dest.join(relative))?;
            }
            continue;
        }
        let Some(relative) = budget.normalize(&raw)? else {
            continue;
        };
        budget.reserve(&relative, entry.size())?;
        write_entry(&mut entry, dest, &relative, &budget)?;
    }

    budget.finish(dest, options)
}

/// Shared bookkeeping for both formats.
struct Budget<'a> {
    options: &'a ExtractOptions,
    files: HashSet<PathBuf>,
    file_count: usize,
    total_bytes: u64,
}

impl<'a> Budget<'a> {
    fn new(options: &'a ExtractOptions) -> Self {
        Self {
            options,
            files: HashSet::new(),
            file_count: 0,
            total_bytes: 0,
        }
    }

    /// Validate, strip and de-duplicate one archive path. `None` means the
    /// entry is the archive's root or a stripped-away directory.
    fn normalize(&mut self, raw: &Path) -> Result<Option<PathBuf>> {
        if raw.is_absolute() || raw.has_root() {
            return Err(RuntimeError::Archive(format!(
                "absolute path is not allowed: {}",
                raw.display()
            )));
        }
        let mut parts = Vec::new();
        for component in raw.components() {
            match component {
                Component::Normal(part) => parts.push(part.to_os_string()),
                Component::CurDir => {}
                Component::ParentDir => {
                    return Err(RuntimeError::Archive(format!(
                        "path escapes the install directory: {}",
                        raw.display()
                    )))
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(RuntimeError::Archive(format!(
                        "absolute path is not allowed: {}",
                        raw.display()
                    )))
                }
            }
        }
        if parts.is_empty() {
            return Ok(None);
        }
        if let Some(root) = &self.options.expected_root {
            if parts.first().and_then(|part| part.to_str()) != Some(root.as_str()) {
                return Err(RuntimeError::Archive(format!(
                    "unexpected archive root in {}: expected {root}",
                    raw.display()
                )));
            }
        }
        if parts.len() <= self.options.strip_components {
            return Ok(None);
        }
        let relative: PathBuf = parts
            .split_off(self.options.strip_components)
            .into_iter()
            .collect();
        if relative.as_os_str().is_empty() {
            return Ok(None);
        }
        Ok(Some(relative))
    }

    fn reserve(&mut self, relative: &Path, size: u64) -> Result<()> {
        if size > self.options.limits.max_file_bytes {
            return Err(RuntimeError::Archive(format!(
                "{} is larger than the per-file limit",
                relative.display()
            )));
        }
        let total = self.total_bytes.saturating_add(size);
        if total > self.options.limits.max_total_bytes {
            return Err(RuntimeError::Archive(
                "archive is larger than the total size limit".into(),
            ));
        }
        if self.file_count >= self.options.limits.max_files {
            return Err(RuntimeError::Archive(
                "archive contains too many files".into(),
            ));
        }
        if !self.files.insert(relative.to_path_buf()) {
            return Err(RuntimeError::Archive(format!(
                "duplicate path in archive: {}",
                relative.display()
            )));
        }
        self.file_count += 1;
        self.total_bytes = total;
        Ok(())
    }

    fn is_executable(&self, relative: &Path) -> bool {
        self.options
            .executables
            .iter()
            .any(|executable| Path::new(executable) == relative)
    }

    fn finish(&self, dest: &Path, options: &ExtractOptions) -> Result<()> {
        for executable in &options.executables {
            let relative = checked_relative(executable)?;
            let path = dest.join(&relative);
            if !path.is_file() {
                return Err(RuntimeError::Archive(format!(
                    "documented executable is missing from the archive: {}",
                    relative.display()
                )));
            }
            set_mode(&path, 0o755)?;
        }
        if !dest.is_dir() {
            return Err(RuntimeError::Archive("install directory is missing".into()));
        }
        Ok(())
    }
}

/// Validate a caller-provided relative path.
fn checked_relative(value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err(RuntimeError::Archive(format!(
            "unsafe relative path: {value}"
        )));
    }
    Ok(path.to_path_buf())
}

fn write_entry(
    reader: &mut impl Read,
    dest: &Path,
    relative: &Path,
    budget: &Budget<'_>,
) -> Result<()> {
    let path = dest.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let limit = budget.options.limits.max_file_bytes.saturating_add(1);
    let written = io::copy(&mut reader.take(limit), &mut file)?;
    if written > budget.options.limits.max_file_bytes {
        let _ = std::fs::remove_file(&path);
        return Err(RuntimeError::Archive(format!(
            "{} exceeded its declared size",
            relative.display()
        )));
    }
    file.flush()?;
    drop(file);
    set_mode(
        &path,
        if budget.is_executable(relative) {
            0o755
        } else {
            0o644
        },
    )?;
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

/// Read a gzip stream to the end, used by tests and callers that need the
/// uncompressed size without extracting.
pub fn extract_tar_gz_size(archive: &Path) -> Result<u64> {
    let file = File::open(archive)?;
    let mut decoder = GzDecoder::new(file);
    let mut buffer = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let read = decoder.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read as u64);
    }
    Ok(total)
}

/// Rewind a file to its start; keeps the `Seek` import meaningful for future
/// partial-download helpers.
/// Serialised record of an archive entry, handy in tests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntrySummary {
    pub path: String,
    pub size: u64,
    pub executable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use tar::{Builder, EntryType, Header};

    struct BuilderHelper {
        path: PathBuf,
        tar: Builder<GzEncoder<File>>,
    }

    impl BuilderHelper {
        fn new(path: PathBuf) -> Self {
            let file = File::create(&path).unwrap();
            let encoder = GzEncoder::new(file, Compression::fast());
            Self {
                path,
                tar: Builder::new(encoder),
            }
        }

        fn add(&mut self, name: &str, data: &[u8]) {
            let mut header = Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            self.tar.append_data(&mut header, name, data).unwrap();
        }

        /// Append a header with a raw name the tar crate would refuse to set,
        /// so the extractor's own path checks are exercised.
        fn add_raw(&mut self, name: &str, data: &[u8]) {
            let mut header = Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            let bytes = name.as_bytes();
            header.as_old_mut().name.fill(0);
            header.as_old_mut().name[..bytes.len()].copy_from_slice(bytes);
            header.set_cksum();
            self.tar.append(&header, data).unwrap();
        }

        fn add_symlink(&mut self, name: &str, target: &str) {
            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Symlink);
            header.set_size(0);
            header.set_mode(0o777);
            header.set_link_name(target).unwrap();
            header.set_cksum();
            self.tar
                .append_data(&mut header, name, io::empty())
                .unwrap();
        }

        fn add_type(&mut self, name: &str, entry_type: EntryType) {
            let mut header = Header::new_gnu();
            header.set_entry_type(entry_type);
            header.set_size(0);
            header.set_mode(0o644);
            header.set_cksum();
            self.tar
                .append_data(&mut header, name, io::empty())
                .unwrap();
        }

        fn finish(self) -> PathBuf {
            self.tar.into_inner().unwrap().finish().unwrap();
            self.path
        }
    }

    fn tar_with(entries: &[(&str, &[u8])]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.tar.gz");
        let mut builder = BuilderHelper::new(path.clone());
        for (name, data) in entries {
            builder.add(name, data);
        }
        builder.finish();
        (dir, path)
    }

    fn options(executable: &str) -> ExtractOptions {
        ExtractOptions::new(Some("package"), executable)
    }

    #[test]
    fn extracts_tar_gz_and_sets_only_the_executable() {
        let (dir, archive) = tar_with(&[
            ("package/bin/tool", b"#!/bin/sh\necho hi\n"),
            ("package/README.md", b"read me"),
        ]);
        let dest = dir.path().join("out");
        extract(&archive, &dest, &options("bin/tool")).unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.join("bin/tool")).unwrap(),
            "#!/bin/sh\necho hi\n"
        );
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(dest.join("bin/tool"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        assert_eq!(
            std::fs::metadata(dest.join("README.md"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o644
        );
    }

    #[test]
    fn rejects_parent_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("traversal.tar.gz");
        let mut builder = BuilderHelper::new(path.clone());
        builder.add("package/bin/tool", b"x");
        builder.add_raw("package/../escape", b"x");
        let archive = builder.finish();
        let err = extract(&archive, &dir.path().join("out"), &options("bin/tool")).unwrap_err();
        assert!(err.to_string().contains("escapes"));
    }

    #[test]
    fn rejects_absolute_paths() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("absolute.tar.gz");
        let mut builder = BuilderHelper::new(path.clone());
        builder.add("package/bin/tool", b"x");
        builder.add_raw("/tmp/escape", b"x");
        let archive = builder.finish();
        let err = extract(&archive, &dir.path().join("out"), &options("bin/tool")).unwrap_err();
        assert!(err.to_string().contains("absolute"));
    }

    #[test]
    fn rejects_symlinks_by_default_and_can_skip_them() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("links.tar.gz");
        let mut builder = BuilderHelper::new(path.clone());
        builder.add("package/bin/tool", b"x");
        builder.add_symlink("package/bin/alias", "tool");
        let path = builder.finish();
        let err = extract(&path, &dir.path().join("out"), &options("bin/tool")).unwrap_err();
        assert!(err.to_string().contains("symbolic link"));

        let mut skip = options("bin/tool");
        skip.symlinks = SymlinkPolicy::Skip;
        extract(&path, &dir.path().join("skip"), &skip).unwrap();
        assert!(!dir.path().join("skip/bin/alias").exists());
    }

    #[test]
    fn rejects_hard_links() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hard.tar.gz");
        let mut builder = BuilderHelper::new(path.clone());
        builder.add("package/bin/tool", b"x");
        builder.add_type("package/bin/hard", EntryType::Link);
        let path = builder.finish();
        let err = extract(&path, &dir.path().join("out"), &options("bin/tool")).unwrap_err();
        assert!(err.to_string().contains("hard link"));
    }

    #[test]
    fn rejects_device_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("device.tar.gz");
        let mut builder = BuilderHelper::new(path.clone());
        builder.add("package/bin/tool", b"x");
        builder.add_type("package/dev", EntryType::Char);
        let path = builder.finish();
        let err = extract(&path, &dir.path().join("out"), &options("bin/tool")).unwrap_err();
        assert!(err.to_string().contains("special file"));
    }

    #[test]
    fn rejects_duplicate_paths() {
        let (dir, archive) =
            tar_with(&[("package/bin/tool", b"one"), ("package/bin/tool", b"two")]);
        let err = extract(&archive, &dir.path().join("out"), &options("bin/tool")).unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn rejects_unexpected_roots() {
        let (dir, archive) = tar_with(&[("other/bin/tool", b"x")]);
        let err = extract(&archive, &dir.path().join("out"), &options("bin/tool")).unwrap_err();
        assert!(err.to_string().contains("unexpected archive root"));
    }

    #[test]
    fn rejects_oversized_trees() {
        let (dir, archive) = tar_with(&[("package/bin/tool", b"0123456789")]);
        let mut small = options("bin/tool");
        small.limits.max_total_bytes = 5;
        let err = extract(&archive, &dir.path().join("out"), &small).unwrap_err();
        assert!(err.to_string().contains("total size"));
    }

    #[test]
    fn requires_the_documented_executable() {
        let (dir, archive) = tar_with(&[("package/README.md", b"read me")]);
        let err = extract(&archive, &dir.path().join("out"), &options("bin/tool")).unwrap_err();
        assert!(err.to_string().contains("executable is missing"));
    }

    fn zip_with(entries: &[(&str, &[u8])]) -> (tempfile::TempDir, PathBuf) {
        use zip::write::SimpleFileOptions;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.zip");
        let file = File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);
        for (name, data) in entries {
            zip.start_file(*name, options).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
        (dir, path)
    }

    #[test]
    fn extracts_zip_and_rejects_traversal() {
        let (dir, archive) = zip_with(&[("package/bin/tool", b"#!/bin/sh\n")]);
        let dest = dir.path().join("out");
        extract(&archive, &dest, &options("bin/tool")).unwrap();
        assert!(dest.join("bin/tool").is_file());

        let (dir2, bad) = zip_with(&[("package/../escape", b"x")]);
        let err = extract(&bad, &dir2.path().join("out"), &options("bin/tool")).unwrap_err();
        assert!(
            err.to_string().contains("unsafe zip path")
                || err.to_string().contains("escapes")
                || err.to_string().contains("unexpected archive root"),
            "{err}"
        );
    }

    #[test]
    fn zip_rejects_symlink_entries() {
        use zip::write::SimpleFileOptions;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("symlink.zip");
        let file = File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let file_options = SimpleFileOptions::default().unix_permissions(0o777);
        zip.add_symlink("package/bin/alias", "tool", file_options)
            .unwrap();
        zip.finish().unwrap();
        let err = extract(&path, &dir.path().join("out"), &options("bin/tool")).unwrap_err();
        assert!(err.to_string().contains("symbolic link"));
    }

    #[test]
    fn uncompressed_size_helper_matches_content() {
        let (dir, archive) = tar_with(&[("package/bin/tool", b"12345")]);
        assert!(extract_tar_gz_size(&archive).unwrap() >= 5);
        drop(dir);
    }
}

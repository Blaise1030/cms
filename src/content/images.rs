//! Images attached to a content save.
//!
//! A save may carry uploads keyed by client IDs. The entry refers to each one with a
//! `orso-image:<clientId>` token in its body or string fields. Every upload is checked
//! from its bytes alone (signature plus a structural walk; the browser's MIME label and
//! filename are never trusted), written to a staging directory inside the project
//! (`.git/orso-staging/`, same filesystem, ignored by git and the watcher), and given a
//! deterministic unique name under `src/assets/<collection>/<slug>/`. Tokens become paths
//! relative to the entry file before the helper validates the candidate. Only after
//! validation and the final branch/hash checks are the staged files hard-linked into place
//! (never overwriting), under the caller's undo journal. Staging is removed when the save
//! ends; staging or temporary files left by a crashed process are removed by the next
//! content mutation.
use super::lifecycle::{relative, Journal, Undo};
use super::mutation::{self as m, Leaf};
use crate::error::ApiError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Largest accepted image, per file.
pub const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
/// Request body limit for create/update: the entry plus several maximum-size images.
pub const MAX_SAVE_BYTES: usize = 6 * MAX_IMAGE_BYTES;
/// Most images one save may carry.
pub const MAX_IMAGES: usize = 20;
/// URL scheme the client uses for a pending image in the entry it submits.
pub const TOKEN_PREFIX: &str = "orso-image:";
const STAGING: &str = ".git/orso-staging";
const TEMP_SUFFIX: &str = ".orso-tmp";
/// Suffix of an asset folder a delete set aside (`.<id>.<pid>-<n>.orso-trash`).
pub(super) const TRASH_SUFFIX: &str = ".orso-trash";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
    Gif,
}

impl ImageFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Gif => "gif",
        }
    }
}

/// Per-image metadata from the `entry` part.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMeta {
    pub client_id: String,
    pub alt: String,
    pub filename: String,
}

/// One uploaded image with its metadata.
pub struct Upload {
    pub client_id: String,
    pub alt: String,
    pub filename: String,
    pub bytes: Vec<u8>,
}

fn unsupported(message: &str) -> ApiError {
    ApiError::badreq(message.to_string()).with_reason("unsupported_image")
}

fn invalid(message: String) -> ApiError {
    ApiError::badreq(message).with_reason("invalid_image")
}

/// Refusal for a file over [`MAX_IMAGE_BYTES`].
pub fn too_large(filename: &str) -> ApiError {
    ApiError::badreq(format!("{filename} is larger than 10 MiB")).with_reason("image_too_large")
}

/// Client IDs are short opaque keys; `entry` names the JSON part.
pub fn valid_client_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id != "entry"
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Pair metadata with uploaded parts; each must have exactly one of the other.
pub fn collect_uploads(
    meta: Vec<ImageMeta>,
    mut parts: HashMap<String, Vec<u8>>,
) -> Result<Vec<Upload>, ApiError> {
    if meta.len() > MAX_IMAGES {
        return Err(invalid(format!(
            "A save can carry at most {MAX_IMAGES} images"
        )));
    }
    let mut out = Vec::with_capacity(meta.len());
    for m in meta {
        if !valid_client_id(&m.client_id) {
            return Err(invalid(format!("invalid image client ID: {}", m.client_id)));
        }
        let bytes = parts
            .remove(&m.client_id)
            .ok_or_else(|| invalid(format!("{} was not uploaded", m.filename)))?;
        out.push(Upload {
            client_id: m.client_id,
            alt: m.alt,
            filename: m.filename,
            bytes,
        });
    }
    if let Some(id) = parts.keys().next() {
        return Err(invalid(format!("image part {id} has no metadata")));
    }
    Ok(out)
}

// ---- Byte inspection --------------------------------------------------------------------------

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn le32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}
fn le16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Chunk walk: valid IHDR (with CRC) first, at least one IDAT, IEND exactly at the end.
fn png_ok(b: &[u8]) -> Option<()> {
    let mut at = 8;
    let mut first = true;
    let mut data = false;
    loop {
        let len = be32(b, at)? as usize;
        let kind = b.get(at + 4..at + 8)?;
        let end = (at + 12).checked_add(len)?;
        if end > b.len() || !kind.iter().all(u8::is_ascii_alphabetic) {
            return None;
        }
        if first {
            let ihdr = &b[at + 8..at + 8 + len];
            let fine = kind == b"IHDR"
                && len == 13
                && be32(ihdr, 0)? > 0
                && be32(ihdr, 4)? > 0
                && [1, 2, 4, 8, 16].contains(&ihdr[8])
                && [0, 2, 3, 4, 6].contains(&ihdr[9])
                && ihdr[10] == 0
                && ihdr[11] == 0
                && ihdr[12] <= 1
                && crc32(&b[at + 4..at + 8 + len]) == be32(b, at + 8 + len)?;
            if !fine {
                return None;
            }
            first = false;
        }
        data |= kind == b"IDAT";
        at = end;
        if kind == b"IEND" {
            return (data && len == 0 && at == b.len()).then_some(());
        }
    }
}

/// Segment walk to the first scan: a frame header with nonzero width and components, then
/// an end-of-image marker after the entropy-coded data.
fn jpeg_ok(b: &[u8]) -> Option<()> {
    let mut at = 2;
    let mut frame = false;
    loop {
        if *b.get(at)? != 0xFF {
            return None;
        }
        while *b.get(at)? == 0xFF {
            at += 1;
        }
        let marker = b[at];
        at += 1;
        match marker {
            0xD8 | 0xD9 => return None,
            0x01 | 0xD0..=0xD7 => continue,
            _ => {}
        }
        let len = be16(b, at)? as usize;
        if len < 2 || at + len > b.len() {
            return None;
        }
        if matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            frame = len >= 8 && be16(b, at + 5)? > 0 && b[at + 7] > 0;
            if !frame {
                return None;
            }
        }
        at += len;
        if marker == 0xDA {
            // Inside entropy-coded data 0xFF is stuffed, so FF D9 only appears as EOI.
            let found = b[at..].windows(2).any(|w| w == [0xFF, 0xD9]);
            return (frame && found).then_some(());
        }
    }
}

/// Block walk: logical screen, colour tables, at least one image, then the trailer.
fn gif_ok(b: &[u8]) -> Option<()> {
    fn sub_blocks(b: &[u8], mut at: usize) -> Option<usize> {
        loop {
            let size = *b.get(at)? as usize;
            at += 1 + size;
            if size == 0 {
                return Some(at);
            }
        }
    }
    if le16(b, 6)? == 0 || le16(b, 8)? == 0 {
        return None;
    }
    let flags = *b.get(10)?;
    let mut at = 13;
    if flags & 0x80 != 0 {
        at += 3 << ((flags & 7) + 1);
    }
    let mut images = 0;
    loop {
        match *b.get(at)? {
            0x2C => {
                if le16(b, at + 5)? == 0 || le16(b, at + 7)? == 0 {
                    return None;
                }
                let local = *b.get(at + 9)?;
                at += 10;
                if local & 0x80 != 0 {
                    at += 3 << ((local & 7) + 1);
                }
                if !(1..=12).contains(b.get(at)?) {
                    return None;
                }
                at = sub_blocks(b, at + 1)?;
                images += 1;
            }
            0x21 => at = sub_blocks(b, at + 2)?,
            0x3B => return (images > 0).then_some(()),
            _ => return None,
        }
    }
}

/// RIFF walk: the container size fits the bytes, every chunk fits the container, and the
/// first chunk is a VP8/VP8L bitstream header (or VP8X followed by image data).
fn webp_ok(b: &[u8]) -> Option<()> {
    let riff_end = 8usize.checked_add(le32(b, 4)? as usize)?;
    if riff_end > b.len() || riff_end < 20 {
        return None;
    }
    let mut at = 12;
    let mut first = true;
    let mut image = false;
    while at < riff_end {
        let kind = b.get(at..at + 4)?;
        let len = le32(b, at + 4)? as usize;
        let data = b.get(at + 8..(at + 8).checked_add(len)?)?;
        if at + 8 + len > riff_end {
            return None;
        }
        match kind {
            b"VP8 " => {
                let fine = len >= 10
                    && data[3..6] == [0x9D, 0x01, 0x2A]
                    && le16(data, 6)? & 0x3FFF > 0
                    && le16(data, 8)? & 0x3FFF > 0;
                if !fine {
                    return None;
                }
                image = true;
            }
            b"VP8L" => {
                if len < 5 || data[0] != 0x2F || data[4] >> 5 != 0 {
                    return None;
                }
                image = true;
            }
            b"ANMF" => image = true,
            b"VP8X" if first && len >= 10 => {}
            _ if first => return None,
            _ => {}
        }
        first = false;
        at += 8 + len + (len & 1);
    }
    image.then_some(())
}

/// Decide an upload's format from its bytes alone, checking its structure is sane.
pub fn validate_upload(bytes: &[u8]) -> Result<ImageFormat, ApiError> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(too_large("The image"));
    }
    let (format, ok) = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        (ImageFormat::Png, png_ok(bytes))
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        (ImageFormat::Jpeg, jpeg_ok(bytes))
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        (ImageFormat::Gif, gif_ok(bytes))
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        (ImageFormat::Webp, webp_ok(bytes))
    } else {
        return Err(unsupported(
            "Only PNG, JPEG, WebP, and GIF images can be uploaded",
        ));
    };
    ok.map(|()| format).ok_or_else(|| {
        unsupported(&format!(
            "The file is not a valid {} image",
            format.extension().to_uppercase()
        ))
    })
}

/// A safe file name from the client's filename stem and the detected format.
pub fn asset_name(filename: &str, format: ImageFormat) -> String {
    let base = filename.rsplit(['/', '\\']).next().unwrap_or_default();
    let stem = match base.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => base,
    };
    let mut slug = String::new();
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let mut slug: String = slug.trim_matches('-').chars().take(60).collect();
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        slug = "image".into();
    }
    format!("{slug}.{}", format.extension())
}

// ---- Tokens -----------------------------------------------------------------------------------

/// Every client ID named by a token in `text`.
fn tokens(text: &str) -> Vec<&str> {
    let mut out = vec![];
    let mut rest = text;
    while let Some(at) = rest.find(TOKEN_PREFIX) {
        rest = &rest[at + TOKEN_PREFIX.len()..];
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .unwrap_or(rest.len());
        out.push(&rest[..end]);
    }
    out
}

fn substitute(text: &str, links: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(TOKEN_PREFIX) {
        out.push_str(&rest[..at]);
        rest = &rest[at + TOKEN_PREFIX.len()..];
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            .unwrap_or(rest.len());
        out.push_str(&links[&rest[..end]]);
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

fn strings_mut(value: &mut Value, visit: &mut dyn FnMut(&mut String)) {
    match value {
        Value::String(s) => visit(s),
        Value::Array(items) => items.iter_mut().for_each(|v| strings_mut(v, visit)),
        Value::Object(map) => map.values_mut().for_each(|v| strings_mut(v, visit)),
        _ => {}
    }
}

// ---- Staging and placement --------------------------------------------------------------------

/// A per-save directory under `.git/orso-staging/`, removed when dropped.
struct Staging(PathBuf);

impl Staging {
    fn create(root: &Path) -> Result<Self, ApiError> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let base = m::resolve_as(root, STAGING, Leaf::Dir)?;
        match std::fs::create_dir(&base) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        let dir = base.join(format!(
            "{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir)?;
        Ok(Self(dir))
    }

    fn write(&self, index: usize, bytes: &[u8]) -> Result<PathBuf, ApiError> {
        let path = self.0.join(index.to_string());
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(path)
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Staged {
    path: PathBuf,
    name: String,
    client_id: String,
    link: String,
}

/// Where a saved upload went: its project-relative path and its link from the entry.
#[derive(Debug, Clone, Serialize)]
pub struct SavedImage {
    pub path: String,
    pub link: String,
}

/// Validated, staged images for one save, with their final names chosen.
pub(super) struct Prepared {
    _staging: Staging,
    asset_dir: String,
    staged: Vec<Staged>,
}

/// Validate `uploads`, replace their tokens in `fields` and `body` with paths relative to
/// the entry at `entry_rel`, and stage the bytes. No upload means nothing to do.
pub(super) fn prepare(
    root: &Path,
    collection: &str,
    id: &str,
    entry_rel: &str,
    uploads: &[Upload],
    fields: &mut Map<String, Value>,
    body: &mut Option<String>,
) -> Result<Option<Prepared>, ApiError> {
    if uploads.is_empty() {
        return Ok(None);
    }
    let mut formats = Vec::with_capacity(uploads.len());
    let mut ids = HashSet::new();
    for u in uploads {
        if !ids.insert(u.client_id.as_str()) {
            return Err(invalid(format!(
                "duplicate image client ID {}",
                u.client_id
            )));
        }
        if u.alt.trim().is_empty() {
            return Err(
                ApiError::badreq(format!("{}: describe the image (alt text)", u.filename))
                    .with_reason("missing_alt"),
            );
        }
        let format = validate_upload(&u.bytes).map_err(|mut e| {
            e.message = format!("{}: {}", u.filename, e.message);
            e
        })?;
        formats.push(format);
    }

    // Every token names an upload and every upload is used, so no orphan asset is written.
    let mut used = HashSet::new();
    let mut unknown = None;
    let mut note = |text: &str| {
        for t in tokens(text) {
            if ids.contains(t) {
                used.insert(t.to_string());
            } else {
                unknown.get_or_insert_with(|| t.to_string());
            }
        }
    };
    if let Some(body) = body.as_deref() {
        note(body);
    }
    for value in fields.values_mut() {
        strings_mut(value, &mut |s| note(s));
    }
    if let Some(t) = unknown {
        return Err(invalid(format!(
            "the entry names image {t}, which was not uploaded"
        )));
    }
    if let Some(u) = uploads.iter().find(|u| !used.contains(&u.client_id)) {
        return Err(invalid(format!("{} is not used in the entry", u.filename)));
    }

    // Deterministic names: the first free `name.ext`, `name-2.ext`, ... in upload order.
    // Existing names are compared case-insensitively for case-insensitive filesystems.
    let asset_dir = format!("src/assets/{collection}/{id}");
    let dir = m::resolve_as(root, &asset_dir, Leaf::Dir)?;
    let mut taken: HashSet<String> = HashSet::new();
    if dir.exists() {
        for e in std::fs::read_dir(&dir)? {
            taken.insert(e?.file_name().to_string_lossy().to_lowercase());
        }
    }
    let entry_dir = Path::new(entry_rel)
        .parent()
        .and_then(Path::to_str)
        .unwrap_or("");
    let mut names = Vec::with_capacity(uploads.len());
    let mut links = HashMap::new();
    for (u, format) in uploads.iter().zip(&formats) {
        let base = asset_name(&u.filename, *format);
        let (stem, ext) = base.rsplit_once('.').unwrap();
        let name = (1..)
            .map(|n| {
                if n == 1 {
                    base.clone()
                } else {
                    format!("{stem}-{n}.{ext}")
                }
            })
            .find(|n| !taken.contains(n))
            .unwrap();
        taken.insert(name.clone());
        links.insert(
            u.client_id.as_str(),
            relative(entry_dir, &format!("{asset_dir}/{name}")),
        );
        names.push(name);
    }
    if let Some(text) = body.as_mut() {
        *text = substitute(text, &links);
    }
    for value in fields.values_mut() {
        strings_mut(value, &mut |s| *s = substitute(s, &links));
    }

    let staging = Staging::create(root)?;
    let mut staged = Vec::with_capacity(uploads.len());
    for (i, (u, name)) in uploads.iter().zip(names).enumerate() {
        staged.push(Staged {
            path: staging.write(i, &u.bytes)?,
            name,
            client_id: u.client_id.clone(),
            link: links[u.client_id.as_str()].clone(),
        });
    }
    Ok(Some(Prepared {
        _staging: staging,
        asset_dir,
        staged,
    }))
}

impl Prepared {
    /// Each upload's final path and link, by client ID.
    pub(super) fn saved(&self) -> BTreeMap<String, SavedImage> {
        self.staged
            .iter()
            .map(|s| {
                let saved = SavedImage {
                    path: format!("{}/{}", self.asset_dir, s.name),
                    link: s.link.clone(),
                };
                (s.client_id.clone(), saved)
            })
            .collect()
    }

    /// Link the staged files into the asset directory, never replacing an existing file.
    /// Every created directory and file is recorded in `journal`. Returns the new asset
    /// paths, project-relative.
    pub(super) fn place(
        &self,
        root: &Path,
        journal: &mut Journal,
    ) -> Result<Vec<String>, ApiError> {
        let mut created = vec![];
        let dir = m::create_dirs(root, &self.asset_dir, &mut created);
        journal.0.extend(created.into_iter().map(Undo::RemoveDir));
        let dir = dir?;
        let mut placed = vec![];
        for s in &self.staged {
            let target = dir.join(&s.name);
            match std::fs::hard_link(&s.path, &target) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    return Err(ApiError::conflict(format!(
                        "{}/{} appeared during the save; save again",
                        self.asset_dir, s.name
                    ))
                    .with_reason("stale_content"))
                }
                Err(e) => return Err(e.into()),
            }
            journal.0.push(Undo::Remove(target));
            placed.push(format!("{}/{}", self.asset_dir, s.name));
        }
        Ok(placed)
    }
}

// ---- Crash leftovers --------------------------------------------------------------------------

/// Remove staging directories, temporary files, and set-aside trash folders that a crashed
/// or partly failed operation left behind. Runs
/// under the project's content lock, so nothing found is in use. Best effort: a failure is
/// logged and the operation continues.
pub(super) fn cleanup_orphans(root: &Path) {
    fn sweep(dir: &Path, depth: usize) -> std::io::Result<()> {
        for e in std::fs::read_dir(dir)? {
            let e = e?;
            let meta = std::fs::symlink_metadata(e.path())?;
            let name = e.file_name();
            let name = name.to_string_lossy();
            if meta.is_dir() && name.starts_with('.') && name.ends_with(TRASH_SUFFIX) {
                // symlink_metadata never reports a symlink as a dir: only a real folder goes.
                std::fs::remove_dir_all(e.path())?;
            } else if meta.is_dir() && !meta.file_type().is_symlink() && depth < 16 {
                sweep(&e.path(), depth + 1)?;
            } else if meta.is_file() && name.starts_with('.') && name.ends_with(TEMP_SUFFIX) {
                std::fs::remove_file(e.path())?;
            }
        }
        Ok(())
    }
    let run = || -> Result<(), ApiError> {
        let staging = m::resolve_as(root, STAGING, Leaf::Dir)?;
        if staging.exists() {
            for e in std::fs::read_dir(&staging)? {
                let path = e?.path();
                if std::fs::symlink_metadata(&path)?.is_dir() {
                    std::fs::remove_dir_all(&path)?;
                } else {
                    std::fs::remove_file(&path)?;
                }
            }
        }
        for rel in ["src/content", "src/assets"] {
            // A symlinked or missing tree is never swept.
            if let Ok(dir) = m::resolve_as(root, rel, Leaf::Dir) {
                if dir.exists() {
                    sweep(&dir, 0)?;
                }
            }
        }
        Ok(())
    };
    if let Err(e) = run() {
        eprintln!(
            "orso: could not clean content leftovers in {}: {}",
            root.display(),
            e.message
        );
    }
}

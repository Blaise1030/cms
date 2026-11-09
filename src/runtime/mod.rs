//! Managed agent runtimes.
//!
//! Downloads pinned Node, OpenCode, Claude, Codex and Cursor builds into a
//! per-user store, verifies them, and resolves the managed copy before falling
//! back to a CLI the user installed themselves. The types here are independent
//! of [`crate::state::AppState`] so they can be unit-tested on their own; the
//! HTTP layer and `AppState` wiring are added separately.

use std::fmt;
use std::path::PathBuf;

pub mod adapter;
pub mod archive;
pub mod descriptors;
pub mod login;
pub mod manager;
pub mod resolve;

pub use adapter::{AcpAdapterSpec, AdapterInstaller, AdapterManifest, AdapterState};
pub use archive::{extract, extract_format, ArchiveFormat, ExtractOptions, Limits, SymlinkPolicy};
pub use descriptors::{
    version_cmp, version_meets_minimum, InstallFile, InstallManifest, LockFile, RuntimeArtifact,
    RuntimeId, RuntimeSpec, RuntimeTarget,
};
pub use manager::{RuntimeEvent, RuntimeManager, RuntimePhase, RuntimeSnapshot, RuntimeSource};
pub use resolve::{
    build_path, AcpResolution, AcpSource, AgentAcpOverrides, PathParts, ResolveSource,
    ResolvedProvider, RuntimeResolver,
};

/// Everything the runtime store can fail with.
#[derive(Debug)]
pub enum RuntimeError {
    /// A download/install is already running for this runtime.
    Busy(RuntimeId),
    /// The user cancelled the download.
    Canceled,
    /// Nothing is installed and there is no candidate to resolve.
    NotFound(String),
    /// There is no artifact for this target in the lock file.
    UnsupportedTarget(RuntimeTarget),
    /// The lock file is malformed or missing a required field.
    InvalidLock(String),
    /// The `orso-install.json` beside an install is malformed.
    InvalidManifest(String),
    /// A filesystem operation failed.
    Io(std::io::Error),
    /// The HTTP request failed.
    Http(String),
    /// The downloaded bytes do not match the pinned SHA-256.
    HashMismatch { expected: String, actual: String },
    /// The download ended before or after the pinned byte count.
    SizeMismatch { expected: u64, actual: u64 },
    /// Not enough disk space for the download plus the installed tree.
    DiskFull { required: u64, available: u64 },
    /// Safe extraction rejected the archive.
    Archive(String),
    /// `--version` printed something other than the pinned version.
    VersionMismatch { expected: String, actual: String },
    /// An ACP adapter could not be installed. The message is already suitable
    /// to show to a non-technical user.
    Adapter(String),
    /// The documented executable is missing or not executable.
    NotExecutable(PathBuf),
    /// An internal lock was poisoned.
    Lock(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy(id) => write!(f, "{} is already being set up", id.as_str()),
            Self::Canceled => write!(f, "download cancelled"),
            Self::NotFound(what) => write!(f, "{what}"),
            Self::UnsupportedTarget(target) => {
                write!(f, "no build is available for {}", target.as_str())
            }
            Self::InvalidLock(message) => write!(f, "invalid runtime lock file: {message}"),
            Self::InvalidManifest(message) => write!(f, "invalid install manifest: {message}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::Http(message) => write!(f, "download failed: {message}"),
            Self::HashMismatch { expected, actual } => write!(
                f,
                "downloaded file does not match its checksum (expected {expected}, got {actual})"
            ),
            Self::SizeMismatch { expected, actual } => {
                write!(f, "downloaded {actual} bytes, expected {expected}")
            }
            Self::DiskFull {
                required,
                available,
            } => write!(
                f,
                "not enough disk space: {required} bytes needed, {available} bytes available"
            ),
            Self::Archive(message) => write!(f, "cannot unpack the download: {message}"),
            Self::VersionMismatch { expected, actual } => {
                write!(f, "installed version is {actual}, expected {expected}")
            }
            Self::Adapter(message) => write!(f, "{message}"),
            Self::NotExecutable(path) => {
                write!(f, "{} is missing or not executable", path.display())
            }
            Self::Lock(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl From<std::io::Error> for RuntimeError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<reqwest::Error> for RuntimeError {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error.to_string())
    }
}

impl From<serde_json::Error> for RuntimeError {
    fn from(error: serde_json::Error) -> Self {
        Self::InvalidLock(error.to_string())
    }
}

/// Result alias for runtime operations.
pub type Result<T> = std::result::Result<T, RuntimeError>;

/// Lowercase hex, used for SHA-256 values in manifests and errors.
pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    out
}

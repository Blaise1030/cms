//! Download, verify, stage and atomically install managed runtimes.
//!
//! The manager owns no global state: the caller supplies the store root, the
//! lock file and the target. A [`RuntimeManager`] is cheap to clone and safe to
//! share; one download per runtime is allowed at a time.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex as AsyncMutex};

use super::adapter::{AcpAdapterSpec, AdapterInstaller, AdapterManifest, AdapterState};
use super::descriptors::{
    hash_file, read_manifest, version_cmp, InstallManifest, LockFile, RuntimeId, RuntimeSpec,
    RuntimeTarget,
};
use super::{Result, RuntimeError};

/// Event name the HTTP layer forwards on `WS /ws/events`.
pub const RUNTIME_EVENT: &str = "setup.runtime";

/// How long a failed attempt keeps its status before a retry is allowed.
const VERSION_TIMEOUT: Duration = Duration::from_secs(30);
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
const DEFAULT_HEADROOM: u64 = 256 * 1024 * 1024;
const DEFAULT_CLEANUP_AFTER: Duration = Duration::from_secs(7 * 24 * 60 * 60);

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Where a runtime came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeSource {
    Managed,
    System,
}

/// The coarse phase shown by the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimePhaseKind {
    NotDownloaded,
    Downloading,
    Finishing,
    Ready,
    DownloadError,
    UpdateAvailable,
}

impl RuntimePhaseKind {
    /// Stable string for logs and assertions.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotDownloaded => "not-downloaded",
            Self::Downloading => "downloading",
            Self::Finishing => "finishing",
            Self::Ready => "ready",
            Self::DownloadError => "download-error",
            Self::UpdateAvailable => "update-available",
        }
    }
}

/// A runtime's full public state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePhase {
    #[serde(rename = "phase")]
    pub kind: RuntimePhaseKind,
    pub version: Option<String>,
    pub available: Option<String>,
    pub progress: Option<u8>,
    pub message: Option<String>,
    pub source: Option<RuntimeSource>,
}

impl RuntimePhase {
    pub fn not_downloaded() -> Self {
        Self {
            kind: RuntimePhaseKind::NotDownloaded,
            version: None,
            available: None,
            progress: None,
            message: None,
            source: None,
        }
    }

    pub fn downloading(progress: Option<u8>, message: Option<String>) -> Self {
        Self {
            kind: RuntimePhaseKind::Downloading,
            version: None,
            available: None,
            progress,
            message,
            source: None,
        }
    }

    pub fn finishing() -> Self {
        Self {
            kind: RuntimePhaseKind::Finishing,
            version: None,
            available: None,
            progress: Some(100),
            message: None,
            source: None,
        }
    }

    pub fn ready(version: impl Into<String>, source: RuntimeSource) -> Self {
        Self {
            kind: RuntimePhaseKind::Ready,
            version: Some(version.into()),
            available: None,
            progress: None,
            message: None,
            source: Some(source),
        }
    }

    pub fn download_error(version: Option<String>, message: impl Into<String>) -> Self {
        Self {
            kind: RuntimePhaseKind::DownloadError,
            version,
            available: None,
            progress: None,
            message: Some(message.into()),
            source: None,
        }
    }

    pub fn update_available(version: impl Into<String>, available: impl Into<String>) -> Self {
        Self {
            kind: RuntimePhaseKind::UpdateAvailable,
            version: Some(version.into()),
            available: Some(available.into()),
            progress: None,
            message: None,
            source: Some(RuntimeSource::Managed),
        }
    }

    /// Installed version, when there is one.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Whether the runtime is ready to use.
    pub fn is_ready(&self) -> bool {
        self.kind == RuntimePhaseKind::Ready
    }
}

/// Serializable snapshot returned by [`RuntimeManager::snapshot`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSnapshot {
    pub runtime: RuntimeId,
    #[serde(flatten)]
    pub phase: RuntimePhase,
    pub download_bytes: Option<u64>,
    pub installed_bytes: Option<u64>,
}

/// One `setup.runtime` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEvent {
    pub runtime: RuntimeId,
    #[serde(flatten)]
    pub phase: RuntimePhase,
}

struct Slot {
    phase: RuntimePhase,
    active: bool,
    cancel: Option<Arc<AtomicBool>>,
    generation: u64,
}

/// Per-store runtime manager.
#[derive(Clone)]
pub struct RuntimeManager {
    store: PathBuf,
    lock: Arc<LockFile>,
    target: RuntimeTarget,
    base_url: Option<String>,
    client: reqwest::Client,
    slots: Arc<Mutex<HashMap<RuntimeId, Slot>>>,
    events: broadcast::Sender<RuntimeEvent>,
    headroom: u64,
    cleanup_after: Duration,
    install_lock: Arc<AsyncMutex<()>>,
    /// Pinned Claude/Codex ACP adapter installs.
    adapters: AdapterInstaller,
    /// Test override for the free-space check.
    disk_available: Option<u64>,
}

impl RuntimeManager {
    /// Build a manager and detect what is already installed under `store`.
    pub fn new(store: impl Into<PathBuf>, lock: LockFile, target: RuntimeTarget) -> Self {
        let store = store.into();
        let mut slots = HashMap::new();
        for id in lock.ids() {
            let phase = match latest_installed(&store, id) {
                Some(version) => RuntimePhase::ready(version, RuntimeSource::Managed),
                None => RuntimePhase::not_downloaded(),
            };
            slots.insert(
                id,
                Slot {
                    phase,
                    active: false,
                    cancel: None,
                    generation: 0,
                },
            );
        }
        let lock = Arc::new(lock);
        Self {
            store: store.clone(),
            lock: lock.clone(),
            target,
            base_url: None,
            client: reqwest::Client::builder()
                .user_agent(concat!("orso/", env!("CARGO_PKG_VERSION")))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            slots: Arc::new(Mutex::new(slots)),
            events: broadcast::channel(256).0,
            headroom: DEFAULT_HEADROOM,
            cleanup_after: DEFAULT_CLEANUP_AFTER,
            install_lock: Arc::new(AsyncMutex::new(())),
            adapters: AdapterInstaller::new(store, lock),
            disk_available: None,
        }
    }

    /// Build a manager from a lock file on disk.
    pub fn from_lock_path(
        store: impl Into<PathBuf>,
        path: &Path,
        target: RuntimeTarget,
    ) -> Result<Self> {
        Ok(Self::new(store, LockFile::load(path)?, target))
    }

    /// Point downloads at a stub server (tests, or a mirror).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into().trim_end_matches('/').to_string());
        self
    }

    /// Extra free space to require beyond the download and installed tree.
    pub fn with_headroom(mut self, bytes: u64) -> Self {
        self.headroom = bytes;
        self
    }

    /// Override the free-space reading. Tests use this to exercise the
    /// "not enough space" path without filling a volume.
    pub fn with_disk_available(mut self, bytes: u64) -> Self {
        self.disk_available = Some(bytes);
        self
    }

    /// Change how long stale staging/replaced directories are kept.
    pub fn with_cleanup_after(mut self, age: Duration) -> Self {
        self.cleanup_after = age;
        self
    }

    /// Store root.
    pub fn store(&self) -> &Path {
        &self.store
    }

    /// Pinned lock file.
    pub fn lock(&self) -> &LockFile {
        &self.lock
    }

    /// Target this manager installs for.
    pub fn target(&self) -> RuntimeTarget {
        self.target
    }

    /// Subscribe to `setup.runtime` events.
    pub fn subscribe(&self) -> broadcast::Receiver<RuntimeEvent> {
        self.events.subscribe()
    }

    /// Snapshot every runtime in the lock.
    pub fn snapshots(&self) -> Vec<RuntimeSnapshot> {
        self.lock
            .ids()
            .into_iter()
            .filter_map(|id| self.snapshot(id).ok())
            .collect()
    }

    /// Snapshot one runtime.
    pub fn snapshot(&self, id: RuntimeId) -> Result<RuntimeSnapshot> {
        let phase = self.phase(id)?;
        let artifact = self
            .lock
            .artifact(id, self.target)
            .ok()
            .map(|artifact| (artifact.download_bytes, artifact.installed_bytes));
        Ok(RuntimeSnapshot {
            runtime: id,
            phase,
            download_bytes: artifact.and_then(|(download, _)| download),
            installed_bytes: artifact.and_then(|(_, installed)| installed),
        })
    }

    /// Current phase for one runtime.
    pub fn phase(&self, id: RuntimeId) -> Result<RuntimePhase> {
        self.slots
            .lock()
            .map_err(|error| RuntimeError::Lock(error.to_string()))?
            .get(&id)
            .map(|slot| slot.phase.clone())
            .ok_or_else(|| RuntimeError::NotFound(format!("{} is not managed here", id.as_str())))
    }

    /// The install recipe for one runtime on this manager's target.
    pub fn spec(&self, id: RuntimeId) -> Result<RuntimeSpec> {
        RuntimeSpec::for_target(id, self.target, &self.lock)
    }

    /// Pinned ACP adapter recipe for Claude or Codex.
    pub fn adapter_spec(&self, id: RuntimeId) -> Result<AcpAdapterSpec> {
        self.adapters.spec(id)
    }

    /// Verified Claude/Codex adapter install, if any.
    pub fn adapter_install(&self, id: RuntimeId) -> Option<AdapterManifest> {
        self.adapters.installed(id)
    }

    /// User-facing adapter state.
    pub fn adapter_state(&self, id: RuntimeId) -> AdapterState {
        self.adapters.state(id)
    }

    /// Whether installing this adapter still needs the managed site builder.
    pub fn adapter_needs_node(&self, id: RuntimeId) -> bool {
        self.adapters.node_missing(id)
    }

    /// Install the pinned adapter if needed. Idempotent; concurrent callers are
    /// serialised by the adapter installer.
    pub async fn ensure_adapter(&self, id: RuntimeId) -> Result<AdapterManifest> {
        self.adapters.ensure(id).await
    }

    /// Stop an in-flight adapter install.
    pub fn cancel_adapter(&self, id: RuntimeId) {
        self.adapters.cancel(id);
    }

    /// Download and install `id`. Only one attempt per runtime is allowed.
    pub async fn download(&self, id: RuntimeId) -> Result<RuntimeSnapshot> {
        let spec = self.spec(id)?;
        let cancel = Arc::new(AtomicBool::new(false));
        let generation;
        let previous_version;
        {
            let mut slots = self
                .slots
                .lock()
                .map_err(|error| RuntimeError::Lock(error.to_string()))?;
            let slot = slots.entry(id).or_insert(Slot {
                phase: RuntimePhase::not_downloaded(),
                active: false,
                cancel: None,
                generation: 0,
            });
            if slot.active {
                return Err(RuntimeError::Busy(id));
            }
            slot.active = true;
            slot.generation = slot.generation.wrapping_add(1);
            generation = slot.generation;
            slot.cancel = Some(cancel.clone());
            previous_version = slot.phase.version().map(str::to_string);
            slot.phase = RuntimePhase::downloading(Some(0), None);
        }
        self.emit(id);

        let result = self.install_spec(&spec, cancel, generation).await;
        let (phase, reply) = match &result {
            Ok(manifest) => (
                RuntimePhase::ready(manifest.version.clone(), RuntimeSource::Managed),
                Ok(()),
            ),
            Err(RuntimeError::Canceled) => (
                match previous_version.clone() {
                    Some(version) => RuntimePhase::ready(version, RuntimeSource::Managed),
                    None => RuntimePhase::not_downloaded(),
                },
                Err(RuntimeError::Canceled),
            ),
            Err(error) => (
                RuntimePhase::download_error(previous_version.clone(), error.to_string()),
                Err(error_clone(error)),
            ),
        };
        {
            let mut slots = self
                .slots
                .lock()
                .map_err(|error| RuntimeError::Lock(error.to_string()))?;
            if let Some(slot) = slots.get_mut(&id) {
                if slot.generation == generation {
                    slot.active = false;
                    slot.cancel = None;
                    slot.phase = phase;
                }
            }
        }
        self.emit(id);
        match reply {
            Ok(()) => self.snapshot(id),
            Err(error) => Err(error),
        }
    }

    /// Abort an in-flight download. The install task removes its partial and
    /// staging directories and restores the previous ready phase.
    pub fn cancel(&self, id: RuntimeId) -> Result<RuntimePhase> {
        let mut slots = self
            .slots
            .lock()
            .map_err(|error| RuntimeError::Lock(error.to_string()))?;
        let slot = slots.get_mut(&id).ok_or_else(|| {
            RuntimeError::NotFound(format!("{} is not managed here", id.as_str()))
        })?;
        if let Some(cancel) = &slot.cancel {
            cancel.store(true, Ordering::SeqCst);
        }
        Ok(slot.phase.clone())
    }

    /// Remove a failed partial download and try again.
    pub async fn retry(&self, id: RuntimeId) -> Result<RuntimeSnapshot> {
        if self.is_active(id)? {
            return Err(RuntimeError::Busy(id));
        }
        let spec = self.spec(id)?;
        let partial = self.partial_path(&spec);
        if partial.exists() {
            std::fs::remove_file(&partial)?;
        }
        self.download(id).await
    }

    /// Remove stale staging/replaced directories and old versions. Keeps the
    /// current install and one previous version for rollback.
    pub fn cleanup(&self) -> Result<()> {
        self.cleanup_expired(self.cleanup_after)
    }

    /// Cleanup with an explicit age threshold.
    pub fn cleanup_expired(&self, age: Duration) -> Result<()> {
        std::fs::create_dir_all(&self.store)?;
        let now = SystemTime::now();
        for entry in std::fs::read_dir(&self.store)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !(name.starts_with(".staging-") || name.starts_with(".replaced-")) {
                continue;
            }
            if is_older_than(&entry.path(), now, age)? {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
        let downloads = self.store.join(".downloads");
        if downloads.is_dir() {
            for entry in std::fs::read_dir(downloads)? {
                let entry = entry?;
                if entry.path().extension().and_then(|ext| ext.to_str()) == Some("partial")
                    && is_older_than(&entry.path(), now, age)?
                {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        for id in self.lock.ids() {
            prune_versions(&self.store.join(id.as_str()), age, now)?;
        }
        Ok(())
    }

    fn is_active(&self, id: RuntimeId) -> Result<bool> {
        Ok(self
            .slots
            .lock()
            .map_err(|error| RuntimeError::Lock(error.to_string()))?
            .get(&id)
            .map(|slot| slot.active)
            .unwrap_or(false))
    }

    fn emit(&self, id: RuntimeId) {
        let phase = self
            .slots
            .lock()
            .ok()
            .and_then(|slots| slots.get(&id).map(|slot| slot.phase.clone()));
        if let Some(phase) = phase {
            let _ = self.events.send(RuntimeEvent { runtime: id, phase });
        }
    }

    fn update_phase(&self, id: RuntimeId, generation: u64, phase: RuntimePhase) {
        let event = {
            let Ok(mut slots) = self.slots.lock() else {
                return;
            };
            let Some(slot) = slots.get_mut(&id) else {
                return;
            };
            if slot.generation != generation {
                return;
            }
            slot.phase = phase.clone();
            RuntimeEvent { runtime: id, phase }
        };
        let _ = self.events.send(event);
    }

    async fn install_spec(
        &self,
        spec: &RuntimeSpec,
        cancel: Arc<AtomicBool>,
        generation: u64,
    ) -> Result<InstallManifest> {
        self.ensure_dirs()?;
        self.check_disk(spec)?;
        let archive = self.download_archive(spec, &cancel, generation).await?;
        let staging = self.staging_path(spec.id);
        let result = self
            .install_archive(spec, &archive, &staging, &cancel, generation)
            .await;
        if result.is_err() {
            let _ = std::fs::remove_dir_all(&staging);
            if matches!(result, Err(RuntimeError::Canceled)) {
                let _ = std::fs::remove_file(&archive);
            }
        }
        result
    }

    async fn install_archive(
        &self,
        spec: &RuntimeSpec,
        archive: &Path,
        staging: &Path,
        cancel: &Arc<AtomicBool>,
        generation: u64,
    ) -> Result<InstallManifest> {
        check_cancel(cancel)?;
        self.update_phase(spec.id, generation, RuntimePhase::finishing());
        let spec_for_stage = spec.clone();
        let archive_for_stage = archive.to_path_buf();
        let staging_for_stage = staging.to_path_buf();
        let manifest = tokio::task::spawn_blocking(move || {
            spec_for_stage.stage(&archive_for_stage, &staging_for_stage)
        })
        .await
        .map_err(|error| RuntimeError::Archive(format!("install task failed: {error}")))??;
        check_cancel(cancel)?;

        let version = run_version(spec, staging).await?;
        if version_cmp(&version, &spec.version) != std::cmp::Ordering::Equal {
            return Err(RuntimeError::VersionMismatch {
                expected: spec.version.clone(),
                actual: version,
            });
        }
        check_cancel(cancel)?;
        let _guard = self.install_lock.lock().await;
        let final_dir = self.install_dir(spec);
        replace_dir(staging, &final_dir, &self.store, spec.id)?;
        drop(_guard);
        let _ = std::fs::remove_file(archive);
        Ok(manifest)
    }

    fn check_disk(&self, spec: &RuntimeSpec) -> Result<()> {
        let required = spec
            .download_bytes
            .unwrap_or(0)
            .saturating_add(spec.installed_bytes.unwrap_or(0))
            .saturating_add(self.headroom);
        let available = self
            .disk_available
            .or_else(|| available_bytes(&self.store))
            .unwrap_or(u64::MAX);
        if available < required {
            return Err(RuntimeError::DiskFull {
                required,
                available,
            });
        }
        Ok(())
    }

    async fn download_archive(
        &self,
        spec: &RuntimeSpec,
        cancel: &Arc<AtomicBool>,
        generation: u64,
    ) -> Result<PathBuf> {
        let partial = self.partial_path(spec);
        if let Some(parent) = partial.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let existing = std::fs::metadata(&partial)
            .map(|meta| meta.len())
            .unwrap_or(0);
        let url = self.download_url(spec);
        let mut request = self.client.get(&url);
        if existing > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={existing}-"));
        }
        let response = request.send().await?;
        if !response.status().is_success() {
            return Err(RuntimeError::Http(format!(
                "HTTP {} from {url}",
                response.status()
            )));
        }
        let resumed = existing > 0 && response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
        if existing > 0 && !resumed {
            let _ = std::fs::remove_file(&partial);
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(resumed)
            .truncate(!resumed)
            .open(&partial)?;
        let mut downloaded = if resumed { existing } else { 0 };
        let expected = spec.download_bytes;
        if let Some(total) = expected {
            if downloaded > total {
                let _ = std::fs::remove_file(&partial);
                return Err(RuntimeError::SizeMismatch {
                    expected: total,
                    actual: downloaded,
                });
            }
        }
        let mut last_emit = Instant::now() - PROGRESS_INTERVAL;
        let mut stream = response.bytes_stream();
        let mut cancel_tick = tokio::time::interval(PROGRESS_INTERVAL);
        cancel_tick.tick().await;
        loop {
            let chunk = tokio::select! {
                _ = cancel_tick.tick() => {
                    check_cancel_bool(cancel, &partial)?;
                    continue;
                }
                chunk = stream.next() => chunk,
            };
            let Some(chunk) = chunk else {
                break;
            };
            let chunk = chunk?;
            downloaded = downloaded.saturating_add(chunk.len() as u64);
            if let Some(total) = expected {
                if downloaded > total {
                    let _ = std::fs::remove_file(&partial);
                    return Err(RuntimeError::SizeMismatch {
                        expected: total,
                        actual: downloaded,
                    });
                }
            }
            file.write_all(&chunk)?;
            let progress = expected
                .map(|total| ((downloaded.saturating_mul(100) / total.max(1)).min(100)) as u8);
            if last_emit.elapsed() >= PROGRESS_INTERVAL {
                self.update_phase(
                    spec.id,
                    generation,
                    RuntimePhase::downloading(progress, None),
                );
                last_emit = Instant::now();
            }
        }
        file.flush()?;
        file.sync_all()?;
        drop(file);
        check_cancel_bool(cancel, &partial)?;
        if let Some(total) = expected {
            if downloaded != total {
                let _ = std::fs::remove_file(&partial);
                return Err(RuntimeError::SizeMismatch {
                    expected: total,
                    actual: downloaded,
                });
            }
        }
        self.update_phase(
            spec.id,
            generation,
            RuntimePhase::downloading(Some(100), None),
        );
        let path = partial.clone();
        let expected_hash = spec.sha256.clone();
        let actual = tokio::task::spawn_blocking(move || hash_file(&path))
            .await
            .map_err(|error| RuntimeError::Io(std::io::Error::other(error.to_string())))??;
        if actual != expected_hash {
            let _ = std::fs::remove_file(&partial);
            return Err(RuntimeError::HashMismatch {
                expected: expected_hash,
                actual,
            });
        }
        Ok(partial)
    }

    fn download_url(&self, spec: &RuntimeSpec) -> String {
        if let Some(base) = &self.base_url {
            let name = spec
                .source_url
                .rsplit('/')
                .next()
                .filter(|name| !name.is_empty())
                .unwrap_or("artifact.tar.gz");
            return format!("{base}/{name}");
        }
        spec.source_url.clone()
    }

    fn install_dir(&self, spec: &RuntimeSpec) -> PathBuf {
        self.store.join(spec.id.as_str()).join(&spec.version)
    }

    fn partial_path(&self, spec: &RuntimeSpec) -> PathBuf {
        self.store
            .join(".downloads")
            .join(format!("{}-{}.partial", spec.id.as_str(), spec.version))
    }

    fn staging_path(&self, id: RuntimeId) -> PathBuf {
        self.store.join(format!(
            ".staging-{}-{}-{}",
            id.as_str(),
            std::process::id(),
            next_nonce()
        ))
    }

    fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(self.store.join(".downloads"))?;
        Ok(())
    }
}

fn error_clone(error: &RuntimeError) -> RuntimeError {
    match error {
        RuntimeError::Busy(id) => RuntimeError::Busy(*id),
        RuntimeError::Canceled => RuntimeError::Canceled,
        RuntimeError::NotFound(message) => RuntimeError::NotFound(message.clone()),
        RuntimeError::UnsupportedTarget(target) => RuntimeError::UnsupportedTarget(*target),
        RuntimeError::InvalidLock(message) => RuntimeError::InvalidLock(message.clone()),
        RuntimeError::InvalidManifest(message) => RuntimeError::InvalidManifest(message.clone()),
        RuntimeError::Io(error) => {
            RuntimeError::Io(std::io::Error::new(error.kind(), error.to_string()))
        }
        RuntimeError::Http(message) => RuntimeError::Http(message.clone()),
        RuntimeError::HashMismatch { expected, actual } => RuntimeError::HashMismatch {
            expected: expected.clone(),
            actual: actual.clone(),
        },
        RuntimeError::SizeMismatch { expected, actual } => RuntimeError::SizeMismatch {
            expected: *expected,
            actual: *actual,
        },
        RuntimeError::DiskFull {
            required,
            available,
        } => RuntimeError::DiskFull {
            required: *required,
            available: *available,
        },
        RuntimeError::Archive(message) => RuntimeError::Archive(message.clone()),
        RuntimeError::VersionMismatch { expected, actual } => RuntimeError::VersionMismatch {
            expected: expected.clone(),
            actual: actual.clone(),
        },
        RuntimeError::Adapter(message) => RuntimeError::Adapter(message.clone()),
        RuntimeError::NotExecutable(path) => RuntimeError::NotExecutable(path.clone()),
        RuntimeError::Lock(message) => RuntimeError::Lock(message.clone()),
    }
}

fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::SeqCst) {
        Err(RuntimeError::Canceled)
    } else {
        Ok(())
    }
}

fn check_cancel_bool(cancel: &AtomicBool, partial: &Path) -> Result<()> {
    if cancel.load(Ordering::SeqCst) {
        let _ = std::fs::remove_file(partial);
        Err(RuntimeError::Canceled)
    } else {
        Ok(())
    }
}

fn next_nonce() -> u64 {
    let counter = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ counter.rotate_left(17)
}

async fn run_version(spec: &RuntimeSpec, root: &Path) -> Result<String> {
    let executable = spec.executable_path(root);
    let mut command = tokio::process::Command::new(&executable);
    command
        .arg("--version")
        .env_clear()
        .env("HOME", std::env::var_os("HOME").unwrap_or_default())
        .env("PATH", version_path(spec, root));
    let output = tokio::time::timeout(VERSION_TIMEOUT, command.output())
        .await
        .map_err(|_| RuntimeError::Http(format!("{} --version timed out", spec.id.as_str())))??;
    if !output.status.success() {
        return Err(RuntimeError::Http(format!(
            "{} --version exited with {}",
            spec.id.as_str(),
            output.status
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    match spec.parse_version(&stdout) {
        Ok(version) => Ok(version),
        Err(first) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            spec.parse_version(&stderr).map_err(|_| first)
        }
    }
}

fn version_path(spec: &RuntimeSpec, root: &Path) -> OsString {
    let mut parts = vec![spec.bin_dir(root)];
    if spec.id != RuntimeId::Node {
        parts.push(root.join("bin"));
    }
    parts.extend(
        ["/usr/bin", "/bin", "/usr/sbin", "/sbin"]
            .into_iter()
            .map(PathBuf::from),
    );
    std::env::join_paths(parts).unwrap_or_else(|_| OsString::from("/usr/bin:/bin"))
}

fn replace_dir(staging: &Path, final_dir: &Path, store: &Path, id: RuntimeId) -> Result<()> {
    if let Some(parent) = final_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let replaced = store.join(format!(
        ".replaced-{}-{}-{}",
        id.as_str(),
        std::process::id(),
        next_nonce()
    ));
    if final_dir.exists() {
        std::fs::rename(final_dir, &replaced)?;
    }
    match std::fs::rename(staging, final_dir) {
        Ok(()) => {
            let _ = std::fs::remove_dir_all(&replaced);
            Ok(())
        }
        Err(error) => {
            if replaced.exists() {
                let _ = std::fs::rename(&replaced, final_dir);
            }
            Err(RuntimeError::Io(error))
        }
    }
}

fn latest_installed(store: &Path, id: RuntimeId) -> Option<String> {
    let root = store.join(id.as_str());
    let entries = std::fs::read_dir(root).ok()?;
    let mut versions = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().ok()?.is_dir() {
            continue;
        }
        let Ok(manifest) = read_manifest(&entry.path()) else {
            continue;
        };
        if manifest.runtime == id {
            versions.push(manifest.version);
        }
    }
    versions
        .into_iter()
        .max_by(|left, right| version_cmp(left, right))
}

fn is_older_than(path: &Path, now: SystemTime, age: Duration) -> Result<bool> {
    let modified = std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(now);
    Ok(now.duration_since(modified).unwrap_or_default() >= age)
}

fn prune_versions(root: &Path, age: Duration, now: SystemTime) -> Result<()> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Ok(());
    };
    let mut versions = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let Ok(manifest) = read_manifest(&entry.path()) else {
            continue;
        };
        versions.push((manifest.installed_at, manifest.version, entry.path()));
    }
    versions.sort_by(|left, right| version_cmp(&right.1, &left.1));
    for (_, _, path) in versions.into_iter().skip(2) {
        if is_older_than(&path, now, age)? {
            let _ = std::fs::remove_dir_all(path);
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn available_bytes(path: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(path.as_ptr(), &mut stat) } == 0 {
        Some(stat.f_bavail as u64 * stat.f_bsize as u64)
    } else {
        None
    }
}

#[cfg(not(target_os = "macos"))]
fn available_bytes(_path: &Path) -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::convert::Infallible;
    use std::net::SocketAddr;

    use axum::body::Body;
    use axum::extract::State as AxumState;
    use axum::http::{header, HeaderMap, StatusCode};
    use axum::response::Response;
    use axum::routing::get;
    use axum::Router;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use sha2::{Digest, Sha256};
    use tar::{Builder, Header};

    use super::super::descriptors::write_manifest;
    use super::super::hex;

    fn sha(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex(&hasher.finalize())
    }

    fn archive(version: &str) -> Vec<u8> {
        let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
        {
            let mut tar = Builder::new(&mut gz);
            for (name, data) in [
                (
                    "package/bin/opencode",
                    format!("#!/bin/sh\necho v{version}\n").into_bytes(),
                ),
                ("package/package.json", br#"{"name":"test"}"#.to_vec()),
            ] {
                let mut header = Header::new_gnu();
                header.set_size(data.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                tar.append_data(&mut header, name, data.as_slice()).unwrap();
            }
            tar.into_inner().unwrap();
        }
        gz.finish().unwrap()
    }

    fn lock(version: &str, archive: &[u8], hash: String) -> LockFile {
        let mut artifacts = BTreeMap::new();
        artifacts.insert(
            "darwin-arm64".into(),
            super::super::descriptors::RuntimeArtifact {
                package: Some("@opencode/cli-darwin-arm64".into()),
                asset: Some(format!("opencode-{version}.tar.gz")),
                url: Some(format!("https://example.test/opencode-{version}.tar.gz")),
                asset_sha256: hash,
                binary_sha256: None,
                download_bytes: Some(archive.len() as u64),
                installed_bytes: Some(archive.len() as u64),
                executable: "bin/opencode".into(),
                executables: vec!["bin/opencode".into()],
                files: BTreeMap::new(),
                root: Some("package".into()),
                strip_components: Some(1),
                format: Some("tar.gz".into()),
            },
        );
        let mut runtimes = BTreeMap::new();
        runtimes.insert(
            "opencode".into(),
            super::super::descriptors::RuntimeLock {
                version: version.into(),
                sdk_version: None,
                tag: None,
                repository: None,
                registry: Some("https://registry.npmjs.org".into()),
                distribution: None,
                artifacts,
            },
        );
        LockFile {
            schema_version: 1,
            runtimes,
            acp: BTreeMap::new(),
        }
    }

    fn manager(store: &Path, lock: LockFile, url: String) -> RuntimeManager {
        RuntimeManager::new(store, lock, RuntimeTarget::DarwinArm64)
            .with_base_url(url)
            .with_headroom(0)
    }

    #[derive(Clone)]
    struct Stub {
        bytes: Arc<Vec<u8>>,
        slow: bool,
    }

    async fn stub_handler(AxumState(state): AxumState<Stub>, headers: HeaderMap) -> Response {
        let total = state.bytes.len();
        let start = headers
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("bytes="))
            .and_then(|value| value.split('-').next())
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0)
            .min(total);
        let slice = state.bytes[start..].to_vec();
        let status = if start > 0 {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        };
        let mut response = if state.slow {
            let stream =
                futures_util::stream::unfold((slice, 0usize), |(bytes, position)| async move {
                    if position >= bytes.len() {
                        return None;
                    }
                    tokio::time::sleep(Duration::from_millis(15)).await;
                    let end = (position + 64).min(bytes.len());
                    let chunk = bytes[position..end].to_vec();
                    Some((
                        Ok::<_, Infallible>(axum::body::Bytes::from(chunk)),
                        (bytes, end),
                    ))
                });
            Response::new(Body::from_stream(stream))
        } else {
            Response::new(Body::from(slice))
        };
        *response.status_mut() = status;
        response.headers_mut().insert(
            header::CONTENT_LENGTH,
            ((total - start) as u64).to_string().parse().unwrap(),
        );
        if start > 0 {
            response.headers_mut().insert(
                header::CONTENT_RANGE,
                format!("bytes {start}-{}/{total}", total - 1)
                    .parse()
                    .unwrap(),
            );
        }
        response
    }

    async fn serve(stub: Stub) -> (String, tokio::task::JoinHandle<()>) {
        let app = Router::new().fallback(get(stub_handler)).with_state(stub);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (format!("http://{addr}"), server)
    }

    #[tokio::test]
    async fn installs_and_emits_monotonic_progress() {
        let version = "2.2.5";
        let bytes = archive(version);
        let (url, server) = serve(Stub {
            bytes: Arc::new(bytes.clone()),
            slow: false,
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path(), lock(version, &bytes, sha(&bytes)), url);
        let mut events = manager.subscribe();
        let snapshot = manager.download(RuntimeId::OpenCode).await.unwrap();
        assert_eq!(snapshot.phase.kind, RuntimePhaseKind::Ready);
        assert_eq!(snapshot.phase.version(), Some(version));
        assert_eq!(snapshot.phase.source, Some(RuntimeSource::Managed));
        let installed = dir.path().join("opencode").join(version);
        assert!(installed.join("bin/opencode").is_file());
        assert!(installed.join("orso-install.json").is_file());
        let spec = manager.spec(RuntimeId::OpenCode).unwrap();
        spec.verify(&installed).unwrap();

        let mut phases = Vec::new();
        let mut progress = Vec::new();
        while let Ok(event) = events.try_recv() {
            phases.push(event.phase.kind);
            if let Some(value) = event.phase.progress {
                progress.push(value);
            }
        }
        assert!(phases.contains(&RuntimePhaseKind::Downloading));
        assert!(phases.contains(&RuntimePhaseKind::Finishing));
        assert_eq!(phases.last(), Some(&RuntimePhaseKind::Ready));
        assert!(
            progress.windows(2).all(|pair| pair[0] <= pair[1]),
            "{progress:?}"
        );
        server.abort();
    }

    #[tokio::test]
    async fn bad_hash_keeps_the_previous_install() {
        let v1 = "1.0.0";
        let old = archive(v1);
        let (url, server) = serve(Stub {
            bytes: Arc::new(old.clone()),
            slow: false,
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let first = manager(dir.path(), lock(v1, &old, sha(&old)), url);
        first.download(RuntimeId::OpenCode).await.unwrap();
        let old_root = dir.path().join("opencode").join(v1);
        let old_hash = hash_file(&old_root.join("bin/opencode")).unwrap();
        server.abort();

        let v2 = "2.0.0";
        let new = archive(v2);
        let (url, server) = serve(Stub {
            bytes: Arc::new(new.clone()),
            slow: false,
        })
        .await;
        let second = manager(dir.path(), lock(v2, &new, "deadbeef".into()), url);
        let error = second.download(RuntimeId::OpenCode).await.unwrap_err();
        assert!(matches!(error, RuntimeError::HashMismatch { .. }));
        assert_eq!(hash_file(&old_root.join("bin/opencode")).unwrap(), old_hash);
        first
            .spec(RuntimeId::OpenCode)
            .unwrap()
            .verify(&old_root)
            .unwrap();
        let phase = second.phase(RuntimeId::OpenCode).unwrap();
        assert_eq!(phase.kind, RuntimePhaseKind::DownloadError);
        assert_eq!(phase.version(), Some(v1));
        server.abort();
    }

    #[tokio::test]
    async fn cancel_removes_the_partial_and_retry_succeeds() {
        let version = "2.2.5";
        let bytes = archive(version);
        let (url, server) = serve(Stub {
            bytes: Arc::new(bytes.clone()),
            slow: true,
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path(), lock(version, &bytes, sha(&bytes)), url);
        let mut events = manager.subscribe();
        let task = tokio::spawn({
            let manager = manager.clone();
            async move { manager.download(RuntimeId::OpenCode).await }
        });
        loop {
            let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
                .await
                .unwrap()
                .unwrap();
            if event.phase.kind == RuntimePhaseKind::Downloading
                && event.phase.progress.unwrap_or(0) >= 10
            {
                break;
            }
        }
        manager.cancel(RuntimeId::OpenCode).unwrap();
        let error = task.await.unwrap().unwrap_err();
        assert!(matches!(error, RuntimeError::Canceled));
        assert_eq!(
            manager.phase(RuntimeId::OpenCode).unwrap().kind,
            RuntimePhaseKind::NotDownloaded
        );
        let partials = std::fs::read_dir(dir.path().join(".downloads"))
            .unwrap()
            .count();
        assert_eq!(partials, 0);
        assert!(!dir.path().join(".staging").exists());

        let retry = manager.retry(RuntimeId::OpenCode).await.unwrap();
        assert_eq!(retry.phase.kind, RuntimePhaseKind::Ready);
        server.abort();
    }

    #[tokio::test]
    async fn retry_after_a_failed_download_succeeds() {
        let version = "2.2.5";
        let bytes = archive(version);
        let (url, server) = serve(Stub {
            bytes: Arc::new(bytes.clone()),
            slow: false,
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let failed = manager(dir.path(), lock(version, &bytes, "00".into()), url.clone());
        assert!(failed.download(RuntimeId::OpenCode).await.is_err());
        assert_eq!(
            failed.phase(RuntimeId::OpenCode).unwrap().kind,
            RuntimePhaseKind::DownloadError
        );
        // A manager with the real hash and the same store retries the same
        // runtime; the failed partial was already removed.
        let good = manager(dir.path(), lock(version, &bytes, sha(&bytes)), url);
        let snapshot = good.retry(RuntimeId::OpenCode).await.unwrap();
        assert_eq!(snapshot.phase.kind, RuntimePhaseKind::Ready);
        server.abort();
    }

    #[tokio::test]
    async fn disk_failure_is_reported_before_downloading() {
        let version = "2.2.5";
        let bytes = archive(version);
        let (url, server) = serve(Stub {
            bytes: Arc::new(bytes.clone()),
            slow: false,
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path(), lock(version, &bytes, sha(&bytes)), url)
            .with_headroom(0)
            .with_disk_available(1);
        let error = manager.download(RuntimeId::OpenCode).await.unwrap_err();
        assert!(matches!(error, RuntimeError::DiskFull { .. }));
        let phase = manager.phase(RuntimeId::OpenCode).unwrap();
        assert_eq!(phase.kind, RuntimePhaseKind::DownloadError);
        assert!(phase.message.unwrap().contains("disk space"));
        server.abort();
    }

    #[tokio::test]
    async fn status_starts_not_downloaded_and_detects_existing_installs() {
        let version = "2.2.5";
        let bytes = archive(version);
        let (url, server) = serve(Stub {
            bytes: Arc::new(bytes.clone()),
            slow: false,
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let first = manager(dir.path(), lock(version, &bytes, sha(&bytes)), url);
        assert_eq!(
            first.phase(RuntimeId::OpenCode).unwrap().kind,
            RuntimePhaseKind::NotDownloaded
        );
        first.download(RuntimeId::OpenCode).await.unwrap();
        server.abort();
        let second = RuntimeManager::new(
            dir.path(),
            lock(version, &bytes, sha(&bytes)),
            RuntimeTarget::DarwinArm64,
        );
        assert_eq!(
            second.phase(RuntimeId::OpenCode).unwrap().kind,
            RuntimePhaseKind::Ready
        );
        assert_eq!(
            second.phase(RuntimeId::OpenCode).unwrap().version(),
            Some(version)
        );
    }

    #[tokio::test]
    async fn only_one_download_per_runtime() {
        let version = "2.2.5";
        let bytes = archive(version);
        let (url, server) = serve(Stub {
            bytes: Arc::new(bytes.clone()),
            slow: true,
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let manager = manager(dir.path(), lock(version, &bytes, sha(&bytes)), url);
        let task = tokio::spawn({
            let manager = manager.clone();
            async move { manager.download(RuntimeId::OpenCode).await }
        });
        // Wait until the first download is in flight — a fixed sleep races on fast CI.
        for _ in 0..200 {
            if manager.phase(RuntimeId::OpenCode).unwrap().kind == RuntimePhaseKind::Downloading {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            manager.phase(RuntimeId::OpenCode).unwrap().kind,
            RuntimePhaseKind::Downloading,
            "first download never entered Downloading"
        );
        let busy = manager.download(RuntimeId::OpenCode).await.unwrap_err();
        assert!(matches!(busy, RuntimeError::Busy(RuntimeId::OpenCode)));
        manager.cancel(RuntimeId::OpenCode).unwrap();
        let _ = task.await;
        server.abort();
    }

    #[test]
    fn cleanup_keeps_the_current_and_one_previous_version() {
        let dir = tempfile::tempdir().unwrap();
        let version = "2.2.5";
        let bytes = archive(version);
        let manager = RuntimeManager::new(
            dir.path(),
            lock(version, &bytes, sha(&bytes)),
            RuntimeTarget::DarwinArm64,
        );
        for old in ["0.9.0", "1.0.0", "2.0.0"] {
            let root = dir.path().join("opencode").join(old);
            std::fs::create_dir_all(root.join("bin")).unwrap();
            std::fs::write(root.join("bin/opencode"), "old").unwrap();
            let manifest = InstallManifest {
                schema_version: 1,
                runtime: RuntimeId::OpenCode,
                version: old.into(),
                target: "darwin-arm64".into(),
                source_url: "x".into(),
                installed_at: 0,
                executable: "bin/opencode".into(),
                files: BTreeMap::new(),
            };
            write_manifest(&root, &manifest).unwrap();
        }
        manager.cleanup_expired(Duration::from_secs(0)).unwrap();
        assert!(dir.path().join("opencode/2.0.0").exists());
        assert!(dir.path().join("opencode/1.0.0").exists());
        assert!(!dir.path().join("opencode/0.9.0").exists());
    }

    #[test]
    fn partial_path_and_install_dir_are_stable() {
        let dir = tempfile::tempdir().unwrap();
        let version = "2.2.5";
        let bytes = archive(version);
        let manager = RuntimeManager::new(
            dir.path(),
            lock(version, &bytes, sha(&bytes)),
            RuntimeTarget::DarwinArm64,
        );
        let spec = manager.spec(RuntimeId::OpenCode).unwrap();
        assert!(manager
            .partial_path(&spec)
            .to_string_lossy()
            .ends_with(".downloads/opencode-2.2.5.partial"));
        assert_eq!(
            manager.install_dir(&spec),
            dir.path().join("opencode/2.2.5")
        );
    }

    #[test]
    fn snapshot_and_event_json_match_the_setup_shape() {
        let snapshot = RuntimeSnapshot {
            runtime: RuntimeId::OpenCode,
            phase: RuntimePhase::downloading(Some(42), None),
            download_bytes: Some(10),
            installed_bytes: Some(20),
        };
        let value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(value["runtime"], "opencode");
        assert_eq!(value["phase"], "downloading");
        assert_eq!(value["progress"], 42);
        assert_eq!(value["downloadBytes"], 10);
        assert_eq!(value["installedBytes"], 20);

        let event = RuntimeEvent {
            runtime: RuntimeId::Claude,
            phase: RuntimePhase::ready("2.1.318", RuntimeSource::Managed),
        };
        let value = serde_json::to_value(&event).unwrap();
        assert_eq!(value["runtime"], "claude");
        assert_eq!(value["phase"], "ready");
        assert_eq!(value["source"], "managed");
        assert_eq!(value["version"], "2.1.318");
    }
}

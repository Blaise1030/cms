//! Managed ACP adapters for Claude and Codex.
//!
//! OpenCode and Cursor speak ACP themselves; Claude and Codex need a small
//! JavaScript bridge. The bridge is pinned in `agent-runtime.lock.json` and
//! installed into `<runtime store>/acp/<provider>/` with the managed `npm`, so a
//! Finder-launched app never depends on the user's Node or a global install.
//!
//! Layout of one adapter install:
//!
//! ```text
//! runtimes/acp/claude/
//! ├── node_modules/@agentclientprotocol/claude-agent-acp/…
//! ├── package.json
//! ├── package-lock.json
//! └── orso-adapter.json        # version, package and the bin path
//! ```
//!
//! Installs happen in `runtimes/.staging-acp-<provider>-…` and are renamed into
//! place only after the package version and bin have been verified. A failed
//! install leaves the previous adapter directory untouched.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as AsyncMutex;

use super::descriptors::{read_manifest, version_cmp, LockFile, RuntimeId};
use super::{Result, RuntimeError};

/// How long `npm install` may take before the attempt is abandoned.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(300);
/// How often a running install checks for cancellation and its deadline.
const POLL: Duration = Duration::from_millis(100);

/// The pinned adapter for one provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpAdapterSpec {
    pub provider: RuntimeId,
    pub package: String,
    pub version: String,
}

impl AcpAdapterSpec {
    /// Look the adapter up in the lock file. Only Claude and Codex have one.
    pub fn for_provider(provider: RuntimeId, lock: &LockFile) -> Result<Self> {
        if !matches!(provider, RuntimeId::Claude | RuntimeId::Codex) {
            return Err(RuntimeError::NotFound(format!(
                "{} does not need an adapter",
                provider.as_str()
            )));
        }
        let package = lock.acp.get(provider.as_str()).ok_or_else(|| {
            RuntimeError::Adapter(format!(
                "Chat with {} is not available in this build.",
                display(provider)
            ))
        })?;
        Ok(Self {
            provider,
            package: package.package.clone(),
            version: package.version.clone(),
        })
    }

    /// Install prefix: `<store>/acp/<provider>`.
    pub fn prefix(&self, store: &Path) -> PathBuf {
        store.join("acp").join(self.provider.as_str())
    }

    /// `package@version`, the npm install target.
    pub fn package_target(&self) -> String {
        format!("{}@{}", self.package, self.version)
    }

    /// Package directory inside a prefix (or a staging prefix).
    fn package_dir(&self, prefix: &Path) -> PathBuf {
        prefix.join("node_modules").join(&self.package)
    }
}

/// `orso-adapter.json`, written beside a completed adapter install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterManifest {
    pub schema_version: u32,
    pub provider: RuntimeId,
    pub package: String,
    pub version: String,
    /// Bin path, relative to the install prefix.
    pub bin: String,
    pub installed_at: u64,
}

/// What the setup screen should say about an adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterState {
    /// This provider does not use an adapter.
    NotNeeded,
    /// No adapter is installed yet.
    Missing,
    /// An install is running now.
    Installing,
    /// A verified adapter is installed.
    Ready { version: String },
    /// The last attempt failed; `message` is safe to show to the user.
    Failed { message: String },
}

impl AdapterState {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }
}

#[derive(Default)]
struct Slot {
    installing: bool,
    cancel: Option<Arc<AtomicBool>>,
    error: Option<String>,
}

/// Installs and reports the pinned ACP adapters. Cheap to clone; all state is
/// shared with the [`super::RuntimeManager`] that owns it.
#[derive(Clone)]
pub struct AdapterInstaller {
    store: PathBuf,
    lock: Arc<LockFile>,
    slots: Arc<Mutex<HashMap<RuntimeId, Slot>>>,
    gate: Arc<AsyncMutex<()>>,
}

impl AdapterInstaller {
    pub(crate) fn new(store: impl Into<PathBuf>, lock: Arc<LockFile>) -> Self {
        Self {
            store: store.into(),
            lock,
            slots: Arc::new(Mutex::new(HashMap::new())),
            gate: Arc::new(AsyncMutex::new(())),
        }
    }

    pub fn store(&self) -> &Path {
        &self.store
    }

    pub fn spec(&self, provider: RuntimeId) -> Result<AcpAdapterSpec> {
        AcpAdapterSpec::for_provider(provider, &self.lock)
    }

    /// A verified install for this provider, if any.
    pub fn installed(&self, provider: RuntimeId) -> Option<AdapterManifest> {
        let spec = self.spec(provider).ok()?;
        read_adapter(&self.store, &spec)
    }

    /// Current user-facing state.
    pub fn state(&self, provider: RuntimeId) -> AdapterState {
        let Ok(spec) = self.spec(provider) else {
            return AdapterState::NotNeeded;
        };
        if let Some(manifest) = read_adapter(&self.store, &spec) {
            return AdapterState::Ready {
                version: manifest.version,
            };
        }
        let slots = lock(&self.slots);
        let Some(slot) = slots.get(&provider) else {
            return AdapterState::Missing;
        };
        if slot.installing {
            return AdapterState::Installing;
        }
        match &slot.error {
            Some(message) => AdapterState::Failed {
                message: message.clone(),
            },
            None => AdapterState::Missing,
        }
    }

    /// Whether installing this adapter needs a managed Node that is not there
    /// yet. Used to give a precise reason instead of a failed install.
    pub fn node_missing(&self, provider: RuntimeId) -> bool {
        self.spec(provider).is_ok() && latest_node_bin(&self.store).is_none()
    }

    /// Install the adapter if it is not already present. Idempotent and safe to
    /// call from a status poll.
    pub async fn ensure(&self, provider: RuntimeId) -> Result<AdapterManifest> {
        let spec = self.spec(provider)?;
        if let Some(manifest) = read_adapter(&self.store, &spec) {
            self.clear_error(provider);
            return Ok(manifest);
        }
        // Mark the intent before waiting for the gate so repeated status polls
        // do not queue duplicate installs behind another adapter.
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut slots = lock(&self.slots);
            let slot = slots.entry(provider).or_default();
            slot.installing = true;
            slot.cancel = Some(cancel.clone());
        }
        // Serialise installs: adapters share the managed Node and npm cache.
        let _guard = self.gate.lock().await;
        if let Some(manifest) = read_adapter(&self.store, &spec) {
            let mut slots = lock(&self.slots);
            let slot = slots.entry(provider).or_default();
            slot.installing = false;
            slot.cancel = None;
            slot.error = None;
            return Ok(manifest);
        }
        if cancel.load(Ordering::SeqCst) {
            let mut slots = lock(&self.slots);
            let slot = slots.entry(provider).or_default();
            slot.installing = false;
            slot.cancel = None;
            slot.error = Some(format!(
                "Setting up chat with {} was cancelled.",
                display(provider)
            ));
            return Err(RuntimeError::Canceled);
        }

        {
            let mut slots = lock(&self.slots);
            let slot = slots.entry(provider).or_default();
            slot.installing = true;
            slot.error = None;
            slot.cancel = Some(cancel.clone());
        }

        let result = self.install(&spec, &cancel).await;
        {
            let mut slots = lock(&self.slots);
            let slot = slots.entry(provider).or_default();
            slot.installing = false;
            slot.cancel = None;
            slot.error = match &result {
                Ok(_) => None,
                Err(RuntimeError::Canceled) => Some(format!(
                    "Setting up chat with {} was cancelled.",
                    display(provider)
                )),
                Err(error) => Some(error.to_string()),
            };
        }
        result
    }

    /// Stop a running install. The final prefix is only replaced on success, so
    /// the previous adapter keeps working.
    pub fn cancel(&self, provider: RuntimeId) {
        if let Some(slot) = lock(&self.slots).get(&provider) {
            if let Some(cancel) = &slot.cancel {
                cancel.store(true, Ordering::SeqCst);
            }
        }
    }

    fn clear_error(&self, provider: RuntimeId) {
        if let Some(slot) = lock(&self.slots).get_mut(&provider) {
            slot.error = None;
        }
    }

    async fn install(
        &self,
        spec: &AcpAdapterSpec,
        cancel: &Arc<AtomicBool>,
    ) -> Result<AdapterManifest> {
        let provider = spec.provider;
        let node_bin = latest_node_bin(&self.store).ok_or_else(|| {
            RuntimeError::Adapter(format!(
                "Set up the site builder before chatting with {}.",
                display(provider)
            ))
        })?;
        let npm = node_bin.join("npm");
        if !is_executable(&npm) {
            return Err(RuntimeError::Adapter(format!(
                "Set up the site builder before chatting with {}.",
                display(provider)
            )));
        }

        let prefix = spec.prefix(&self.store);
        let staging = self.store.join(format!(
            ".staging-acp-{}-{}-{}",
            provider.as_str(),
            std::process::id(),
            nonce()
        ));
        if staging.exists() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        std::fs::create_dir_all(&staging)?;
        let log_dir = self.store.join(".downloads");
        std::fs::create_dir_all(&log_dir)?;
        let log = log_dir.join(format!("acp-{}-{}.log", provider.as_str(), nonce()));

        let result = run_npm(&npm, &node_bin, &staging, spec, cancel, &log).await;
        if let Err(error) = result {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(match error {
                RuntimeError::Canceled => RuntimeError::Canceled,
                RuntimeError::Adapter(message) => RuntimeError::Adapter(message),
                other => RuntimeError::Adapter(install_failed(provider, &other)),
            });
        }

        let package_dir = spec.package_dir(&staging);
        let version = match package_version(&package_dir) {
            Ok(version) => version,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(error);
            }
        };
        if version != spec.version {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(RuntimeError::Adapter(format!(
                "The chat add-on for {} doesn't match. Set it up again.",
                display(provider)
            )));
        }
        let bin = match package_bin(&staging, &package_dir, provider) {
            Ok(bin) => bin,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(error);
            }
        };
        let manifest = AdapterManifest {
            schema_version: 1,
            provider,
            package: spec.package.clone(),
            version,
            bin,
            installed_at: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0),
        };
        write_adapter(&staging, &manifest)?;
        if let Err(error) = replace_dir(&staging, &prefix, &self.store, provider) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
        read_adapter(&self.store, spec).ok_or_else(|| {
            RuntimeError::Adapter(format!(
                "Chat with {} could not be set up. Try again.",
                display(provider)
            ))
        })
    }
}

/// Absolute argv for a managed adapter: `[<managed node>, <adapter bin>]`.
pub fn adapter_argv(store: &Path, provider: RuntimeId, lock: &LockFile) -> Option<Vec<String>> {
    let spec = AcpAdapterSpec::for_provider(provider, lock).ok()?;
    let manifest = read_adapter(store, &spec)?;
    let node_bin = latest_node_bin(store)?;
    let node = node_bin.join("node");
    if !node.is_file() {
        return None;
    }
    let bin = spec.prefix(store).join(&manifest.bin);
    if !bin.is_file() {
        return None;
    }
    Some(vec![
        node.to_string_lossy().into_owned(),
        bin.to_string_lossy().into_owned(),
    ])
}

/// The newest managed Node `bin` directory, if Node is installed.
pub fn latest_node_bin(store: &Path) -> Option<PathBuf> {
    let root = store.join(RuntimeId::Node.as_str());
    let mut newest: Option<(String, PathBuf)> = None;
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let Ok(manifest) = read_manifest(&entry.path()) else {
            continue;
        };
        if manifest.runtime != RuntimeId::Node {
            continue;
        }
        let node = entry.path().join(&manifest.executable);
        if !node.is_file() {
            continue;
        }
        if newest
            .as_ref()
            .is_some_and(|(version, _)| version_cmp(&manifest.version, version).is_le())
        {
            continue;
        }
        newest = Some((
            manifest.version,
            node.parent().map(Path::to_path_buf).unwrap_or_default(),
        ));
    }
    newest.map(|(_, bin)| bin)
}

/// Read and validate `orso-adapter.json` below `spec.prefix(store)`.
pub fn read_adapter(store: &Path, spec: &AcpAdapterSpec) -> Option<AdapterManifest> {
    let prefix = spec.prefix(store);
    let text = std::fs::read_to_string(prefix.join("orso-adapter.json")).ok()?;
    let manifest: AdapterManifest = serde_json::from_str(&text).ok()?;
    if manifest.schema_version != 1
        || manifest.provider != spec.provider
        || manifest.package != spec.package
        || manifest.version != spec.version
    {
        return None;
    }
    let bin = prefix.join(&manifest.bin);
    if !bin.is_file() || !bin.starts_with(&prefix) {
        return None;
    }
    Some(manifest)
}

fn write_adapter(prefix: &Path, manifest: &AdapterManifest) -> Result<()> {
    let text = serde_json::to_string_pretty(manifest)
        .map_err(|error| RuntimeError::Adapter(error.to_string()))?;
    std::fs::write(prefix.join("orso-adapter.json"), format!("{text}\n"))?;
    Ok(())
}

fn package_version(package_dir: &Path) -> Result<String> {
    let package_json = read_package_json(package_dir)?;
    package_json
        .get("version")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| RuntimeError::Adapter("The chat add-on is missing its version.".into()))
}

/// Resolve the package's `bin` entry to a path relative to the install prefix.
fn package_bin(prefix: &Path, package_dir: &Path, provider: RuntimeId) -> Result<String> {
    let package_json = read_package_json(package_dir)?;
    let bin = package_json.get("bin");
    let relative = match bin {
        Some(serde_json::Value::String(path)) => Some(path.clone()),
        Some(serde_json::Value::Object(entries)) => {
            let wanted = match provider {
                RuntimeId::Claude => "claude-agent-acp",
                RuntimeId::Codex => "codex-acp",
                _ => {
                    return Err(RuntimeError::Adapter(
                        "This assistant has no adapter.".into(),
                    ))
                }
            };
            entries
                .get(wanted)
                .or_else(|| {
                    entries
                        .iter()
                        .find(|(name, _)| name.contains("acp"))
                        .map(|(_, value)| value)
                })
                .or_else(|| {
                    (entries.len() == 1)
                        .then(|| entries.values().next())
                        .flatten()
                })
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        }
        _ => None,
    };
    let relative = relative.ok_or_else(|| {
        RuntimeError::Adapter("The chat add-on is missing the program to start.".into())
    })?;
    let candidate = Path::new(&relative);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir))
    {
        return Err(RuntimeError::Adapter(
            "The chat add-on points outside its own folder.".into(),
        ));
    }
    let bin = package_dir.join(candidate);
    if !bin.is_file() {
        return Err(RuntimeError::Adapter(
            "The chat add-on is missing the program to start.".into(),
        ));
    }
    let package_relative = package_dir
        .strip_prefix(prefix)
        .map_err(|_| RuntimeError::Adapter("The chat add-on is outside its folder.".into()))?;
    Ok(package_relative
        .join(candidate)
        .to_string_lossy()
        .replace('\\', "/"))
}

fn read_package_json(package_dir: &Path) -> Result<serde_json::Value> {
    let path = package_dir.join("package.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|_| RuntimeError::Adapter("The chat add-on did not finish downloading.".into()))?;
    serde_json::from_str(&text)
        .map_err(|_| RuntimeError::Adapter("The chat add-on is not usable. Try again.".into()))
}

async fn run_npm(
    npm: &Path,
    node_bin: &Path,
    prefix: &Path,
    spec: &AcpAdapterSpec,
    cancel: &Arc<AtomicBool>,
    log: &Path,
) -> Result<()> {
    let file = std::fs::File::create(log)?;
    let mut command = tokio::process::Command::new(npm);
    command
        .args(["install", "--prefix"])
        .arg(prefix)
        .args(["--no-audit", "--no-fund", "--omit=dev"])
        .arg(spec.package_target())
        .env_clear()
        .env("HOME", std::env::var_os("HOME").unwrap_or_default())
        .env("PATH", npm_path(node_bin))
        .env(
            "npm_config_cache",
            log.parent().unwrap_or(Path::new(".")).join(".npm-cache"),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::from(file.try_clone()?))
        .stderr(Stdio::from(file))
        .kill_on_drop(true)
        .process_group(0);
    let mut child = command.spawn().map_err(|error| {
        RuntimeError::Adapter(format!(
            "Couldn't finish setting up chat with {}. Check your connection and try again. ({error})",
            display(spec.provider)
        ))
    })?;
    let pid = child.id();
    let deadline = Instant::now() + INSTALL_TIMEOUT;
    loop {
        tokio::select! {
            status = child.wait() => {
                let status = status.map_err(|error| {
                    RuntimeError::Adapter(install_failed(spec.provider, &RuntimeError::Io(error)))
                })?;
                if status.success() {
                    return Ok(());
                }
                return Err(RuntimeError::Adapter(install_failed_text(
                    spec.provider,
                    "The download did not finish.",
                )));
            }
            _ = tokio::time::sleep(POLL) => {
                if cancel.load(Ordering::SeqCst) {
                    kill_group(pid);
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    return Err(RuntimeError::Canceled);
                }
                if Instant::now() >= deadline {
                    kill_group(pid);
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    return Err(RuntimeError::Adapter(install_failed_text(
                        spec.provider,
                        "The download took too long.",
                    )));
                }
            }
        }
    }
}

fn npm_path(node_bin: &Path) -> std::ffi::OsString {
    let mut dirs = vec![node_bin.to_path_buf()];
    dirs.extend(
        ["/usr/bin", "/bin", "/usr/sbin", "/sbin"]
            .into_iter()
            .map(PathBuf::from),
    );
    std::env::join_paths(dirs).unwrap_or_else(|_| std::ffi::OsString::from("/usr/bin:/bin"))
}

fn install_failed(provider: RuntimeId, error: &RuntimeError) -> String {
    match error {
        RuntimeError::NotFound(message) => install_failed_text(provider, message),
        RuntimeError::Io(error) => install_failed_text(provider, &error.to_string()),
        other => install_failed_text(provider, &other.to_string()),
    }
}

fn install_failed_text(provider: RuntimeId, _detail: &str) -> String {
    format!(
        "Couldn't finish setting up chat with {}. Check your connection and try again.",
        display(provider)
    )
}

fn display(provider: RuntimeId) -> &'static str {
    match provider {
        RuntimeId::Claude => "Claude",
        RuntimeId::Codex => "Codex",
        RuntimeId::OpenCode => "OpenCode",
        RuntimeId::Cursor => "Cursor",
        RuntimeId::Node => "the site builder",
    }
}

fn replace_dir(staging: &Path, final_dir: &Path, store: &Path, provider: RuntimeId) -> Result<()> {
    if let Some(parent) = final_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let replaced = store.join(format!(
        ".replaced-acp-{}-{}-{}",
        provider.as_str(),
        std::process::id(),
        nonce()
    ));
    if final_dir.exists() {
        std::fs::rename(final_dir, &replaced)?;
    }
    match std::fs::rename(staging, final_dir) {
        Ok(()) => {
            let _ = std::fs::remove_dir_all(&replaced);
            Ok(())
        }
        Err(_error) => {
            if replaced.exists() {
                let _ = std::fs::rename(&replaced, final_dir);
            }
            Err(RuntimeError::Adapter(format!(
                "Chat with {} could not be set up. Try again.",
                display(provider)
            )))
        }
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn nonce() -> u64 {
    use std::sync::atomic::AtomicU64;
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let counter = NEXT.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ counter.rotate_left(17)
}

fn kill_group(pid: Option<u32>) {
    #[cfg(unix)]
    if let Some(pid) = pid {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
    let _ = pid;
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::descriptors::{write_manifest, InstallFile, RuntimeArtifact, RuntimeLock};
    use std::collections::BTreeMap;

    const CLAUDE_VERSION: &str = "2.1.318";
    const ADAPTER_VERSION: &str = "0.100.0";
    const PACKAGE: &str = "@agentclientprotocol/claude-agent-acp";

    fn lock_with(adapter_version: &str) -> LockFile {
        let artifact = RuntimeArtifact {
            package: Some("@anthropic-ai/claude-agent-sdk-darwin-arm64".into()),
            asset: Some("claude.tar.gz".into()),
            url: Some("https://example.test/claude.tar.gz".into()),
            asset_sha256: "00".into(),
            binary_sha256: None,
            download_bytes: None,
            installed_bytes: None,
            executable: "bin/claude".into(),
            executables: vec!["bin/claude".into()],
            files: BTreeMap::new(),
            root: Some("package".into()),
            strip_components: Some(1),
            format: Some("tar.gz".into()),
        };
        LockFile {
            schema_version: 1,
            runtimes: BTreeMap::from([
                (
                    "node".into(),
                    RuntimeLock {
                        version: "26.13.0".into(),
                        sdk_version: None,
                        tag: None,
                        repository: None,
                        registry: None,
                        distribution: None,
                        artifacts: BTreeMap::from([(
                            "darwin-arm64".into(),
                            RuntimeArtifact {
                                executable: "bin/node".into(),
                                ..artifact.clone()
                            },
                        )]),
                    },
                ),
                (
                    "claude".into(),
                    RuntimeLock {
                        version: CLAUDE_VERSION.into(),
                        sdk_version: None,
                        tag: None,
                        repository: None,
                        registry: None,
                        distribution: None,
                        artifacts: BTreeMap::from([("darwin-arm64".into(), artifact)]),
                    },
                ),
            ]),
            acp: BTreeMap::from([(
                "claude".into(),
                super::super::descriptors::AcpPackage {
                    package: PACKAGE.into(),
                    version: adapter_version.into(),
                },
            )]),
        }
    }

    fn install_runtime(store: &Path, id: RuntimeId, version: &str, executable: &str) -> PathBuf {
        let root = store.join(id.as_str()).join(version);
        let path = root.join(executable);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "#!/bin/sh\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let manifest = super::super::descriptors::InstallManifest {
            schema_version: 1,
            runtime: id,
            version: version.into(),
            target: "darwin-arm64".into(),
            source_url: "https://example.test".into(),
            installed_at: 0,
            executable: executable.into(),
            files: BTreeMap::from([(
                executable.into(),
                InstallFile {
                    sha256: "00".into(),
                    size: 10,
                    mode: 0o755,
                },
            )]),
        };
        write_manifest(&root, &manifest).unwrap();
        root
    }

    /// A fake `npm` that writes a plausible adapter package tree. `fail_marker`
    /// makes it exit with an npm-style error.
    fn fake_npm(store: &Path, version: &str, fail_marker: &Path, calls: &Path) {
        let script = format!(
            r#"#!/bin/sh
echo run >> "{calls}"
if [ -f "{fail}" ]; then
  echo "npm ERR! simulated network failure" >&2
  exit 1
fi
prefix=""
pkg=""
while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) prefix="$2"; shift 2 ;;
    *) pkg="$1"; shift ;;
  esac
done
base=${{pkg%@*}}
mkdir -p "$prefix/node_modules/$base/dist"
cat > "$prefix/node_modules/$base/package.json" <<EOF
{{"name":"$base","version":"{version}","bin":{{"claude-agent-acp":"dist/index.js"}}}}
EOF
printf '#!/usr/bin/env node\n' > "$prefix/node_modules/$base/dist/index.js"
"#,
            calls = calls.display(),
            fail = fail_marker.display(),
            version = version
        );
        let bin = store.join("node").join("26.13.0").join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let npm = bin.join("npm");
        std::fs::write(&npm, script).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&npm, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn setup(version: &str, fail_marker: &Path) -> (tempfile::TempDir, PathBuf, LockFile, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("runtimes");
        install_runtime(&store, RuntimeId::Node, "26.13.0", "bin/node");
        install_runtime(&store, RuntimeId::Claude, CLAUDE_VERSION, "bin/claude");
        let calls = dir.path().join("calls");
        fake_npm(&store, version, fail_marker, &calls);
        (dir, store, lock_with(ADAPTER_VERSION), calls)
    }

    #[tokio::test]
    async fn installs_the_pinned_adapter_and_resolves_argv() {
        let fail = std::path::PathBuf::from("/nonexistent/fail");
        let (dir, store, lock, calls) = setup(ADAPTER_VERSION, &fail);
        let installer = AdapterInstaller::new(&store, Arc::new(lock.clone()));
        let manifest = installer.ensure(RuntimeId::Claude).await.unwrap();
        assert_eq!(manifest.version, ADAPTER_VERSION);
        assert_eq!(manifest.package, PACKAGE);
        assert!(manifest.bin.ends_with("dist/index.js"));
        assert_eq!(
            installer.state(RuntimeId::Claude),
            AdapterState::Ready {
                version: ADAPTER_VERSION.into()
            }
        );

        let argv = adapter_argv(&store, RuntimeId::Claude, &lock).unwrap();
        assert_eq!(
            argv,
            [
                store
                    .join("node/26.13.0/bin/node")
                    .to_string_lossy()
                    .into_owned(),
                store
                    .join("acp/claude")
                    .join(&manifest.bin)
                    .to_string_lossy()
                    .into_owned(),
            ]
        );

        // Idempotent: a second call does not run npm again.
        installer.ensure(RuntimeId::Claude).await.unwrap();
        assert_eq!(std::fs::read_to_string(&calls).unwrap().lines().count(), 1);
        assert!(dir
            .path()
            .join("runtimes/acp/claude/orso-adapter.json")
            .is_file());
    }

    #[tokio::test]
    async fn version_mismatch_fails_without_installing() {
        let fail = std::path::PathBuf::from("/nonexistent/fail");
        let (_dir, store, lock, _calls) = setup("9.9.9", &fail);
        let installer = AdapterInstaller::new(&store, Arc::new(lock));
        let error = installer.ensure(RuntimeId::Claude).await.unwrap_err();
        assert!(matches!(error, RuntimeError::Adapter(_)), "{error}");
        assert!(matches!(
            installer.state(RuntimeId::Claude),
            AdapterState::Failed { .. }
        ));
        assert!(!store.join("acp/claude").exists());
        assert!(!store.read_dir().unwrap().flatten().any(|entry| entry
            .file_name()
            .to_string_lossy()
            .starts_with(".staging-acp-")));
    }

    #[tokio::test]
    async fn failed_update_keeps_the_previous_adapter() {
        let dir = tempfile::tempdir().unwrap();
        let fail = dir.path().join("fail-now");
        let (_dir, store, lock, _calls) = setup(ADAPTER_VERSION, &fail);
        let installer = AdapterInstaller::new(&store, Arc::new(lock.clone()));
        let previous = installer.ensure(RuntimeId::Claude).await.unwrap();

        // A newer pin, with a failing npm: the old install must survive.
        std::fs::write(&fail, "1").unwrap();
        let next = AdapterInstaller::new(&store, Arc::new(lock_with("0.101.0")));
        let error = next.ensure(RuntimeId::Claude).await.unwrap_err();
        assert!(matches!(error, RuntimeError::Adapter(_)));
        let previous_bin = store.join("acp/claude").join(&previous.bin);
        assert!(previous_bin.is_file());
        assert!(matches!(
            next.state(RuntimeId::Claude),
            AdapterState::Failed { .. }
        ));
        assert!(adapter_argv(&store, RuntimeId::Claude, &lock).is_some());
    }

    #[test]
    fn resolves_the_codex_bin_entry() {
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("acp/codex");
        let package = prefix.join("node_modules/@agentclientprotocol/codex-acp");
        std::fs::create_dir_all(package.join("bin")).unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"@agentclientprotocol/codex-acp","version":"2.5.0","bin":{"codex-acp":"bin/codex-acp.js"}}"#,
        )
        .unwrap();
        std::fs::write(package.join("bin/codex-acp.js"), "#!/usr/bin/env node\n").unwrap();
        let relative = package_bin(&prefix, &package, RuntimeId::Codex).unwrap();
        assert_eq!(
            relative,
            "node_modules/@agentclientprotocol/codex-acp/bin/codex-acp.js"
        );
        let mut lock = lock_with(ADAPTER_VERSION);
        lock.acp.insert(
            "codex".into(),
            super::super::descriptors::AcpPackage {
                package: "@agentclientprotocol/codex-acp".into(),
                version: "2.5.0".into(),
            },
        );
        let spec = AcpAdapterSpec::for_provider(RuntimeId::Codex, &lock).unwrap();
        assert_eq!(
            spec.package_target(),
            "@agentclientprotocol/codex-acp@2.5.0"
        );
    }

    #[tokio::test]
    async fn missing_node_gives_a_plain_reason() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("runtimes");
        let installer = AdapterInstaller::new(&store, Arc::new(lock_with(ADAPTER_VERSION)));
        let error = installer.ensure(RuntimeId::Claude).await.unwrap_err();
        let message = error.to_string();
        assert!(message.contains("site builder"), "{message}");
        assert!(!message.contains("npm"), "{message}");
    }

    #[tokio::test]
    async fn cancel_stops_the_install_and_keeps_the_previous_install() {
        let dir = tempfile::tempdir().unwrap();
        let fail = dir.path().join("never");
        let (_dir, store, lock, _calls) = setup(ADAPTER_VERSION, &fail);
        // Slow npm so cancel lands mid-install.
        let npm = store.join("node/26.13.0/bin/npm");
        let mut script = std::fs::read_to_string(&npm).unwrap();
        script = script.replace("echo run >>", "sleep 5\necho run >>");
        std::fs::write(&npm, script).unwrap();

        let installer = AdapterInstaller::new(&store, Arc::new(lock));
        let task = {
            let installer = installer.clone();
            tokio::spawn(async move { installer.ensure(RuntimeId::Claude).await })
        };
        tokio::time::sleep(Duration::from_millis(150)).await;
        installer.cancel(RuntimeId::Claude);
        let result = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(result, Err(RuntimeError::Canceled)), "{result:?}");
        assert!(matches!(
            installer.state(RuntimeId::Claude),
            AdapterState::Failed { .. }
        ));
        assert!(!store.join("acp/claude").exists());
    }
}

//! Resolve a provider to a managed runtime or the user's own CLI.
//!
//! Managed installs are always addressed by absolute path. `PATH` is only used
//! to find a system fallback and to build the environment for child processes:
//! the git shim first, then managed Node, managed provider bins, the user's
//! login-shell `PATH`, then the daemon's own `PATH`.
//!
//! `agents.json` and `ORSO_ACP_ARGV` remain the highest-precedence overrides.
//! `providers.rs` is untouched; the later wiring agent passes those overrides to
//! [`RuntimeResolver::with_agents_acp`] / [`acp_precedence`].

use std::collections::{BTreeMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::providers::Provider;

use super::descriptors::{
    read_manifest, version_cmp, version_meets_minimum, LockFile, RuntimeId, RuntimeSpec,
    RuntimeTarget,
};
use super::{Result, RuntimeError};

/// Where a provider executable came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolveSource {
    Managed,
    System,
}

/// Which override supplied an ACP argv.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AcpSource {
    Env,
    AgentsConfig,
    Managed,
    System,
}

/// A resolved provider executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProvider {
    pub id: String,
    pub executable: PathBuf,
    pub version: Option<String>,
    pub source: ResolveSource,
    pub managed: bool,
}

impl ResolvedProvider {
    /// Base argv for an interactive launch.
    pub fn argv(&self) -> Vec<String> {
        vec![self.executable.to_string_lossy().into_owned()]
    }

    /// Directory to add to `PATH` for this provider.
    pub fn bin_dir(&self) -> PathBuf {
        self.executable
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// ACP argv and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpResolution {
    pub argv: Vec<String>,
    pub source: AcpSource,
}

/// One managed install found in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedInstall {
    pub id: RuntimeId,
    pub version: String,
    pub root: PathBuf,
}

impl ManagedInstall {
    /// Executable path for this install.
    pub fn executable(&self, spec: &RuntimeSpec) -> PathBuf {
        spec.executable_path(&self.root)
    }
}

/// ACP overrides parsed from `agents.json` (or another registry).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentAcpOverrides {
    entries: BTreeMap<String, Vec<String>>,
}

impl AgentAcpOverrides {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, provider_id: impl Into<String>, argv: Vec<String>) {
        if !argv.is_empty() {
            self.entries.insert(provider_id.into(), argv);
        }
    }

    /// Collect the explicit `acp` arrays from a provider registry. The caller
    /// should only pass entries actually read from `agents.json`: the built-in
    /// defaults are not overrides.
    pub fn from_providers(providers: &[Provider]) -> Self {
        let mut overrides = Self::new();
        for provider in providers {
            if !provider.acp.is_empty() {
                overrides.insert(provider.id.clone(), provider.acp.clone());
            }
        }
        overrides
    }

    pub fn get(&self, provider_id: &str) -> Option<&Vec<String>> {
        self.entries.get(provider_id)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Precedence for ACP argv: environment, `agents.json`, managed, system.
pub fn acp_precedence(
    environment: Option<Vec<String>>,
    agents: Option<Vec<String>>,
    managed: Option<Vec<String>>,
    system: Option<Vec<String>>,
) -> Option<AcpResolution> {
    for (argv, source) in [
        (environment, AcpSource::Env),
        (agents, AcpSource::AgentsConfig),
        (managed, AcpSource::Managed),
        (system, AcpSource::System),
    ] {
        if let Some(argv) = argv.filter(|argv| !argv.is_empty()) {
            return Some(AcpResolution { argv, source });
        }
    }
    None
}

/// Parse `ORSO_ACP_ARGV` (a JSON string array).
pub fn parse_acp_override(raw: Option<&str>) -> Option<Vec<String>> {
    raw.and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
        .filter(|argv| !argv.is_empty())
}

/// Ordered pieces of a child `PATH`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathParts {
    pub git_shim: Option<PathBuf>,
    pub managed_node_bin: Option<PathBuf>,
    pub provider_bins: Vec<PathBuf>,
    pub login_shell: OsString,
    pub daemon: OsString,
    pub extra: Vec<PathBuf>,
}

/// Build a `PATH` string, preserving the documented order and de-duplicating
/// directories.
pub fn build_path(parts: &PathParts) -> OsString {
    let mut seen = HashSet::new();
    let mut dirs = Vec::new();
    let mut push = |path: PathBuf| {
        if !path.as_os_str().is_empty() && seen.insert(path.clone()) {
            dirs.push(path);
        }
    };
    if let Some(dir) = &parts.git_shim {
        push(dir.clone());
    }
    if let Some(dir) = &parts.managed_node_bin {
        push(dir.clone());
    }
    for dir in &parts.provider_bins {
        push(dir.clone());
    }
    for dir in std::env::split_paths(&parts.login_shell) {
        push(dir);
    }
    for dir in std::env::split_paths(&parts.daemon) {
        push(dir);
    }
    for dir in &parts.extra {
        push(dir.clone());
    }
    std::env::join_paths(dirs).unwrap_or_else(|_| parts.daemon.clone())
}

/// Resolve provider ids against a runtime store.
#[derive(Clone)]
pub struct RuntimeResolver {
    store: PathBuf,
    lock: LockFile,
    target: RuntimeTarget,
    daemon_path: OsString,
    login_shell_path: OsString,
    git_shim: Option<PathBuf>,
    acp_env: Option<Vec<String>>,
    agents_acp: AgentAcpOverrides,
    providers: Vec<Provider>,
}

impl RuntimeResolver {
    /// Build a resolver without invoking a login shell.
    pub fn new(store: impl Into<PathBuf>, lock: LockFile, target: RuntimeTarget) -> Self {
        Self {
            store: store.into(),
            lock,
            target,
            daemon_path: std::env::var_os("PATH").unwrap_or_default(),
            login_shell_path: OsString::new(),
            git_shim: None,
            acp_env: parse_acp_override(std::env::var("ORSO_ACP_ARGV").ok().as_deref()),
            agents_acp: AgentAcpOverrides::new(),
            providers: crate::providers::default_providers(),
        }
    }

    /// Build a resolver that also reads the login shell's `PATH` so a
    /// Finder-launched app sees Homebrew installs.
    pub fn discover(store: impl Into<PathBuf>, lock: LockFile, target: RuntimeTarget) -> Self {
        let mut resolver = Self::new(store, lock, target);
        resolver.login_shell_path = login_shell_path();
        resolver
    }

    pub fn with_daemon_path(mut self, path: impl Into<OsString>) -> Self {
        self.daemon_path = path.into();
        self
    }

    pub fn with_login_shell_path(mut self, path: impl Into<OsString>) -> Self {
        self.login_shell_path = path.into();
        self
    }

    pub fn with_git_shim(mut self, dir: Option<PathBuf>) -> Self {
        self.git_shim = dir;
        self
    }

    pub fn with_acp_env(mut self, argv: Option<Vec<String>>) -> Self {
        self.acp_env = Some(argv.unwrap_or_default());
        self
    }

    pub fn with_agents_acp(mut self, overrides: AgentAcpOverrides) -> Self {
        self.agents_acp = overrides;
        self
    }

    pub fn with_providers(mut self, providers: Vec<Provider>) -> Self {
        self.providers = providers;
        self
    }

    pub fn store(&self) -> &Path {
        &self.store
    }

    pub fn lock(&self) -> &LockFile {
        &self.lock
    }

    pub fn target(&self) -> RuntimeTarget {
        self.target
    }

    /// Managed Node's `bin` directory, when Node is installed.
    pub fn managed_node_bin(&self) -> Option<PathBuf> {
        let install = self.managed_install(RuntimeId::Node)?;
        Some(install.root.join("bin"))
    }

    /// Newest managed install for a runtime, if it is usable.
    pub fn managed_install(&self, id: RuntimeId) -> Option<ManagedInstall> {
        let root = self.store.join(id.as_str());
        let mut candidates = Vec::new();
        for entry in std::fs::read_dir(&root).ok()?.flatten() {
            if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                continue;
            }
            let Ok(manifest) = read_manifest(&entry.path()) else {
                continue;
            };
            if manifest.runtime != id {
                continue;
            }
            if let Some(minimum) = id.minimum_version() {
                if !version_meets_minimum(&manifest.version, minimum) {
                    continue;
                }
            }
            let spec = RuntimeSpec::for_target(id, self.target, &self.lock).ok()?;
            if !is_executable(&spec.executable_path(&entry.path())) {
                continue;
            }
            candidates.push(ManagedInstall {
                id,
                version: manifest.version,
                root: entry.path(),
            });
        }
        candidates
            .into_iter()
            .max_by(|left, right| version_cmp(&left.version, &right.version))
    }

    /// Resolve a provider id (managed first, then the system CLI). Absolute
    /// paths are accepted as explicit system installations.
    pub fn resolve(&self, provider_id: &str) -> Result<ResolvedProvider> {
        if provider_id.contains('/') {
            let path = PathBuf::from(provider_id);
            if is_executable(&path) {
                return Ok(ResolvedProvider {
                    id: provider_id.to_string(),
                    executable: path,
                    version: None,
                    source: ResolveSource::System,
                    managed: false,
                });
            }
            return Err(RuntimeError::NotFound(format!(
                "{provider_id} is not executable"
            )));
        }

        if let Some(id) = RuntimeId::from_provider(provider_id) {
            if let Some(install) = self.managed_install(id) {
                let spec = RuntimeSpec::for_target(id, self.target, &self.lock)?;
                return Ok(ResolvedProvider {
                    id: provider_id.to_string(),
                    executable: spec.executable_path(&install.root),
                    version: Some(install.version),
                    source: ResolveSource::Managed,
                    managed: true,
                });
            }
        }

        let program = system_program(provider_id);
        let executable = self
            .find_on_path(program)
            .ok_or_else(|| RuntimeError::NotFound(format!("{provider_id} is not installed")))?;
        Ok(ResolvedProvider {
            id: provider_id.to_string(),
            executable,
            version: None,
            source: ResolveSource::System,
            managed: false,
        })
    }

    /// Resolve and run `--version`, enforcing the runtime's minimum version.
    pub async fn resolve_checked(&self, provider_id: &str) -> Result<ResolvedProvider> {
        let mut resolved = self.resolve(provider_id)?;
        let version = if resolved.managed {
            resolved.version.clone()
        } else {
            Some(run_cli_version(&resolved.executable).await?)
        };
        if let Some(version) = &version {
            if let Some(id) = RuntimeId::from_provider(provider_id) {
                if let Some(minimum) = id.minimum_version() {
                    if !version_meets_minimum(version, minimum) {
                        return Err(RuntimeError::VersionMismatch {
                            expected: format!("at least {minimum}"),
                            actual: version.clone(),
                        });
                    }
                }
            }
        }
        resolved.version = version;
        Ok(resolved)
    }

    /// ACP argv with the documented precedence.
    pub fn acp(&self, provider_id: &str) -> Option<AcpResolution> {
        let environment = self
            .acp_env
            .clone()
            .or_else(|| parse_acp_override(std::env::var("ORSO_ACP_ARGV").ok().as_deref()));
        let agents = self.agents_acp.get(provider_id).cloned();
        let managed = self.managed_acp_argv(provider_id);
        let system = self.system_acp_argv(provider_id);
        acp_precedence(environment, agents, managed, system)
    }

    /// `PATH` for a child process. `provider_id` adds that provider's managed
    /// bin first; every other managed runtime bin follows.
    pub fn child_path(&self, provider_id: Option<&str>) -> OsString {
        let mut provider_bins = Vec::new();
        if let Some(provider_id) = provider_id {
            if let Ok(resolved) = self.resolve(provider_id) {
                if resolved.managed {
                    provider_bins.push(resolved.bin_dir());
                }
            }
        }
        for id in self.lock.ids() {
            if let Some(install) = self.managed_install(id) {
                if let Ok(spec) = RuntimeSpec::for_target(id, self.target, &self.lock) {
                    provider_bins.push(spec.bin_dir(&install.root));
                }
            }
        }
        build_path(&PathParts {
            git_shim: self.git_shim.clone(),
            managed_node_bin: self.managed_node_bin(),
            provider_bins,
            login_shell: self.login_shell_path.clone(),
            daemon: self.daemon_path.clone(),
            extra: Vec::new(),
        })
    }

    fn find_on_path(&self, program: &str) -> Option<PathBuf> {
        let mut seen = HashSet::new();
        for dir in std::env::split_paths(&self.login_shell_path)
            .chain(std::env::split_paths(&self.daemon_path))
            .chain(common_dirs())
        {
            if !seen.insert(dir.clone()) {
                continue;
            }
            let candidate = dir.join(program);
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn managed_acp_argv(&self, provider_id: &str) -> Option<Vec<String>> {
        let id = RuntimeId::from_provider(provider_id)?;
        let install = self.managed_install(id)?;
        let spec = RuntimeSpec::for_target(id, self.target, &self.lock).ok()?;
        match id {
            RuntimeId::OpenCode | RuntimeId::Cursor => {
                let executable = spec.executable_path(&install.root);
                Some(vec![
                    executable.to_string_lossy().into_owned(),
                    "acp".into(),
                ])
            }
            RuntimeId::Claude | RuntimeId::Codex => self.managed_adapter_argv(id),
            RuntimeId::Node => None,
        }
    }

    /// Adapters live in `runtimes/acp/<provider>/` and run on managed Node.
    fn managed_adapter_argv(&self, id: RuntimeId) -> Option<Vec<String>> {
        super::adapter::adapter_argv(&self.store, id, &self.lock)
    }

    fn system_acp_argv(&self, provider_id: &str) -> Option<Vec<String>> {
        let id = RuntimeId::from_provider(provider_id);
        if let Some(id) = id {
            if let (RuntimeId::Claude | RuntimeId::Codex, Some(package)) =
                (id, self.lock.acp.get(id.as_str()))
            {
                return Some(vec![
                    "npx".into(),
                    "--yes".into(),
                    format!("{}@{}", package.package, package.version),
                ]);
            }
        }
        if let Ok(resolved) = self.resolve(provider_id) {
            if matches!(id, Some(RuntimeId::OpenCode) | Some(RuntimeId::Cursor)) {
                return Some(vec![
                    resolved.executable.to_string_lossy().into_owned(),
                    "acp".into(),
                ]);
            }
        }
        self.providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .map(|provider| provider.acp.clone())
            .filter(|argv| !argv.is_empty())
    }
}

fn system_program(provider_id: &str) -> &str {
    match provider_id {
        "cursor" => "cursor-agent",
        other => other,
    }
}

fn common_dirs() -> Vec<PathBuf> {
    let mut dirs = [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect::<Vec<_>>();
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/bin"));
        dirs.push(PathBuf::from(&home).join(".cargo/bin"));
    }
    dirs
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

/// Ask the login shell for its `PATH`. Returns an empty string on failure.
pub fn login_shell_path() -> OsString {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    std::process::Command::new(shell)
        .args(["-ilc", "printf %s \"$PATH\""])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| OsString::from(String::from_utf8_lossy(&output.stdout).trim()))
        .unwrap_or_default()
}

async fn run_cli_version(executable: &Path) -> Result<String> {
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        tokio::process::Command::new(executable)
            .arg("--version")
            .output(),
    )
    .await
    .map_err(|_| {
        RuntimeError::NotFound(format!("{} --version timed out", executable.display()))
    })??;
    if !output.status.success() {
        return Err(RuntimeError::NotFound(format!(
            "{} --version exited with {}",
            executable.display(),
            output.status
        )));
    }
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let token = text
        .split_whitespace()
        .map(|token| {
            token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-')
        })
        .map(|token| token.strip_prefix('v').unwrap_or(token))
        .find(|token| {
            token
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_digit())
                && token.contains('.')
        })
        .ok_or_else(|| {
            RuntimeError::NotFound(format!("{} did not report a version", executable.display()))
        })?;
    Ok(token.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::descriptors::{write_manifest, InstallFile, RuntimeArtifact, RuntimeLock};
    use std::collections::BTreeMap;

    fn lock() -> LockFile {
        let mut artifacts = BTreeMap::new();
        artifacts.insert(
            "darwin-arm64".into(),
            RuntimeArtifact {
                package: Some("@opencode/cli-darwin-arm64".into()),
                asset: Some("opencode.tar.gz".into()),
                url: Some("https://example.test/opencode.tar.gz".into()),
                asset_sha256: "00".into(),
                binary_sha256: None,
                download_bytes: None,
                installed_bytes: None,
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
            RuntimeLock {
                version: "2.2.5".into(),
                sdk_version: None,
                tag: None,
                repository: None,
                registry: None,
                distribution: None,
                artifacts,
            },
        );
        // Claude is present so minimum-version fallback can be exercised.
        let mut claude_artifacts = BTreeMap::new();
        claude_artifacts.insert(
            "darwin-arm64".into(),
            RuntimeArtifact {
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
            },
        );
        runtimes.insert(
            "claude".into(),
            RuntimeLock {
                version: "2.1.318".into(),
                sdk_version: Some("0.3.318".into()),
                tag: None,
                repository: None,
                registry: None,
                distribution: None,
                artifacts: claude_artifacts,
            },
        );
        LockFile {
            schema_version: 1,
            runtimes,
            acp: BTreeMap::from([(
                "claude".into(),
                super::super::descriptors::AcpPackage {
                    package: "@agentclientprotocol/claude-agent-acp".into(),
                    version: "0.100.0".into(),
                },
            )]),
        }
    }

    fn install(store: &Path, id: &str, version: &str, executable: &str) {
        let root = store.join(id).join(version);
        let path = root.join(executable);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "#!/bin/sh\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let manifest = super::super::descriptors::InstallManifest {
            schema_version: 1,
            runtime: id.parse().unwrap(),
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
    }

    fn system_bin(dir: &Path, name: &str) {
        let path = dir.join(name);
        std::fs::write(&path, "#!/bin/sh\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn managed_is_preferred_over_system() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        let system = dir.path().join("system");
        std::fs::create_dir_all(&system).unwrap();
        install(&store, "opencode", "2.2.5", "bin/opencode");
        system_bin(&system, "opencode");
        let resolver = RuntimeResolver::new(&store, lock(), RuntimeTarget::DarwinArm64)
            .with_login_shell_path(system.as_os_str().to_os_string());
        let resolved = resolver.resolve("opencode").unwrap();
        assert_eq!(resolved.source, ResolveSource::Managed);
        assert_eq!(resolved.version.as_deref(), Some("2.2.5"));
        assert!(resolved.managed);
        assert_eq!(
            resolved.executable,
            store.join("opencode/2.2.5/bin/opencode")
        );
    }

    #[test]
    fn system_fallback_uses_login_path_then_daemon_path() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        let system = dir.path().join("system");
        std::fs::create_dir_all(&system).unwrap();
        system_bin(&system, "opencode");
        let resolver = RuntimeResolver::new(&store, lock(), RuntimeTarget::DarwinArm64)
            .with_login_shell_path(system.as_os_str().to_os_string())
            .with_daemon_path("/definitely/not/here");
        let resolved = resolver.resolve("opencode").unwrap();
        assert_eq!(resolved.source, ResolveSource::System);
        assert_eq!(resolved.executable, system.join("opencode"));
    }

    #[test]
    fn absolute_paths_and_unknown_ids_resolve() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        let system = dir.path().join("system");
        std::fs::create_dir_all(&system).unwrap();
        system_bin(&system, "my-agent");
        let resolver = RuntimeResolver::new(&store, lock(), RuntimeTarget::DarwinArm64)
            .with_login_shell_path(system.as_os_str().to_os_string());
        let resolved = resolver.resolve("my-agent").unwrap();
        assert_eq!(resolved.source, ResolveSource::System);
        let absolute = resolver
            .resolve(system.join("my-agent").to_str().unwrap())
            .unwrap();
        assert_eq!(absolute.source, ResolveSource::System);
        assert!(resolver.resolve("./missing-agent").is_err());
    }

    #[test]
    fn managed_below_minimum_falls_back_to_system() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        let system = dir.path().join("system");
        std::fs::create_dir_all(&system).unwrap();
        install(&store, "claude", "2.1.100", "bin/claude");
        system_bin(&system, "claude");
        let resolver = RuntimeResolver::new(&store, lock(), RuntimeTarget::DarwinArm64)
            .with_login_shell_path(system.as_os_str().to_os_string());
        let resolved = resolver.resolve("claude").unwrap();
        assert_eq!(resolved.source, ResolveSource::System);
    }

    #[test]
    fn acp_precedence_is_env_agents_managed_system() {
        let env = vec!["env".to_string()];
        let agents = vec!["agents".to_string()];
        let managed = vec!["managed".to_string()];
        let system = vec!["system".to_string()];
        assert_eq!(
            acp_precedence(
                Some(env.clone()),
                Some(agents.clone()),
                Some(managed.clone()),
                Some(system.clone())
            )
            .unwrap()
            .source,
            AcpSource::Env
        );
        assert_eq!(
            acp_precedence(None, Some(agents), Some(managed), Some(system))
                .unwrap()
                .source,
            AcpSource::AgentsConfig
        );
        assert_eq!(
            acp_precedence(
                None,
                None,
                Some(vec!["managed".into()]),
                Some(vec!["system".into()])
            )
            .unwrap()
            .source,
            AcpSource::Managed
        );
        assert_eq!(
            acp_precedence(None, None, None, Some(vec!["system".into()]))
                .unwrap()
                .source,
            AcpSource::System
        );
        assert!(acp_precedence(None, None, None, None).is_none());
        assert!(acp_precedence(Some(vec![]), None, None, None).is_none());
    }

    #[test]
    fn managed_opencode_acp_uses_the_absolute_executable() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        install(&store, "opencode", "2.2.5", "bin/opencode");
        let resolver =
            RuntimeResolver::new(&store, lock(), RuntimeTarget::DarwinArm64).with_acp_env(None);
        let acp = resolver.acp("opencode").unwrap();
        assert_eq!(acp.source, AcpSource::Managed);
        assert_eq!(
            acp.argv,
            [
                store
                    .join("opencode/2.2.5/bin/opencode")
                    .to_string_lossy()
                    .into_owned(),
                "acp".into()
            ]
        );
    }

    #[test]
    fn explicit_overrides_beat_managed() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        install(&store, "opencode", "2.2.5", "bin/opencode");
        let mut overrides = AgentAcpOverrides::new();
        overrides.insert("opencode", vec!["agents".into(), "acp".into()]);
        let resolver = RuntimeResolver::new(&store, lock(), RuntimeTarget::DarwinArm64)
            .with_acp_env(Some(vec!["env".into()]))
            .with_agents_acp(overrides);
        let acp = resolver.acp("opencode").unwrap();
        assert_eq!(acp.source, AcpSource::Env);
        assert_eq!(acp.argv, ["env"]);

        let resolver = RuntimeResolver::new(&store, lock(), RuntimeTarget::DarwinArm64)
            .with_acp_env(None)
            .with_agents_acp({
                let mut overrides = AgentAcpOverrides::new();
                overrides.insert("opencode", vec!["agents".into(), "acp".into()]);
                overrides
            });
        // Make sure an ambient ORSO_ACP_ARGV does not leak into this test.
        assert_eq!(
            resolver.acp("opencode").unwrap().source,
            AcpSource::AgentsConfig
        );
    }

    #[test]
    fn managed_adapter_argv_is_preferred_but_overrides_win() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        install(&store, "node", "26.13.0", "bin/node");
        install(&store, "claude", "2.1.318", "bin/claude");
        let prefix = store.join("acp/claude");
        std::fs::create_dir_all(
            prefix.join("node_modules/@agentclientprotocol/claude-agent-acp/dist"),
        )
        .unwrap();
        std::fs::write(
            prefix.join("orso-adapter.json"),
            serde_json::json!({
                "schemaVersion": 1,
                "provider": "claude",
                "package": "@agentclientprotocol/claude-agent-acp",
                "version": "0.100.0",
                "bin": "node_modules/@agentclientprotocol/claude-agent-acp/dist/index.js",
                "installedAt": 0
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            prefix.join("node_modules/@agentclientprotocol/claude-agent-acp/dist/index.js"),
            "#!/usr/bin/env node\n",
        )
        .unwrap();

        let resolver =
            RuntimeResolver::new(&store, lock(), RuntimeTarget::DarwinArm64).with_acp_env(None);
        let acp = resolver.acp("claude").unwrap();
        assert_eq!(acp.source, AcpSource::Managed);
        assert_eq!(
            acp.argv,
            [
                store
                    .join("node/26.13.0/bin/node")
                    .to_string_lossy()
                    .into_owned(),
                prefix
                    .join("node_modules/@agentclientprotocol/claude-agent-acp/dist/index.js")
                    .to_string_lossy()
                    .into_owned(),
            ]
        );

        let resolver = resolver.with_agents_acp({
            let mut overrides = AgentAcpOverrides::new();
            overrides.insert("claude", vec!["agents".into(), "acp".into()]);
            overrides
        });
        assert_eq!(
            resolver.acp("claude").unwrap().source,
            AcpSource::AgentsConfig
        );

        let resolver = resolver.with_acp_env(Some(vec!["env".into()]));
        assert_eq!(resolver.acp("claude").unwrap().source, AcpSource::Env);
    }

    #[test]
    fn path_order_is_git_node_providers_login_daemon() {
        let parts = PathParts {
            git_shim: Some(PathBuf::from("/shim")),
            managed_node_bin: Some(PathBuf::from("/node/bin")),
            provider_bins: vec![PathBuf::from("/managed/provider/bin")],
            login_shell: OsString::from("/login/bin:/node/bin"),
            daemon: OsString::from("/daemon/bin:/login/bin"),
            extra: vec![PathBuf::from("/extra")],
        };
        let path = build_path(&parts);
        let dirs = std::env::split_paths(&path)
            .map(|dir| dir.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            dirs,
            [
                "/shim",
                "/node/bin",
                "/managed/provider/bin",
                "/login/bin",
                "/daemon/bin",
                "/extra"
            ]
        );
    }

    #[test]
    fn child_path_includes_managed_bins() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store");
        install(&store, "opencode", "2.2.5", "bin/opencode");
        let resolver = RuntimeResolver::new(&store, lock(), RuntimeTarget::DarwinArm64)
            .with_login_shell_path(OsString::from("/login"))
            .with_daemon_path(OsString::from("/daemon"))
            .with_git_shim(Some(PathBuf::from("/shim")));
        let path = resolver.child_path(Some("opencode"));
        let dirs = std::env::split_paths(&path)
            .map(|dir| dir.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(dirs[0], "/shim");
        assert_eq!(dirs[1], store.join("opencode/2.2.5/bin").to_string_lossy());
        assert!(dirs.contains(&"/login".to_string()));
        assert!(dirs.contains(&"/daemon".to_string()));
    }

    #[test]
    fn parse_acp_override_ignores_bad_json() {
        assert_eq!(
            parse_acp_override(Some(r#"["a","b"]"#)),
            Some(vec!["a".into(), "b".into()])
        );
        assert_eq!(parse_acp_override(Some("nope")), None);
        assert_eq!(parse_acp_override(Some("[]")), None);
        assert_eq!(parse_acp_override(None), None);
    }
}

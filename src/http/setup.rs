//! Assistant setup: `GET /api/setup/status`, runtime downloads, provider
//! verification and sign-in.
//!
//! The wire shapes match `web/src/core/types.ts`: runtime phases are camelCase
//! (`notDownloaded`, `downloadError`, …) and every provider carries the flat
//! `runtime`, `installed`, `signedIn`, `chatReady`, `freeModels` and `reason`
//! fields the setup screen reads.

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::body::Bytes;
use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::providers;
use crate::runtime::login::{SignInReply, SignInState};
use crate::runtime::manager::RuntimePhaseKind;
use crate::runtime::{RuntimeId, RuntimePhase, RuntimeSnapshot};
use crate::state::AppState;

/// How long a probe (auth, version) stays fresh.
const PROBE_TTL: Duration = Duration::from_secs(30);
/// End-to-end budget for `initialize` + `session/new`.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(25);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatus {
    git: bool,
    gh: bool,
    github_signed_in: bool,
    node: RuntimeStatus,
    providers: Vec<ProviderStatus>,
    projects_dir: PathBuf,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupSnapshot {
    node: RuntimeStatus,
    providers: Vec<ProviderStatus>,
}

/// One runtime's wire state.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    phase: String,
    version: Option<String>,
    source: Option<String>,
    progress: Option<u8>,
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    download_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    installed_bytes: Option<u64>,
}

impl RuntimeStatus {
    fn unsupported(message: impl Into<String>) -> Self {
        Self {
            phase: "notDownloaded".into(),
            version: None,
            source: None,
            progress: None,
            message: Some(message.into()),
            download_bytes: None,
            installed_bytes: None,
        }
    }

    fn ready_system() -> Self {
        Self {
            phase: "ready".into(),
            version: None,
            source: Some("system".into()),
            progress: None,
            message: None,
            download_bytes: None,
            installed_bytes: None,
        }
    }

    fn from_snapshot(snapshot: &RuntimeSnapshot) -> Self {
        let mut status = Self::from_phase(&snapshot.phase);
        status.download_bytes = snapshot.download_bytes;
        status.installed_bytes = snapshot.installed_bytes;
        status
    }

    fn from_phase(phase: &RuntimePhase) -> Self {
        Self {
            phase: match phase.kind {
                RuntimePhaseKind::NotDownloaded => "notDownloaded",
                RuntimePhaseKind::Downloading => "downloading",
                RuntimePhaseKind::Finishing => "finishing",
                RuntimePhaseKind::Ready => "ready",
                RuntimePhaseKind::DownloadError => "downloadError",
                RuntimePhaseKind::UpdateAvailable => "updateAvailable",
            }
            .into(),
            version: phase.version.clone(),
            source: phase.source.map(|source| {
                match source {
                    crate::runtime::RuntimeSource::Managed => "managed",
                    crate::runtime::RuntimeSource::System => "system",
                }
                .to_string()
            }),
            progress: phase.progress,
            message: phase.message.clone(),
            download_bytes: None,
            installed_bytes: None,
        }
    }

    fn is_ready(&self) -> bool {
        self.phase == "ready"
    }
}

/// One assistant as `/api/setup/status` and `/api/setup/runtimes` report it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    id: String,
    name: String,
    /// A runnable copy exists (older daemons only sent this).
    available: bool,
    runtime: RuntimeStatus,
    installed: bool,
    /// `true`, `false`, or `null` when the CLI could not tell us.
    signed_in: Option<bool>,
    chat_ready: bool,
    free_models: bool,
    reason: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResult {
    chat_ready: bool,
    reason: Option<String>,
}

#[derive(Deserialize)]
struct CodeBody {
    code: String,
}

/// `GET /api/setup/status` → prerequisites plus the assistant snapshot.
pub async fn status(State(state): State<AppState>) -> Result<Json<SetupStatus>, ApiError> {
    let search = search_path(&state).await;
    let found = |program: &str| on_path(program, &search);
    let providers = provider_statuses(&state, &found).await?;
    let github = super::github::status(State(state.clone())).await.0;
    Ok(Json(SetupStatus {
        git: found("git") && !is_macos_git_stub(&search),
        gh: found("gh"),
        github_signed_in: github.user.is_some(),
        node: node_status(&state),
        providers,
        projects_dir: state.config.projects_dir.clone(),
    }))
}

/// `GET /api/setup/runtimes` → `{ node, providers }`.
pub async fn runtimes(State(state): State<AppState>) -> Result<Json<SetupSnapshot>, ApiError> {
    let search = search_path(&state).await;
    let found = |program: &str| on_path(program, &search);
    Ok(Json(SetupSnapshot {
        node: node_status(&state),
        providers: provider_statuses(&state, &found).await?,
    }))
}

/// `POST /api/setup/runtimes/{id}/download` → the runtime's status (200 even on a
/// download error; the status carries the message).
pub async fn download(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<(StatusCode, Json<RuntimeStatus>), ApiError> {
    let runtime = runtime_id(&id)?;
    let result = action(state.clone(), &id, |state, id| async move {
        state.runtimes.download(id).await
    })
    .await?;
    let (code, body) = result;
    // Claude and Codex need the pinned ACP adapter as well as the CLI.
    if matches!(runtime, RuntimeId::Claude | RuntimeId::Codex) && body.phase == "ready" {
        spawn_adapter_setup(&state, runtime);
    }
    Ok((code, body))
}

/// `POST /api/setup/runtimes/{id}/cancel`.
pub async fn cancel(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<(StatusCode, Json<RuntimeStatus>), ApiError> {
    let id = runtime_id(&id)?;
    let _ = state.runtimes.cancel(id);
    if matches!(id, RuntimeId::Claude | RuntimeId::Codex) {
        state.runtimes.cancel_adapter(id);
    }
    state.setup_probes.clear(id.as_str());
    refresh_site_paths(&state);
    Ok((StatusCode::OK, Json(snapshot_status(&state, id))))
}

/// `POST /api/setup/runtimes/{id}/retry`.
pub async fn retry(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<(StatusCode, Json<RuntimeStatus>), ApiError> {
    action(state, &id, |state, id| async move {
        state.runtimes.retry(id).await
    })
    .await
}

async fn action<F, Fut>(
    state: AppState,
    id: &str,
    run: F,
) -> Result<(StatusCode, Json<RuntimeStatus>), ApiError>
where
    F: FnOnce(AppState, RuntimeId) -> Fut,
    Fut: std::future::Future<Output = crate::runtime::Result<RuntimeSnapshot>>,
{
    let id = runtime_id(id)?;
    if state.runtime_target.is_none() {
        return Ok((
            StatusCode::OK,
            Json(RuntimeStatus::unsupported(
                "This Mac is not supported by the managed downloads.",
            )),
        ));
    }
    match run(state.clone(), id).await {
        Ok(snapshot) => {
            state.setup_probes.clear(id.as_str());
            Ok((
                StatusCode::OK,
                Json(RuntimeStatus::from_snapshot(&snapshot)),
            ))
        }
        Err(crate::runtime::RuntimeError::Busy(_)) => Err(ApiError::conflict(
            "that download is already running; wait for it or cancel it",
        )),
        Err(_) => {
            state.setup_probes.clear(id.as_str());
            refresh_site_paths(&state);
            // The manager keeps the failure in its status; return it so the row
            // can show Retry without treating the request itself as broken.
            Ok((StatusCode::OK, Json(snapshot_status(&state, id))))
        }
    }
}

/// A managed Node install changes the `PATH` used by `npm install` and the dev
/// server; refresh both after any download or cancel.
fn refresh_site_paths(state: &AppState) {
    let path = state.resolver.child_path(None);
    state.projects.set_install_path(path.clone());
    state.devservers.set_path(path);
}

fn runtime_id(id: &str) -> Result<RuntimeId, ApiError> {
    id.parse::<RuntimeId>()
        .map_err(|_| ApiError::enoent(format!("no such runtime: {id}")))
}

fn snapshot_status(state: &AppState, id: RuntimeId) -> RuntimeStatus {
    match state.runtimes.snapshot(id) {
        Ok(snapshot) => RuntimeStatus::from_snapshot(&snapshot),
        Err(error) => RuntimeStatus::unsupported(error.to_string()),
    }
}

fn node_status(state: &AppState) -> RuntimeStatus {
    if state.runtime_target.is_none() {
        return RuntimeStatus::unsupported("This Mac is not supported by the managed downloads.");
    }
    snapshot_status(state, RuntimeId::Node)
}

async fn provider_statuses(
    state: &AppState,
    found: &impl Fn(&str) -> bool,
) -> Result<Vec<ProviderStatus>, ApiError> {
    let providers = providers::load(state.config.agents_config.as_deref())?;
    let mut statuses = Vec::with_capacity(providers.len());
    for provider in providers {
        statuses.push(provider_status(state, &provider, found).await);
    }
    Ok(statuses)
}

async fn provider_status(
    state: &AppState,
    provider: &providers::Provider,
    found: &impl Fn(&str) -> bool,
) -> ProviderStatus {
    let id = provider.id.as_str();
    let program = provider.command.first().map(String::as_str).unwrap_or(id);
    let managed = state
        .resolver
        .resolve(id)
        .map(|resolved| resolved.managed)
        .unwrap_or(false);
    let available = found(program) || managed;
    let runtime = provider_runtime(state, id, available);
    let acp = super::chat::acp_argv_for(state, id).ok().flatten();
    let free_models = id == "opencode";
    let native_acp = matches!(id, "opencode" | "cursor");
    let adapter_provider = match id {
        "claude" => Some(RuntimeId::Claude),
        "codex" => Some(RuntimeId::Codex),
        _ => None,
    };
    let explicit_override = super::chat::has_explicit_acp(state, id);
    let adapter_state = adapter_provider.map(|runtime| state.runtimes.adapter_state(runtime));
    let adapter_ready = adapter_provider.is_none()
        || explicit_override
        || adapter_state
            .as_ref()
            .is_some_and(crate::runtime::AdapterState::is_ready);

    // Once the managed provider and Node are ready, install the pinned adapter
    // in the background. The next poll reports chatReady.
    if let Some(runtime_id) = adapter_provider {
        if !explicit_override
            && (runtime.is_ready() || available)
            && matches!(adapter_state, Some(crate::runtime::AdapterState::Missing))
            && !state.runtimes.adapter_needs_node(runtime_id)
        {
            spawn_adapter_setup(state, runtime_id);
        }
    }

    let runnable = acp.is_some()
        && (explicit_override || ((available || managed) && runtime.is_ready()))
        && (native_acp || adapter_ready);

    let signed_in = if available || managed {
        probe_signed_in(state, id).await
    } else {
        None
    };
    let chat_ready = runnable && (free_models || signed_in != Some(false));
    let installed = available || managed || runtime.is_ready();
    let reason = if chat_ready {
        None
    } else if state.runtime_target.is_none() {
        Some("Assistants aren't available on this Mac.".into())
    } else if !runtime.is_ready() && !available {
        Some(format!("Download {} to start chatting.", display(id)))
    } else if !native_acp && !adapter_ready {
        super::chat::acp_unavailable_reason(state, id).or_else(|| {
            Some(format!(
                "Chat with {} isn't ready yet. Download it again from setup.",
                display(id)
            ))
        })
    } else if acp.is_none() {
        super::chat::acp_unavailable_reason(state, id)
            .or_else(|| Some(format!("Chat isn't set up for {} yet.", display(id))))
    } else if !free_models && signed_in == Some(false) {
        Some(format!("Sign in to use your {} plan.", display(id)))
    } else {
        Some(format!("{} isn't ready yet.", display(id)))
    };

    ProviderStatus {
        id: id.to_string(),
        name: providers::display_name(id).to_string(),
        available,
        runtime,
        installed,
        signed_in,
        chat_ready,
        free_models,
        reason,
    }
}

/// Install an adapter without blocking the status request.
fn spawn_adapter_setup(state: &AppState, runtime: RuntimeId) {
    let runtimes = state.runtimes.clone();
    tokio::spawn(async move {
        let _ = runtimes.ensure_adapter(runtime).await;
    });
}

fn provider_runtime(state: &AppState, provider_id: &str, system_available: bool) -> RuntimeStatus {
    let Some(runtime) = RuntimeId::from_provider(provider_id) else {
        return RuntimeStatus::unsupported("this provider is configured outside the daemon");
    };
    if state.runtime_target.is_none() {
        return RuntimeStatus::unsupported("This Mac is not supported by the managed downloads.");
    }
    let status = snapshot_status(state, runtime);
    if !status.is_ready() && system_available && status.phase == "notDownloaded" {
        return RuntimeStatus::ready_system();
    }
    status
}

async fn probe_signed_in(state: &AppState, provider_id: &str) -> Option<bool> {
    if let Some(value) = state.setup_probes.get(provider_id) {
        return value;
    }
    let value = state.logins.signed_in(provider_id).await;
    state.setup_probes.put(provider_id, value);
    value
}

/// `POST /api/setup/providers/{id}/verify` → start the resolved ACP command and
/// complete `initialize` + `session/new` in a temp directory.
pub async fn verify(
    State(state): State<AppState>,
    AxumPath(provider): AxumPath<String>,
) -> Result<Json<VerifyResult>, ApiError> {
    // The adapter is part of "installed": make sure it is in place before
    // starting the ACP handshake.
    if let Some(runtime) = RuntimeId::from_provider(&provider) {
        if matches!(runtime, RuntimeId::Claude | RuntimeId::Codex)
            && !super::chat::has_explicit_acp(&state, &provider)
        {
            if let Err(error) = state.runtimes.ensure_adapter(runtime).await {
                return Ok(Json(VerifyResult {
                    chat_ready: false,
                    reason: Some(error.to_string()),
                }));
            }
            state.setup_probes.clear(&provider);
        }
    }
    let Some(argv) = super::chat::acp_argv_for(&state, &provider)? else {
        return Ok(Json(VerifyResult {
            chat_ready: false,
            reason: super::chat::acp_unavailable_reason(&state, &provider)
                .or_else(|| Some(format!("Chat isn't set up for {} yet.", display(&provider)))),
        }));
    };
    let Some((command, args)) = argv.split_first() else {
        return Ok(Json(VerifyResult {
            chat_ready: false,
            reason: Some("Chat isn't set up yet.".into()),
        }));
    };
    let dir = std::env::temp_dir().join(format!("orso-verify-{}", crate::config::random_token()));
    tokio::fs::create_dir_all(&dir).await?;
    let env = super::chat::agent_env(&state, "");
    let env: Vec<(&str, &str)> = env
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let result = tokio::time::timeout(VERIFY_TIMEOUT, async {
        let client = crate::chat::acp::AcpClient::spawn_with_env(command, args, &dir, &env).await?;
        let initialized = client.initialize().await?;
        let _ = client.session_new(&dir).await?;
        client.shutdown().await?;
        Ok::<_, crate::error::BoxError>(initialized)
    })
    .await;
    let _ = tokio::fs::remove_dir_all(&dir).await;
    match result {
        Ok(Ok(_)) => {
            state.setup_probes.remember_verified(&provider);
            Ok(Json(VerifyResult {
                chat_ready: true,
                reason: None,
            }))
        }
        Ok(Err(error)) => Ok(Json(VerifyResult {
            chat_ready: false,
            reason: Some(format!(
                "{} did not finish checking in: {error}",
                display(&provider)
            )),
        })),
        Err(_) => Ok(Json(VerifyResult {
            chat_ready: false,
            reason: Some(format!(
                "{} took too long to check in. Try again.",
                display(&provider)
            )),
        })),
    }
}

/// `POST /api/setup/providers/{id}/sign-in` → start or poll the provider's login.
pub async fn sign_in(
    State(state): State<AppState>,
    AxumPath(provider): AxumPath<String>,
) -> Json<SignInReply> {
    Json(state.logins.start(&provider).await)
}

/// `GET /api/setup/providers/{id}/sign-in` → current flow state.
pub async fn sign_in_current(
    State(state): State<AppState>,
    AxumPath(provider): AxumPath<String>,
) -> Json<SignInReply> {
    Json(state.logins.current(&provider).unwrap_or(SignInReply {
        state: SignInState::Cancelled,
        url: None,
        needs_code: false,
        message: Some("No sign-in is in progress.".into()),
    }))
}

/// `POST /api/setup/providers/{id}/sign-in/code` `{ code }`.
pub async fn sign_in_code(
    State(state): State<AppState>,
    AxumPath(provider): AxumPath<String>,
    body: Bytes,
) -> Result<Json<SignInReply>, ApiError> {
    let CodeBody { code } = serde_json::from_slice(&body)
        .map_err(|e| ApiError::badreq(format!("expected JSON body {{ code }}: {e}")))?;
    let reply = state.logins.submit_code(&provider, &code).await;
    state.setup_probes.clear(&provider);
    Ok(Json(reply))
}

/// `POST /api/setup/providers/{id}/cancel`.
pub async fn cancel_sign_in(
    State(state): State<AppState>,
    AxumPath(provider): AxumPath<String>,
) -> Json<SignInReply> {
    let reply = state.logins.cancel(&provider);
    state.setup_probes.clear(&provider);
    Json(reply)
}

/// `POST /api/setup/providers/{id}/sign-out`.
pub async fn sign_out(
    State(state): State<AppState>,
    AxumPath(provider): AxumPath<String>,
) -> Json<SignInReply> {
    let reply = state.logins.sign_out(&provider).await;
    state.setup_probes.clear(&provider);
    Json(reply)
}

fn display(provider_id: &str) -> &str {
    match provider_id {
        "opencode" => "OpenCode",
        "claude" => "Claude",
        "codex" => "Codex",
        "cursor" => "Cursor",
        other => other,
    }
}

async fn search_path(state: &AppState) -> OsString {
    match state.config.search_path.clone() {
        Some(path) => path,
        None => default_search_path().await,
    }
}

/// The login shell's `PATH` (where agents are found; an app opened from the Finder only
/// has launchd's minimal one), then the daemon's own.
async fn default_search_path() -> OsString {
    let login = crate::shell::login_path().await.unwrap_or_default();
    let own = std::env::var_os("PATH").unwrap_or_default();
    let dirs: Vec<PathBuf> = std::env::split_paths(&login)
        .chain(std::env::split_paths(&own))
        .collect();
    std::env::join_paths(dirs).unwrap_or(own)
}

/// macOS ships `/usr/bin/git` even without git: it only offers to install the Command Line
/// Tools. It is real once those tools or Xcode are installed.
fn is_macos_git_stub(search: &OsStr) -> bool {
    let first = std::env::split_paths(search)
        .map(|dir| dir.join("git"))
        .find(|p| is_executable(p));
    cfg!(target_os = "macos")
        && first.is_some_and(|p| p == Path::new("/usr/bin/git"))
        && ![
            "/Library/Developer/CommandLineTools/usr/bin/git",
            "/Applications/Xcode.app/Contents/Developer/usr/bin/git",
        ]
        .iter()
        .any(|p| is_executable(Path::new(p)))
}

/// Whether `program` names an executable file: as given when it contains a `/`, else in one of
/// the `PATH`-style `search` directories.
fn on_path(program: &str, search: &OsStr) -> bool {
    let program = Path::new(program);
    if program.components().count() > 1 {
        return is_executable(program);
    }
    std::env::split_paths(search).any(|dir| is_executable(&dir.join(program)))
}

fn is_executable(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Short-lived cache for provider auth probes, so `/api/setup/status` polling is cheap.
#[derive(Default)]
pub struct SetupProbes {
    entries: Mutex<HashMap<String, CachedProbe>>,
}

#[derive(Clone, Copy)]
struct CachedProbe {
    at: Instant,
    signed_in: Option<bool>,
}

impl SetupProbes {
    pub fn get(&self, provider_id: &str) -> Option<Option<bool>> {
        let entries = self.lock();
        let entry = entries.get(provider_id)?;
        (entry.at.elapsed() < PROBE_TTL).then_some(entry.signed_in)
    }

    pub fn put(&self, provider_id: &str, signed_in: Option<bool>) {
        self.lock().insert(
            provider_id.to_string(),
            CachedProbe {
                at: Instant::now(),
                signed_in,
            },
        );
    }

    /// A successful ACP handshake is the strongest signal we have; remember it.
    pub fn remember_verified(&self, provider_id: &str) {
        if provider_id != "opencode" {
            self.put(provider_id, Some(true));
        }
    }

    pub fn clear(&self, provider_id: &str) {
        self.lock().remove(provider_id);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, CachedProbe>> {
        self.entries
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}

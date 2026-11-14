//! Shared application state.

use std::collections::HashSet;
use std::ffi::OsString;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

use serde_json::json;

use crate::changes::conflict_sessions::ConflictSessions;
use crate::chat::manager::ChatManager;
use crate::config::Config;
use crate::conflict_preview::ConflictPreviewManager;
use crate::content::integration::ContentEndpoint;
use crate::devserver::{DevServerManager, DevServerOptions};
use crate::error::ApiError;
use crate::events::EventBus;
use crate::github::GithubAuth;
use crate::http::setup::SetupProbes;
use crate::preview::{self, UpstreamClient};
use crate::projects::{DevserverStatus, ProjectRegistry};
use crate::runtime::login::LoginManager;
use crate::runtime::{LockFile, RuntimeManager, RuntimeResolver, RuntimeTarget};
use crate::watch::WatchManager;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub projects: Arc<ProjectRegistry>,
    /// Broadcaster behind `WS /ws/events`.
    pub events: EventBus,
    /// ACP chat agents, one process per `(project, chat)`.
    pub chats: Arc<ChatManager>,
    /// Supervised `npm run dev` per project.
    pub devservers: Arc<DevServerManager>,
    /// `fs.change` watcher per project.
    pub watchers: Arc<WatchManager>,
    /// GitHub session (device-flow sign-in) and API access.
    pub github: Arc<GithubAuth>,
    /// HTTP client for the `/preview/{id}/*` reverse proxy.
    pub preview: UpstreamClient,
    /// Proposed conflict resolutions; never mutates a branch until Finish.
    pub conflicts: Arc<ConflictSessions>,
    pub conflict_previews: Arc<ConflictPreviewManager>,
    /// Serializes Get latest through session creation as one operation.
    pub conflict_updates: Arc<Mutex<()>>,
    /// Projects whose Get latest skipped the checked-out change while an agent was live.
    pub pending_latest: Arc<std::sync::Mutex<HashSet<String>>>,
    /// Managed Node / assistant downloads and the site builder's Node.
    pub runtimes: Arc<RuntimeManager>,
    /// The platform target this build can install, when supported.
    pub runtime_target: Option<RuntimeTarget>,
    /// Managed-first provider and `PATH` resolution for chat and site builds.
    pub resolver: Arc<RuntimeResolver>,
    /// Provider sign-in flows.
    pub logins: Arc<LoginManager>,
    /// Short cache for provider auth/version probes.
    pub setup_probes: Arc<SetupProbes>,
}

fn git_shim_dir(config: &Config) -> Option<std::path::PathBuf> {
    crate::git_shim::ensure_shim_dir(config)
}

fn cached_login_path() -> OsString {
    static LOGIN_PATH: OnceLock<OsString> = OnceLock::new();
    LOGIN_PATH
        .get_or_init(crate::runtime::resolve::login_shell_path)
        .clone()
}

fn runtime_lock(config: &Config) -> LockFile {
    if let Some(path) = &config.runtime_lock {
        match LockFile::load(path) {
            Ok(lock) => return lock,
            Err(error) => eprintln!("orso: {error}; using the built-in runtime list"),
        }
    }
    match LockFile::parse(include_str!("../agent-runtime.lock.json")) {
        Ok(lock) => lock,
        Err(error) => {
            eprintln!("orso: {error}; managed assistants are unavailable");
            LockFile {
                schema_version: 1,
                runtimes: Default::default(),
                acp: Default::default(),
            }
        }
    }
}

/// Forward `setup.runtime` events onto `WS /ws/events` as the flat shape the web app reads.
fn bridge_runtime_events(runtimes: &RuntimeManager, events: &EventBus) {
    let mut rx = runtimes.subscribe();
    let events = events.clone();
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Ok(value) = serde_json::to_value(&event) {
                        events.emit(crate::runtime::manager::RUNTIME_EVENT, value);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

/// How long a content save waits for a `Starting` preview to become ready.
const CONTENT_ENDPOINT_WAIT: std::time::Duration = std::time::Duration::from_secs(60);

impl AppState {
    /// Production state: project creation runs `npm install` and supervises `npm run dev`.
    pub fn new(config: Config) -> Self {
        Self::with_scaffold_install(config, true)
    }

    /// Like [`AppState::new`], but `install: false` skips `npm install` during project
    /// creation *and* does not spawn dev servers (there is nothing to run without
    /// `node_modules`). This is an internal hook so tests do not need the network; use
    /// [`AppState::with_options`] to supervise a stand-in command instead.
    pub fn with_scaffold_install(config: Config, install: bool) -> Self {
        let proxy_port = config.port;
        let devserver = if install {
            DevServerOptions::configured(proxy_port)
        } else {
            DevServerOptions::disabled()
        };
        Self::with_options(config, install, devserver)
    }

    /// Watch one project's directory (failure is logged, never fatal).
    pub async fn watch_project(&self, id: &str) {
        let Some(p) = self.projects.get(id).await else {
            return;
        };
        if let Err(e) = self.watchers.start(id, &p.dir) {
            eprintln!("orso: cannot watch {id}: {e}");
        }
    }

    /// Watch every registered project (daemon start).
    pub async fn watch_all(&self) {
        for p in self.projects.list().await {
            self.watch_project(&p.id).await;
        }
    }

    /// The project's `POST /__orso` endpoint, while its dev server is ready.
    pub async fn content_endpoint(
        &self,
        project: &str,
    ) -> Result<Option<ContentEndpoint>, ApiError> {
        // A change create/switch restarts the preview just before a resumed save; give that
        // restart time to come up instead of reporting the preview as down.
        let deadline = tokio::time::Instant::now() + CONTENT_ENDPOINT_WAIT;
        let p = loop {
            let p = self
                .projects
                .get(project)
                .await
                .ok_or_else(|| ApiError::enoent(format!("no such project: {project}")))?;
            if p.devserver != DevserverStatus::Starting || tokio::time::Instant::now() >= deadline {
                break p;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        };
        if p.devserver != DevserverStatus::Ready {
            return Ok(None);
        }
        Ok(self
            .devservers
            .content_token(project)
            .map(|token| ContentEndpoint {
                url: format!("http://127.0.0.1:{}/__orso", p.port),
                token,
            }))
    }

    /// Full control over scaffolding and dev-server supervision (tests).
    pub fn with_options(config: Config, install: bool, devserver: DevServerOptions) -> Self {
        let current_target = RuntimeTarget::current();
        let target = current_target.unwrap_or(RuntimeTarget::DarwinArm64);
        let lock = runtime_lock(&config);
        let mut runtimes = RuntimeManager::new(config.runtime_dir.clone(), lock.clone(), target);
        if let Some(base_url) = config.runtime_base_url.clone() {
            runtimes = runtimes.with_base_url(base_url);
        }

        let mut resolver = RuntimeResolver::new(config.runtime_dir.clone(), lock, target)
            .with_login_shell_path(cached_login_path())
            .with_daemon_path(std::env::var_os("PATH").unwrap_or_default())
            .with_git_shim(git_shim_dir(&config));
        match crate::providers::load(config.agents_config.as_deref()) {
            Ok(providers) => resolver = resolver.with_providers(providers),
            Err(error) => eprintln!("orso: {}", error.message),
        }
        if let Ok(overrides) = crate::providers::acp_overrides(config.agents_config.as_deref()) {
            resolver = resolver.with_agents_acp(overrides);
        }
        let node_path = resolver.child_path(None);
        let resolver = Arc::new(resolver);

        let projects = ProjectRegistry::open(config.projects_dir.clone(), install)
            .with_install_path(node_path.clone());
        let devserver = devserver.with_path(node_path);
        let github = Arc::new(GithubAuth::open(
            config.github.clone(),
            &config.projects_dir.join(crate::projects::STATE_DIR),
        ));
        let config = Arc::new(config);
        let projects = Arc::new(projects);
        let events = EventBus::new();
        let state_dir = config.projects_dir.join(crate::projects::STATE_DIR);
        let chats = Arc::new(ChatManager::new(events.clone(), state_dir));
        let devservers = Arc::new(DevServerManager::new(
            projects.clone(),
            events.clone(),
            devserver,
        ));
        let watchers = Arc::new(WatchManager::new(events.clone()));
        let logins = Arc::new(LoginManager::new((*resolver).clone()));
        bridge_runtime_events(&runtimes, &events);
        let state = Self {
            config,
            projects,
            events,
            chats,
            devservers,
            watchers,
            github,
            preview: preview::new_client(),
            conflicts: Arc::new(ConflictSessions::new()),
            conflict_previews: Arc::new(ConflictPreviewManager::new()),
            conflict_updates: Arc::new(Mutex::new(())),
            pending_latest: Arc::default(),
            runtimes: Arc::new(runtimes),
            runtime_target: current_target,
            resolver,
            logins,
            setup_probes: Arc::new(SetupProbes::default()),
        };
        state.wire_chat_turn_end();
        state
    }

    /// After an ACP turn ends: refresh the preview and Content panel, checkpoint
    /// the edits, and record the resulting `checkpoint` chunk in the transcript.
    /// A sync chat is verified and advances instead; its worktree never touches
    /// the project checkout, so nothing is checkpointed there.
    fn wire_chat_turn_end(&self) {
        let state = self.clone();
        self.chats.set_on_turn_end(move |ended| {
            if ended.stop_reason != "end_turn" {
                return;
            }
            let state_dir = crate::http::chat::state_dir(&state);
            let is_sync = crate::chat::history::get(&state_dir, &ended.project_id, &ended.chat_id)
                .is_some_and(|record| record.kind == crate::chat::history::RecordKind::Sync);
            if is_sync {
                crate::changes::sync_chat::on_turn_end(&state, &ended);
                return;
            }
            let state = state.clone();
            tokio::spawn(async move {
                crate::http::ctl::emit_preview_reload(&state, &ended.project_id);
                crate::http::ctl::emit_content_refresh(&state, &ended.project_id);
                match crate::http::checkpoints::finish_turn(
                    &state,
                    &ended.project_id,
                    ended.summary.as_deref(),
                )
                .await
                {
                    Ok(outcome) => {
                        if let Some(chunk) = crate::http::chat::checkpoint_chunk(&outcome) {
                            state.chats.record(&ended.project_id, &ended.chat_id, chunk);
                            // The turn-end event raced this async checkpoint; reload once more.
                            state.events.emit(
                                "chat.turn",
                                json!({
                                    "projectId": ended.project_id,
                                    "chatId": ended.chat_id,
                                }),
                            );
                        }
                    }
                    Err(e) => eprintln!("orso: checkpoint after chat turn failed: {e}"),
                }
            });
        });
    }
}

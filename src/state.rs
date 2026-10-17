//! Shared application state.

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::changes::auto_resolve::{AutoResolveJobs, Timeouts};
use crate::changes::conflict_sessions::ConflictSessions;
use crate::chat::manager::ChatManager;
use crate::config::Config;
use crate::conflict_preview::ConflictPreviewManager;
use crate::devserver::{DevServerManager, DevServerOptions};
use crate::events::EventBus;
use crate::github::GithubAuth;
use crate::preview::{self, UpstreamClient};
use crate::projects::ProjectRegistry;
use crate::terminal::TerminalManager;
use crate::watch::WatchManager;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub projects: Arc<ProjectRegistry>,
    /// Broadcaster behind `WS /ws/events`.
    pub events: EventBus,
    /// PTY terminal sessions (one per project).
    pub terminals: Arc<TerminalManager>,
    /// Headless chat turns (one per project, never alongside its terminal).
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
    /// Headless agent runs that resolve a conflict session in a temp worktree.
    pub auto_resolve: Arc<AutoResolveJobs>,
    /// Serializes Get latest through session creation as one operation.
    pub conflict_updates: Arc<Mutex<()>>,
    /// Projects whose Get latest skipped the checked-out change while a terminal was live.
    pub pending_latest: Arc<std::sync::Mutex<HashSet<String>>>,
}

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
            DevServerOptions::npm(proxy_port)
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
            eprintln!("workbench: cannot watch {id}: {e}");
        }
    }

    /// Watch every registered project (daemon start).
    pub async fn watch_all(&self) {
        for p in self.projects.list().await {
            self.watch_project(&p.id).await;
        }
    }

    /// Full control over scaffolding and dev-server supervision (tests).
    pub fn with_options(config: Config, install: bool, devserver: DevServerOptions) -> Self {
        let projects = ProjectRegistry::open(config.projects_dir.clone(), install);
        let github = Arc::new(GithubAuth::open(
            config.github.clone(),
            &config.projects_dir.join(crate::projects::STATE_DIR),
        ));
        let config = Arc::new(config);
        let projects = Arc::new(projects);
        let events = EventBus::new();
        let terminals = Arc::new(TerminalManager::new(config.clone(), events.clone()));
        let chats = Arc::new(ChatManager::new(events.clone()));
        let devservers = Arc::new(DevServerManager::new(
            projects.clone(),
            events.clone(),
            devserver,
        ));
        let watchers = Arc::new(WatchManager::new(events.clone()));
        Self {
            config,
            projects,
            events,
            terminals,
            chats,
            devservers,
            watchers,
            github,
            preview: preview::new_client(),
            conflicts: Arc::new(ConflictSessions::new()),
            conflict_previews: Arc::new(ConflictPreviewManager::new()),
            auto_resolve: Arc::new(AutoResolveJobs::new(Timeouts::default())),
            conflict_updates: Arc::new(Mutex::new(())),
            pending_latest: Arc::default(),
        }
    }
}

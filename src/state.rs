//! Shared application state.

use std::sync::Arc;

use crate::acp::AgentManager;
use crate::config::Config;
use crate::devserver::{DevServerManager, DevServerOptions};
use crate::events::EventBus;
use crate::preview::{self, UpstreamClient};
use crate::projects::ProjectRegistry;
use crate::watch::WatchManager;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub projects: Arc<ProjectRegistry>,
    /// Broadcaster behind `WS /ws/events`.
    pub events: EventBus,
    /// ACP agent sessions (one per project).
    pub agents: Arc<AgentManager>,
    /// Supervised `npm run dev` per project.
    pub devservers: Arc<DevServerManager>,
    /// `fs.change` watcher per project.
    pub watchers: Arc<WatchManager>,
    /// HTTP client for the `/preview/{id}/*` reverse proxy.
    pub preview: UpstreamClient,
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
        let devserver = if install {
            DevServerOptions::npm()
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
        let config = Arc::new(config);
        let projects = Arc::new(projects);
        let events = EventBus::new();
        let agents = Arc::new(AgentManager::new(
            config.clone(),
            projects.clone(),
            events.clone(),
        ));
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
            agents,
            devservers,
            watchers,
            preview: preview::new_client(),
        }
    }
}

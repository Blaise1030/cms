//! Shared application state.

use std::sync::Arc;

use crate::acp::AgentManager;
use crate::config::Config;
use crate::events::EventBus;
use crate::projects::ProjectRegistry;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub projects: Arc<ProjectRegistry>,
    /// Broadcaster behind `WS /ws/events`.
    pub events: EventBus,
    /// ACP agent sessions (one per project).
    pub agents: Arc<AgentManager>,
}

impl AppState {
    /// Production state: project creation runs `npm install`.
    pub fn new(config: Config) -> Self {
        Self::with_scaffold_install(config, true)
    }

    /// Like [`AppState::new`], but `install: false` skips `npm install` during project
    /// creation. This is an internal hook so tests do not need the network.
    pub fn with_scaffold_install(config: Config, install: bool) -> Self {
        let projects = ProjectRegistry::open(config.projects_dir.clone(), install);
        let config = Arc::new(config);
        let projects = Arc::new(projects);
        let events = EventBus::new();
        let agents = Arc::new(AgentManager::new(
            config.clone(),
            projects.clone(),
            events.clone(),
        ));
        Self {
            config,
            projects,
            events,
            agents,
        }
    }
}

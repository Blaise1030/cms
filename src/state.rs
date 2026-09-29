//! Shared application state.

use std::sync::Arc;

use crate::config::Config;
use crate::projects::ProjectRegistry;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub projects: Arc<ProjectRegistry>,
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
        Self {
            config: Arc::new(config),
            projects: Arc::new(projects),
        }
    }
}

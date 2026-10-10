//! workbench daemon library.

pub mod auth;
pub mod changes;
pub mod config;
pub mod content;
pub mod devserver;
pub mod error;
pub mod events;
pub mod fs;
pub mod git;
pub mod github;
pub mod hook;
pub mod http;
pub mod preview;
pub mod projects;
pub mod providers;
pub mod state;
pub mod terminal;
pub mod watch;
// `static` is a keyword, so the file `static.rs` is exposed as `static_files`.
#[path = "static.rs"]
pub mod static_files;

use std::net::SocketAddr;

use config::Config;
use state::AppState;

/// Crate version string.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Serve the HTTP API on an already-bound listener until Ctrl-C.
pub async fn serve_on(listener: tokio::net::TcpListener, state: AppState) -> std::io::Result<()> {
    let terminals = state.terminals.clone();
    let devservers = state.devservers.clone();
    let watchers = state.watchers.clone();
    let identity = state.github.identity().await;
    state.projects.ensure_identities(&identity).await;
    state.github.note_applied(&identity).await;
    state.watch_all().await;
    // Projects that survived a restart get their dev servers back (on a free port).
    devservers.resume_all().await;
    let app = http::router(state);
    let served = axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await;
    terminals.shutdown().await;
    devservers.shutdown().await;
    watchers.shutdown();
    served
}

/// Bind `127.0.0.1:<port>`, print the ready URL, and serve.
pub async fn serve(config: Config) -> std::io::Result<()> {
    config
        .validate()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    let mut config = config;
    config.port = bound.port();
    let state = AppState::new(config);
    if let Some(e) = state.projects.init_error() {
        return Err(std::io::Error::other(e.to_string()));
    }
    let base = match state.config.ui_url.as_deref() {
        Some(url) => url.trim_end_matches('/').to_string(),
        None => format!("http://{bound}"),
    };
    match state.config.token.as_deref() {
        Some(token) if !state.config.no_auth => println!("workbench ready: {base}/?t={token}"),
        _ => println!("workbench ready: {base}/ (auth disabled)"),
    }
    serve_on(listener, state).await
}

//! orso daemon library.

pub mod auth;
pub mod changes;
pub mod chat;
pub mod config;
pub mod conflict_preview;
pub mod content;
pub mod ctl;
pub mod design;
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

/// Serve the HTTP API on an already-bound listener until an interrupt or termination signal.
pub async fn serve_on(listener: tokio::net::TcpListener, state: AppState) -> std::io::Result<()> {
    serve_until(listener, state, shutdown_signal()).await
}

/// Serve the HTTP API on an already-bound listener until `signal` resolves, then shut down.
pub async fn serve_until(
    listener: tokio::net::TcpListener,
    state: AppState,
    signal: impl std::future::Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    let terminals = state.terminals.clone();
    let chats = state.chats.clone();
    let devservers = state.devservers.clone();
    let watchers = state.watchers.clone();
    let previews = state.conflict_previews.clone();
    let sessions = state.conflicts.clone();
    let identity = state.github.identity().await;
    state.projects.ensure_identities(&identity).await;
    state.github.note_applied(&identity).await;
    state.watch_all().await;
    // Projects that survived a restart get their dev servers back (on a free port).
    devservers.resume_all().await;
    let cleanup = tokio::spawn({
        let previews = previews.clone();
        async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                previews.prune(&sessions.active_ids().await).await;
            }
        }
    });
    let app = http::router(state);
    // Graceful shutdown waits for open responses, and a chat turn's SSE body only ends with
    // the turn: end the turns first.
    let signal = {
        let chats = chats.clone();
        async move {
            signal.await;
            chats.shutdown().await;
        }
    };
    let served = axum::serve(listener, app)
        .with_graceful_shutdown(signal)
        .await;
    terminals.shutdown().await;
    chats.shutdown().await;
    devservers.shutdown().await;
    watchers.shutdown();
    cleanup.abort();
    previews.shutdown().await;
    served
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        if let Ok(mut term) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = term.recv() => {} }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
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
        Some(token) if !state.config.no_auth => println!("orso ready: {base}/?t={token}"),
        _ => println!("orso ready: {base}/ (auth disabled)"),
    }
    serve_on(listener, state).await
}

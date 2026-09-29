//! workbench daemon library.

pub mod auth;
pub mod config;
pub mod error;
pub mod http;
pub mod state;

use std::net::SocketAddr;

use config::Config;
use state::AppState;

/// Crate version string.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Serve the HTTP API on an already-bound listener until Ctrl-C.
pub async fn serve_on(listener: tokio::net::TcpListener, state: AppState) -> std::io::Result<()> {
    let app = http::router(state);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}

/// Bind `127.0.0.1:<port>`, print the ready URL, and serve.
pub async fn serve(config: Config) -> std::io::Result<()> {
    config
        .validate()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    let state = AppState::new(config);
    match state.config.token.as_deref() {
        Some(token) if !state.config.no_auth => {
            println!("workbench ready: http://{bound}/?t={token}")
        }
        _ => println!("workbench ready: http://{bound}/ (auth disabled)"),
    }
    serve_on(listener, state).await
}

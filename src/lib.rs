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
pub mod git_shim;
pub mod github;
pub mod http;
pub mod preview;
pub mod projects;
pub mod providers;
pub mod runtime;
pub mod settings;
pub mod setup_prefs;
pub mod shell;
pub mod state;
pub mod watch;
// `static` is a keyword, so the file `static.rs` is exposed as `static_files`.
#[path = "static.rs"]
pub mod static_files;

use std::net::SocketAddr;
use std::process::Command;

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
    let chats = state.chats.clone();
    let devservers = state.devservers.clone();
    let watchers = state.watchers.clone();
    let previews = state.conflict_previews.clone();
    let sessions = state.conflicts.clone();
    let identity = state.github.identity().await;
    state.projects.ensure_identities(&identity).await;
    state.github.note_applied(&identity).await;
    // A sync that survived the restart is reconciled before anything watches it.
    changes::sync_chat::recover(&state).await;
    state.watch_all().await;
    // Projects that survived a restart get their dev servers back (on a free port).
    devservers.resume_all().await;
    let cleanup = tokio::spawn({
        let previews = previews.clone();
        let state = state.clone();
        async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                previews.prune(&sessions.active_ids().await).await;
                changes::sync_chat::sweep(&state).await;
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

/// Default hostname advertised in the ready URL. Still binds `127.0.0.1`; browsers
/// resolve `*.localhost` to loopback. Override with `--hostname`.
pub const DEFAULT_HOSTNAME: &str = "orso.localhost";

/// Public base URL for the ready link (`http://{hostname}:{port}`). Bind stays on loopback.
pub fn advertise_base(hostname: &str, port: u16) -> String {
    format!("http://{hostname}:{port}")
}

/// Build the ready URL printed (and optionally opened) after bind.
pub fn ready_url(base: &str, token: Option<&str>, no_auth: bool) -> String {
    let base = base.trim_end_matches('/');
    match token {
        Some(token) if !no_auth => format!("{base}/?t={token}"),
        _ => format!("{base}/"),
    }
}

/// Open `url` with the OS default browser. Failures are logged; they do not stop serve.
pub fn open_ready_url(url: &str) {
    let result = {
        #[cfg(target_os = "macos")]
        {
            Command::new("open").arg(url).spawn()
        }
        #[cfg(target_os = "linux")]
        {
            Command::new("xdg-open").arg(url).spawn()
        }
        #[cfg(target_os = "windows")]
        {
            Command::new("cmd").args(["/C", "start", "", url]).spawn()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "open browser is not supported on this OS",
            ))
        }
    };
    if let Err(e) = result {
        eprintln!("orso: could not open browser: {e}");
    }
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
    let open_browser = config.open_browser;
    let state = AppState::new(config);
    if let Some(e) = state.projects.init_error() {
        return Err(std::io::Error::other(e.to_string()));
    }
    let base = match state.config.ui_url.as_deref() {
        Some(url) => url.trim_end_matches('/').to_string(),
        None => advertise_base(&state.config.hostname, bound.port()),
    };
    let url = ready_url(&base, state.config.token.as_deref(), state.config.no_auth);
    if state.config.no_auth || state.config.token.is_none() {
        println!("orso ready: {url} (auth disabled)");
    } else {
        println!("orso ready: {url}");
    }
    if open_browser {
        open_ready_url(&url);
    }
    serve_on(listener, state).await
}

#[cfg(test)]
mod ready_url_tests {
    use super::{advertise_base, ready_url};

    #[test]
    fn ready_url_with_token() {
        assert_eq!(
            ready_url("http://127.0.0.1:7800", Some("abc"), false),
            "http://127.0.0.1:7800/?t=abc"
        );
    }

    #[test]
    fn ready_url_trims_trailing_slash() {
        assert_eq!(
            ready_url("http://localhost:5173/", Some("ab12"), false),
            "http://localhost:5173/?t=ab12"
        );
    }

    #[test]
    fn ready_url_no_auth() {
        assert_eq!(
            ready_url("http://127.0.0.1:7800", Some("abc"), true),
            "http://127.0.0.1:7800/"
        );
    }

    #[test]
    fn advertise_base_defaults_to_orso_localhost() {
        assert_eq!(
            advertise_base("orso.localhost", 7800),
            "http://orso.localhost:7800"
        );
    }

    #[test]
    fn advertise_base_respects_custom_hostname() {
        assert_eq!(
            advertise_base("myapp.localhost", 7810),
            "http://myapp.localhost:7810"
        );
    }
}

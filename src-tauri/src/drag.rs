//! The main window has no title bar, so the daemon's web UI moves it from its
//! `data-tauri-drag-region` bars, which needs `start_dragging` from a remote origin. A static
//! capability could only name `http://127.0.0.1:*`, which would also cover project dev servers
//! opened in the main window, so the exact daemon origin is granted once its port is known.

use std::error::Error;

use tauri::ipc::CapabilityBuilder;
use tauri::{Manager, Runtime, Url};

const MAIN_WINDOW: &str = "main";

/// Lets pages from `base_url`'s origin in the main window start dragging it, and nothing else.
/// Tauri can't revoke capabilities, so an earlier daemon's origin keeps its grant after a
/// restart on a new port; nothing listens there any more.
pub fn allow_daemon_origin<R: Runtime>(
    app: &impl Manager<R>,
    base_url: &str,
) -> Result<(), Box<dyn Error>> {
    let url = Url::parse(base_url)?;
    let port = url
        .port()
        .filter(|_| url.scheme() == "http" && url.host_str() == Some("127.0.0.1"))
        .ok_or_else(|| format!("not a loopback daemon address: {base_url}"))?;
    app.add_capability(
        CapabilityBuilder::new(format!("daemon-window-drag-{port}"))
            .remote(format!("http://127.0.0.1:{port}"))
            .local(false)
            .window(MAIN_WINDOW)
            .permission("core:window:allow-start-dragging"),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_http_daemon_addresses_are_granted() {
        let app = tauri::test::mock_builder()
            .build(tauri::generate_context!(test = true))
            .expect("mock app");
        for base_url in [
            "http://localhost:51234",
            "http://127.0.0.2:51234",
            "https://127.0.0.1:51234",
            "http://127.0.0.1",
            "not a url",
        ] {
            assert!(allow_daemon_origin(&app, base_url).is_err(), "{base_url}");
        }
        assert!(allow_daemon_origin(&app, "http://127.0.0.1:51234").is_ok());
    }
}

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
pub fn allow_origin<R: Runtime>(
    app: &impl Manager<R>,
    base_url: &str,
) -> Result<(), Box<dyn Error>> {
    let (host, port) = grantable(base_url, cfg!(debug_assertions))
        .ok_or_else(|| format!("not a loopback daemon address: {base_url}"))?;
    app.add_capability(
        CapabilityBuilder::new(format!("window-drag-{host}-{port}"))
            .remote(format!("http://{host}:{port}"))
            .local(false)
            .window(MAIN_WINDOW)
            .permission("core:window:allow-start-dragging"),
    )?;
    Ok(())
}

/// `http://127.0.0.1:<port>` (the daemon), plus `http://localhost:<port>` (Vite, for
/// `just desktop-dev`) only when `dev_build`.
fn grantable(base_url: &str, dev_build: bool) -> Option<(String, u16)> {
    let url = Url::parse(base_url).ok()?;
    let host = url.host_str()?;
    let allowed =
        url.scheme() == "http" && (host == "127.0.0.1" || dev_build && host == "localhost");
    allowed.then_some((host.to_owned(), url.port()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEVER: [&str; 6] = [
        "https://127.0.0.1:51234",
        "https://localhost:5173",
        "http://127.0.0.2:51234",
        "http://example.com:80",
        "http://127.0.0.1",
        "not a url",
    ];

    #[test]
    fn release_builds_grant_only_the_loopback_daemon() {
        assert_eq!(
            grantable("http://127.0.0.1:51234", false),
            Some(("127.0.0.1".into(), 51234))
        );
        assert_eq!(grantable("http://localhost:5173", false), None);
        for base_url in NEVER {
            assert_eq!(grantable(base_url, false), None, "{base_url}");
        }
    }

    #[test]
    fn dev_builds_also_grant_the_vite_origin() {
        assert_eq!(
            grantable("http://localhost:5173/", true),
            Some(("localhost".into(), 5173))
        );
        assert_eq!(
            grantable("http://127.0.0.1:7800", true),
            Some(("127.0.0.1".into(), 7800))
        );
        for base_url in NEVER {
            assert_eq!(grantable(base_url, true), None, "{base_url}");
        }
    }

    #[test]
    fn refused_addresses_are_not_granted() {
        let app = tauri::test::mock_builder()
            .build(tauri::generate_context!(test = true))
            .expect("mock app");
        for base_url in NEVER {
            assert!(allow_origin(&app, base_url).is_err(), "{base_url}");
        }
        assert!(allow_origin(&app, "http://127.0.0.1:51234").is_ok());
    }
}

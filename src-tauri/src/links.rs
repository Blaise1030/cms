//! New-window requests from the webview (`target=_blank` links, `window.open`). Without a
//! handler WebKit drops them, so install pages and GitHub links would do nothing. No new
//! webview is ever allowed (WebKit would build one sharing the IPC bridge, in a window Tauri
//! does not track): the running daemon's own pages open in the main window, other web pages
//! in the default browser.

use std::process::Command;

use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Manager, Url, Wry};

use crate::logs;
use crate::supervisor::Supervisor;

#[derive(Debug, PartialEq, Eq)]
pub enum NewWindow {
    Browser,
    Main,
    Deny,
}

/// `daemon` is the running daemon's base URL, `None` while it is down or restarting. Only
/// its exact origin counts; other loopback ports (e.g. a dev server) are ordinary web pages.
pub fn classify(url: &Url, daemon: Option<&Url>) -> NewWindow {
    match url.scheme() {
        "http" | "https" if daemon.is_some_and(|d| d.origin() == url.origin()) => NewWindow::Main,
        "http" | "https" => NewWindow::Browser,
        _ => NewWindow::Deny,
    }
}

pub fn on_new_window(app: &AppHandle, url: Url) -> NewWindowResponse<Wry> {
    let daemon = app
        .try_state::<Supervisor>()
        .and_then(|s| s.handle())
        .and_then(|h| Url::parse(&h.base_url).ok());
    match classify(&url, daemon.as_ref()) {
        NewWindow::Main => {
            // Navigating from inside WebKit's new-window callback would re-enter it.
            let app = app.clone();
            std::thread::spawn(move || {
                let label = url.path().to_owned();
                crate::supervisor::navigate_main(&app, url, &label);
            });
            NewWindowResponse::Deny
        }
        NewWindow::Browser => {
            open_in_browser(url);
            NewWindowResponse::Deny
        }
        NewWindow::Deny => {
            logs::shell(&format!("blocked new window for {}", url.scheme()));
            NewWindowResponse::Deny
        }
    }
}

/// `classify` only passes `http(s)` URLs here, so `open` cannot read the argument as a flag.
fn open_in_browser(url: Url) {
    std::thread::spawn(
        move || match Command::new("/usr/bin/open").arg(url.as_str()).status() {
            Ok(s) if s.success() => {}
            Ok(s) => logs::shell(&format!(
                "could not open link in browser: open exited with {s}"
            )),
            Err(e) => logs::shell(&format!("could not open link in browser: {e}")),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAEMON: &str = "http://127.0.0.1:51234";

    fn class(url: &str, daemon: Option<&str>) -> NewWindow {
        let daemon = daemon.map(|d| d.parse::<Url>().unwrap());
        classify(&url.parse().unwrap(), daemon.as_ref())
    }

    #[test]
    fn web_pages_open_in_the_browser() {
        for url in [
            "https://git-scm.com/downloads/mac",
            "https://github.com/login/device",
            "http://example.com/",
            "http://127.0.0.1@evil.example/",
            "http://127.0.0.1:51234@evil.example/",
        ] {
            assert_eq!(class(url, Some(DAEMON)), NewWindow::Browser, "{url}");
        }
    }

    #[test]
    fn daemon_pages_open_in_the_main_window() {
        for url in [
            "http://127.0.0.1:51234/preview/site/",
            "http://127.0.0.1:51234/api/docs/branch-protection",
        ] {
            assert_eq!(class(url, Some(DAEMON)), NewWindow::Main, "{url}");
        }
    }

    #[test]
    fn other_loopback_origins_are_not_the_daemon() {
        for url in [
            "http://127.0.0.1:4321/",
            "http://localhost:51234/",
            "http://[::1]:51234/",
            "https://127.0.0.1:51234/",
        ] {
            assert_eq!(class(url, Some(DAEMON)), NewWindow::Browser, "{url}");
        }
    }

    #[test]
    fn without_a_running_daemon_nothing_is_the_daemon() {
        assert_eq!(
            class("http://127.0.0.1:51234/preview/site/", None),
            NewWindow::Browser
        );
    }

    #[test]
    fn other_schemes_are_denied() {
        for url in [
            "tauri://localhost/error.html",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "mailto:a@b.c",
        ] {
            assert_eq!(class(url, Some(DAEMON)), NewWindow::Deny, "{url}");
            assert_eq!(class(url, None), NewWindow::Deny, "{url}");
        }
    }
}

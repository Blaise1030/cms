//! New-window requests from the webview (`target=_blank` links, `window.open`). Without a
//! handler WebKit drops them, so install pages and GitHub links would do nothing. WebKit is
//! never allowed to build the window itself (it would share the opener's IPC bridge, in a
//! window Tauri does not track): the running daemon's own pages (e.g. a preview opened in a
//! new tab) open in a fresh Tauri window that no capability grants IPC, other web pages in
//! the default browser.

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder, Wry};

use crate::logs;
use crate::supervisor::Supervisor;

#[derive(Debug, PartialEq, Eq)]
pub enum NewWindow {
    Browser,
    Secondary,
    Deny,
}

/// Window label for the `n`th secondary window. Capabilities only name `main`, so these get
/// no IPC.
pub fn secondary_label(n: u64) -> String {
    format!("preview-{n}")
}

/// `app_origins` are the daemon (and, in `just desktop-dev`, Vite). Only their exact
/// origins open as secondary windows; other loopback ports are ordinary web pages.
pub fn classify(url: &Url, app_origins: &[Url]) -> NewWindow {
    match url.scheme() {
        "http" | "https" if app_origins.iter().any(|o| o.origin() == url.origin()) => {
            NewWindow::Secondary
        }
        "http" | "https" => NewWindow::Browser,
        _ => NewWindow::Deny,
    }
}

pub fn on_new_window(app: &AppHandle, url: Url) -> NewWindowResponse<Wry> {
    let mut app_origins = Vec::new();
    if let Some(daemon) = app
        .try_state::<Supervisor>()
        .and_then(|s| s.handle())
        .and_then(|h| Url::parse(&h.base_url).ok())
    {
        app_origins.push(daemon);
    }
    // Vite proxies /preview to the daemon; treat its origin like the daemon's so
    // "Open in new tab" still opens a managed secondary window in desktop-dev.
    if let Some(ui) = crate::dev::plan().ui {
        app_origins.push(ui);
    }
    match classify(&url, &app_origins) {
        NewWindow::Secondary => {
            open_secondary(app, url);
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

/// Built off WebKit's new-window callback, which a synchronous window build would re-enter.
/// The default data store is shared, so the daemon's session cookie still applies.
fn open_secondary(app: &AppHandle, url: Url) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let label = secondary_label(NEXT.fetch_add(1, Ordering::Relaxed));
    let app = app.clone();
    std::thread::spawn(move || {
        let handler_app = app.clone();
        let built = WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(url))
            .title("Workbench")
            .inner_size(1200.0, 800.0)
            .on_new_window(move |url, _| on_new_window(&handler_app, url))
            .on_document_title_changed(|window, title| {
                let _ = window.set_title(&title);
            })
            .build();
        match built {
            Ok(_) => logs::shell(&format!("opened window {label}")),
            Err(e) => logs::shell(&format!("could not open window {label}: {e}")),
        }
    });
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
    const VITE: &str = "http://localhost:5180";

    fn origins(bases: &[&str]) -> Vec<Url> {
        bases.iter().map(|b| b.parse().unwrap()).collect()
    }

    fn class(url: &str, bases: &[&str]) -> NewWindow {
        classify(&url.parse().unwrap(), &origins(bases))
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
            assert_eq!(class(url, &[DAEMON]), NewWindow::Browser, "{url}");
        }
    }

    #[test]
    fn daemon_pages_open_in_a_secondary_window() {
        for url in [
            "http://127.0.0.1:51234/preview/site/",
            "http://127.0.0.1:51234/api/docs/branch-protection",
        ] {
            assert_eq!(class(url, &[DAEMON]), NewWindow::Secondary, "{url}");
        }
    }

    #[test]
    fn vite_preview_pages_open_in_a_secondary_window_in_desktop_dev() {
        assert_eq!(
            class("http://localhost:5180/preview/site/", &[DAEMON, VITE]),
            NewWindow::Secondary
        );
        // Without the Vite origin in the list, same URL is a normal web page.
        assert_eq!(
            class("http://localhost:5180/preview/site/", &[DAEMON]),
            NewWindow::Browser
        );
    }

    #[test]
    fn secondary_labels_are_unique_and_never_main() {
        assert_eq!(secondary_label(1), "preview-1");
        assert_ne!(secondary_label(1), secondary_label(2));
    }

    #[test]
    fn other_loopback_origins_are_not_the_daemon() {
        for url in [
            "http://127.0.0.1:4321/",
            "http://localhost:51234/",
            "http://[::1]:51234/",
            "https://127.0.0.1:51234/",
        ] {
            assert_eq!(class(url, &[DAEMON]), NewWindow::Browser, "{url}");
        }
    }

    #[test]
    fn without_a_running_daemon_nothing_is_the_daemon() {
        assert_eq!(
            class("http://127.0.0.1:51234/preview/site/", &[]),
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
            assert_eq!(class(url, &[DAEMON]), NewWindow::Deny, "{url}");
            assert_eq!(class(url, &[]), NewWindow::Deny, "{url}");
        }
    }
}

//! New-window requests from the webview (`target=_blank` links, `window.open`). Without a
//! handler WebKit drops them, so install pages and GitHub links would do nothing. Web pages
//! open in the default browser; the daemon's own pages (preview, bundled docs) open in an
//! app window that shares the session cookie.

use std::net::IpAddr;
use std::process::Command;

use tauri::webview::NewWindowResponse;
use tauri::{Runtime, Url};

use crate::logs;

#[derive(Debug, PartialEq, Eq)]
pub enum NewWindow {
    Browser,
    App,
    Deny,
}

pub fn classify(url: &Url) -> NewWindow {
    match url.scheme() {
        "http" | "https" if is_loopback(url) => NewWindow::App,
        "http" | "https" => NewWindow::Browser,
        _ => NewWindow::Deny,
    }
}

fn is_loopback(url: &Url) -> bool {
    match url.host_str() {
        Some(host) if host.eq_ignore_ascii_case("localhost") => true,
        Some(host) => host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback()),
        None => false,
    }
}

pub fn on_new_window<R: Runtime>(url: Url) -> NewWindowResponse<R> {
    match classify(&url) {
        NewWindow::App => NewWindowResponse::Allow,
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

    fn class(url: &str) -> NewWindow {
        classify(&url.parse().unwrap())
    }

    #[test]
    fn web_pages_open_in_the_browser() {
        for url in [
            "https://git-scm.com/downloads/mac",
            "https://github.com/login/device",
            "http://example.com/",
            "http://127.0.0.1@evil.example/",
        ] {
            assert_eq!(class(url), NewWindow::Browser, "{url}");
        }
    }

    #[test]
    fn daemon_pages_stay_in_the_app() {
        for url in [
            "http://127.0.0.1:51234/preview/site/",
            "http://localhost:51234/api/docs/branch-protection",
            "http://[::1]:51234/",
        ] {
            assert_eq!(class(url), NewWindow::App, "{url}");
        }
    }

    #[test]
    fn other_schemes_are_denied() {
        for url in [
            "tauri://localhost/error.html",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "mailto:a@b.c",
        ] {
            assert_eq!(class(url), NewWindow::Deny, "{url}");
        }
    }
}

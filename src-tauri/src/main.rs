//! orso desktop: start the `orso serve` daemon, point the webview at its ready URL, and stop the
//! daemon (which stops its dev servers, chats and previews) when the app quits.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

mod snapshot;

use tauri::{Manager, RunEvent, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

/// How long the daemon gets to stop its children after SIGTERM before it is killed.
const STOP_GRACE: Duration = Duration::from_secs(10);

struct Daemon(Mutex<Option<Child>>);

/// `ORSO_BIN` wins (handy in dev); otherwise the sidecar Tauri puts next to our executable.
fn daemon_path() -> PathBuf {
    if let Some(p) = std::env::var_os("ORSO_BIN") {
        return p.into();
    }
    let exe = std::env::current_exe().expect("current exe");
    exe.parent().expect("exe dir").join("orso")
}

/// Apps launched from Finder get a bare PATH; add the usual tool locations so git etc. resolve.
fn launch_path() -> String {
    let current = std::env::var("PATH").unwrap_or_default();
    let mut parts: Vec<&str> = vec!["/opt/homebrew/bin", "/usr/local/bin"];
    parts.extend(current.split(':').filter(|p| !p.is_empty()));
    parts.extend(["/usr/bin", "/bin", "/usr/sbin", "/sbin"]);
    let mut seen = std::collections::HashSet::new();
    parts.retain(|p| seen.insert(*p));
    parts.join(":")
}

fn show_error(window: &WebviewWindow, message: &str) {
    let msg = serde_json_string(message);
    let _ = window.eval(format!(
        "document.getElementById('status').textContent='orso could not start';\
         const e=document.getElementById('error');e.style.display='block';e.textContent={msg};"
    ));
}

/// Minimal JS string literal escaping (avoids a serde_json dependency).
fn serde_json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Marks the page as the macOS desktop shell before the app's scripts run; the web UI then leaves
/// room for the traffic lights and makes its top bars window drag regions (see web/src/lib/desktop.ts).
const DESKTOP_MARKER: &str = r#"document.documentElement.dataset.desktop = "macos";"#;

/// No title bar: the traffic lights float over the web UI's 44px top bar, vertically centred.
fn build_window(app: &tauri::App) -> tauri::Result<WebviewWindow> {
    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("orso")
        .inner_size(1440.0, 900.0)
        .min_inner_size(900.0, 600.0)
        .initialization_script(DESKTOP_MARKER)
        // Tauri's native file-drop handler swallows every drag, which breaks the web UI's own
        // HTML5 drag and drop (rail reordering, dropping images into content). The UI handles
        // drops itself, so the native handler stays off.
        .disable_drag_drop_handler()
        // In-window navigation stays on the app and the daemon; anything else (GitHub, docs,
        // a published site) opens in the default browser instead.
        .on_navigation(|url| {
            if is_internal(url) {
                return true;
            }
            open_external(url);
            false
        })
        // target=_blank links and window.open() never get a native window: they go to the browser.
        .on_new_window(|url, _| {
            open_external(&url);
            tauri::webview::NewWindowResponse::Deny
        });
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(14.0, 22.0));
    let window = builder.build()?;
    let w = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Resized(_) = event {
            if let Ok(fullscreen) = w.is_fullscreen() {
                sync_fullscreen(w.app_handle(), fullscreen);
            }
        }
    });
    Ok(window)
}

/// The bundled loading page and anything on loopback (the daemon's UI and its previews).
fn is_internal(url: &Url) -> bool {
    match url.scheme() {
        "tauri" | "about" | "data" | "blob" => true,
        "http" | "https" => matches!(
            url.host_str(),
            Some("127.0.0.1" | "localhost" | "[::1]" | "tauri.localhost")
        ),
        _ => false,
    }
}

/// The daemon's origin and auth token, from its ready URL.
static DAEMON_AUTH: OnceLock<(String, String)> = OnceLock::new();

fn remember_daemon(ready: &Url) {
    if let Some((_, token)) = ready.query_pairs().find(|(k, _)| k == "t") {
        let _ = DAEMON_AUTH.set((ready.origin().ascii_serialization(), token.into_owned()));
    }
}

/// The browser has no session with the daemon (only the app's webview does), so a daemon page
/// opened there carries the token; the daemon swaps it for its session cookie, as it does for
/// `orso serve --open`.
fn with_daemon_token(url: &Url) -> Url {
    let mut url = url.clone();
    if let Some((origin, token)) = DAEMON_AUTH.get() {
        let ours = url.origin().ascii_serialization() == *origin;
        if ours && !url.query_pairs().any(|(k, _)| k == "t") {
            url.query_pairs_mut().append_pair("t", token);
        }
    }
    url
}

/// Hand a URL to the OS: http(s) to the default browser, mailto: to the mail app, and so on.
fn open_external(url: &Url) {
    if !matches!(url.scheme(), "http" | "https" | "mailto") {
        eprintln!("orso-desktop: not opening {url}: unsupported scheme");
        return;
    }
    let url = with_daemon_token(url).to_string();
    // Waited on a thread so the short-lived `open` is reaped, not left as a zombie.
    std::thread::spawn(move || {
        if let Err(e) = Command::new("open").arg(&url).status() {
            eprintln!("orso-desktop: could not open {url}: {e}");
        }
    });
}

/// `data-fullscreen` on <html> drives the web UI's `fullscreen:` variant (traffic lights hidden).
fn sync_fullscreen(app: &tauri::AppHandle, fullscreen: bool) {
    if let Some(webview) = app.get_webview_window("main") {
        let _ = webview.eval(format!(
            "document.documentElement.toggleAttribute('data-fullscreen', {fullscreen})"
        ));
    }
}

fn start_daemon(window: WebviewWindow) -> std::io::Result<Child> {
    let mut child = Command::new(daemon_path())
        // Port 0: the daemon picks a free port and prints it in the ready line.
        // 127.0.0.1 rather than orso.localhost: WKWebView does not resolve *.localhost.
        .args(["serve", "--port", "0", "--hostname", "127.0.0.1"])
        .env("PATH", launch_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let stdout = child.stdout.take().expect("piped stdout");
    std::thread::spawn(move || {
        let mut ready = false;
        // Keep draining stdout for the daemon's lifetime so its writes never block.
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            println!("{line}");
            if ready {
                continue;
            }
            if let Some(rest) = line.strip_prefix("orso ready: ") {
                let url = rest.split_whitespace().next().unwrap_or_default();
                match Url::parse(url) {
                    Ok(url) => {
                        ready = true;
                        remember_daemon(&url);
                        let _ = window.navigate(url);
                    }
                    Err(e) => show_error(&window, &format!("bad ready URL {url:?}: {e}")),
                }
            }
        }
        if !ready {
            show_error(
                &window,
                "The orso daemon exited before it was ready. Run the app from a terminal to see its log.",
            );
        }
    });
    Ok(child)
}

/// SIGTERM lets the daemon run its graceful shutdown (chats, dev servers, previews); if it is
/// still alive after the grace period it is killed.
fn stop_daemon(child: &mut Child) {
    if matches!(child.try_wait(), Ok(Some(_))) {
        return;
    }
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }
    let deadline = Instant::now() + STOP_GRACE;
    while Instant::now() < deadline {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    eprintln!("orso-desktop: daemon did not stop in {STOP_GRACE:?}; killing it");
    let _ = child.kill();
    let _ = child.wait();
}

/// A review screenshot: `region` (window CSS px) of the app's own webview, as a PNG data URL.
#[tauri::command]
async fn capture_region(window: WebviewWindow, region: snapshot::Region) -> Result<String, String> {
    snapshot::capture(&window, region).await
}

fn main() {
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![capture_region])
        .on_page_load(|webview, _| {
            // A reload in full screen must not lose the attribute.
            if let Ok(fullscreen) = webview.window().is_fullscreen() {
                sync_fullscreen(webview.app_handle(), fullscreen);
            }
        })
        .setup(|app| {
            let window = build_window(app)?;
            let child = match start_daemon(window.clone()) {
                Ok(child) => Some(child),
                Err(e) => {
                    show_error(&window, &format!("Could not run {:?}: {e}", daemon_path()));
                    None
                }
            };
            app.manage(Daemon(Mutex::new(child)));
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building orso desktop");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            if let Some(mut child) = app.state::<Daemon>().0.lock().unwrap().take() {
                stop_daemon(&mut child);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_links_opened_in_the_browser_carry_the_token() {
        remember_daemon(&Url::parse("http://127.0.0.1:5000/?t=abc").unwrap());
        let preview = Url::parse("http://127.0.0.1:5000/preview/cms/blog/post/").unwrap();
        assert_eq!(
            with_daemon_token(&preview).as_str(),
            "http://127.0.0.1:5000/preview/cms/blog/post/?t=abc"
        );
        // Another origin (a dev server on another port, GitHub) never gets the token.
        for other in ["http://127.0.0.1:5001/", "https://github.com/x"] {
            let url = Url::parse(other).unwrap();
            assert_eq!(with_daemon_token(&url), url);
        }
        // An existing query is kept, and a link that already has a token is left alone.
        let q = Url::parse("http://127.0.0.1:5000/preview/x?page=2").unwrap();
        assert_eq!(
            with_daemon_token(&q).as_str(),
            "http://127.0.0.1:5000/preview/x?page=2&t=abc"
        );
        let has = Url::parse("http://127.0.0.1:5000/?t=zzz").unwrap();
        assert_eq!(with_daemon_token(&has), has);
    }
}

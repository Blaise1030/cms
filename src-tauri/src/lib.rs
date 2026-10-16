mod busy;
mod commands;
mod dev;
mod drag;
pub mod links;
pub mod logs;
pub mod supervisor;
#[cfg(feature = "update-e2e")]
mod update_e2e;
mod updater;

use tauri::webview::PageLoadEvent;
use tauri::{Manager, RunEvent, WebviewWindowBuilder};

use supervisor::Supervisor;
use updater::Updates;

/// Tells pages in the title-bar-less main window to clear the traffic lights and mark drag
/// regions (`web/src/lib/desktop.ts`). Runs before page scripts, on every navigation.
const DESKTOP_MARKER: &str = r#"(() => {
  const mark = () => { document.documentElement.dataset.desktop = "macos"; };
  if (document.documentElement) mark();
  else document.addEventListener("DOMContentLoaded", mark);
})();"#;

pub fn run() {
    #[cfg(feature = "update-e2e")]
    update_e2e::init();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Supervisor::default())
        .manage(Updates::default())
        .invoke_handler(tauri::generate_handler![
            commands::collect_logs,
            commands::retry_daemon,
            commands::apply_update
        ])
        .setup(|app| {
            // Built here rather than from the config so new-window requests get a handler.
            let config = app
                .config()
                .app
                .windows
                .iter()
                .find(|w| w.label == "main")
                .cloned()
                .ok_or("no main window in tauri.conf.json")?;
            let handle = app.handle().clone();
            let window = WebviewWindowBuilder::from_config(app.handle(), &config)?
                .initialization_script(DESKTOP_MARKER)
                .on_new_window(move |url, _| links::on_new_window(&handle, url))
                .on_page_load(|window, payload| {
                    if payload.event() == PageLoadEvent::Finished {
                        updater::page_loaded(window.app_handle());
                    }
                })
                .build()?;
            match window.url() {
                Ok(url) => logs::shell(&format!("main window showing {url}")),
                Err(e) => logs::shell(&format!("main window URL unavailable: {e}")),
            }
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                if let Err(e) = Supervisor::start(&handle) {
                    logs::shell(&format!("daemon unavailable: {e}"));
                }
            });
            // A dev build would otherwise download the latest release.
            if dev::plan().ui.is_none() {
                updater::start(app.handle());
            }
            #[cfg(feature = "update-e2e")]
            update_e2e::start(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Workbench");

    app.run(|app, event| {
        if let RunEvent::ExitRequested { .. } | RunEvent::Exit = event {
            if let Some(supervisor) = app.try_state::<Supervisor>() {
                supervisor.shutdown();
            }
        }
    });
}

#[cfg(test)]
mod update_e2e_gate_tests {
    #[test]
    fn update_e2e_hooks_are_not_in_default_builds() {
        let update_e2e_enabled = cfg!(feature = "update-e2e");
        assert!(!update_e2e_enabled);
    }

    #[test]
    fn release_workflow_never_enables_update_e2e() {
        let workflow = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../.github/workflows/release.yml");
        let workflow = std::fs::read_to_string(workflow).expect("release.yml");
        assert!(!workflow.contains("update-e2e"));
    }
}

#[cfg(test)]
mod main_window_tests {
    #[test]
    fn main_window_has_no_title_bar_but_keeps_the_traffic_lights() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).expect("tauri.conf.json");
        let windows = config["app"]["windows"].as_array().expect("windows");
        let main = windows
            .iter()
            .find(|w| w["label"] == "main")
            .expect("main window");
        assert_eq!(main["titleBarStyle"], "Overlay");
        assert_eq!(main["hiddenTitle"], true);
        assert_eq!(main["decorations"], serde_json::Value::Null);
    }
}

#[cfg(test)]
mod ipc_acl_tests {
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{get_ipc_response, mock_builder, INVOKE_KEY};
    use tauri::webview::InvokeRequest;
    use tauri::WebviewWindowBuilder;

    // Stand-ins with the real names: the ACL check runs before dispatch, so
    // these exercise the app's actual manifest and capabilities.
    #[tauri::command]
    fn collect_logs() -> &'static str {
        "ran"
    }
    #[tauri::command]
    fn retry_daemon() -> &'static str {
        "ran"
    }
    #[tauri::command]
    fn apply_update() -> &'static str {
        "ran"
    }
    #[tauri::command]
    fn unlisted() -> &'static str {
        "ran"
    }

    const SHELL_COMMANDS: [&str; 3] = ["collect_logs", "retry_daemon", "apply_update"];

    fn invoke(cmd: &str, url: &str) -> Result<String, serde_json::Value> {
        invoke_from("main", cmd, url)
    }

    fn invoke_from(label: &str, cmd: &str, url: &str) -> Result<String, serde_json::Value> {
        respond(label, cmd, url).map(|body| body.deserialize::<String>().expect("string response"))
    }

    fn respond(
        label: &str,
        cmd: &str,
        url: &str,
    ) -> Result<tauri::ipc::InvokeResponseBody, serde_json::Value> {
        respond_after(&[], label, cmd, url)
    }

    /// `Err` with the ACL's refusal, or `Ok` if the command ran (whatever it returned).
    /// `daemons` are the base URLs the supervisor has published so far, in order.
    fn respond_after(
        daemons: &[&str],
        label: &str,
        cmd: &str,
        url: &str,
    ) -> Result<tauri::ipc::InvokeResponseBody, serde_json::Value> {
        let app = mock_builder()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .invoke_handler(tauri::generate_handler![
                collect_logs,
                retry_daemon,
                apply_update,
                unlisted
            ])
            .build(tauri::generate_context!(test = true))
            .expect("mock app");
        for base_url in daemons {
            crate::drag::allow_origin(&app, base_url).expect("grant");
        }
        let webview = WebviewWindowBuilder::new(&app, label, Default::default())
            .build()
            .expect("mock window");
        get_ipc_response(
            &webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: url.parse().expect("url"),
                body: InvokeBody::default(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
    }

    fn refused_after(daemons: &[&str], label: &str, cmd: &str, url: &str) -> bool {
        match respond_after(daemons, label, cmd, url) {
            Ok(_) => false,
            Err(err) => err.as_str().unwrap_or_default().contains("not allowed"),
        }
    }

    const START_DRAGGING: &str = "plugin:window|start_dragging";
    const DAEMON_BASE: &str = "http://127.0.0.1:51234";
    const DAEMON: &str = "http://127.0.0.1:51234/";

    fn capabilities() -> Vec<(std::path::PathBuf, serde_json::Value)> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
        std::fs::read_dir(&dir)
            .expect("capabilities dir")
            .map(|entry| {
                let path = entry.expect("entry").path();
                let json = std::fs::read_to_string(&path).expect("read");
                (path, serde_json::from_str(&json).expect("capability JSON"))
            })
            .collect()
    }

    #[test]
    fn bundled_pages_can_drag_the_main_window() {
        for page in ["splash.html", "error.html", "update.html"] {
            let url = format!("tauri://localhost/{page}");
            assert!(respond("main", START_DRAGGING, &url).is_ok(), "{page}");
        }
    }

    #[test]
    fn the_published_daemon_origin_can_drag_the_main_window() {
        for url in [DAEMON, "http://127.0.0.1:51234/p/site/preview?x=1#y"] {
            assert!(
                refused_after(&[], "main", START_DRAGGING, url),
                "{url} before publish"
            );
            assert!(
                respond_after(&[DAEMON_BASE], "main", START_DRAGGING, url).is_ok(),
                "{url}"
            );
        }
    }

    #[test]
    fn a_restarted_daemon_on_a_new_port_can_drag() {
        let restarted = "http://127.0.0.1:51300/";
        assert!(refused_after(
            &[DAEMON_BASE],
            "main",
            START_DRAGGING,
            restarted
        ));
        assert!(respond_after(
            &[DAEMON_BASE, "http://127.0.0.1:51300"],
            "main",
            START_DRAGGING,
            restarted
        )
        .is_ok());
    }

    #[test]
    fn in_desktop_dev_the_vite_origin_can_drag() {
        // Granting localhost at all is debug-only; see drag::tests for the release refusal.
        let granted = [DAEMON_BASE, "http://localhost:5173"];
        assert!(respond_after(&granted, "main", START_DRAGGING, "http://localhost:5173/").is_ok());
        assert!(refused_after(
            &granted,
            "main",
            START_DRAGGING,
            "http://localhost:5174/"
        ));
    }

    #[test]
    fn other_origins_in_the_main_window_cannot_drag() {
        for url in [
            "http://127.0.0.1:4321/",
            "http://127.0.0.1:5123/",
            "http://localhost:51234/",
            "http://127.0.0.2:51234/",
            "https://127.0.0.1:51234/",
            "https://example.com/",
        ] {
            assert!(
                refused_after(&[DAEMON_BASE], "main", START_DRAGGING, url),
                "{url}"
            );
        }
    }

    #[test]
    fn the_daemon_origin_gets_no_other_window_control() {
        for cmd in [
            "plugin:window|internal_toggle_maximize",
            "plugin:window|close",
            "plugin:window|set_title",
            "plugin:window|set_size",
            "plugin:webview|create_webview_window",
            "plugin:event|emit",
        ] {
            assert!(refused_after(&[DAEMON_BASE], "main", cmd, DAEMON), "{cmd}");
        }
        for cmd in SHELL_COMMANDS {
            assert!(
                respond_after(&[DAEMON_BASE], "main", cmd, DAEMON).is_err(),
                "{cmd}"
            );
        }
    }

    #[test]
    fn secondary_windows_cannot_be_dragged_by_their_pages() {
        let label = crate::links::secondary_label(1);
        for url in [DAEMON, "http://127.0.0.1:51234/preview/site/"] {
            assert!(
                refused_after(&[DAEMON_BASE], &label, START_DRAGGING, url),
                "{url}"
            );
        }
    }

    #[test]
    fn no_static_capability_grants_remote_access() {
        for (path, capability) in capabilities() {
            assert!(capability["remote"].is_null(), "{}", path.display());
        }
    }

    #[test]
    fn bundled_pages_can_invoke_shell_commands() {
        for cmd in SHELL_COMMANDS {
            for page in ["error.html", "update.html"] {
                let url = format!("tauri://localhost/{page}");
                assert_eq!(invoke(cmd, &url), Ok("ran".into()), "{cmd} from {page}");
            }
        }
    }

    #[test]
    fn daemon_origin_cannot_invoke_shell_commands() {
        for cmd in SHELL_COMMANDS {
            for url in [
                "http://127.0.0.1:51234/",
                "http://127.0.0.1:51234/preview/site/",
            ] {
                assert!(invoke(cmd, url).is_err(), "{cmd} from {url}");
            }
        }
    }

    #[test]
    fn no_webview_can_drive_the_updater_plugin() {
        // The plugin is registered in the mock app, so only the ACL can refuse.
        for cmd in [
            "plugin:updater|check",
            "plugin:updater|download_and_install",
            "plugin:updater|install",
        ] {
            for url in ["tauri://localhost/update.html", "http://127.0.0.1:51234/"] {
                let err = invoke(cmd, url).expect_err(cmd);
                let message = err.as_str().unwrap_or_default();
                assert!(message.contains("not allowed"), "{cmd} from {url}: {err}");
            }
        }
    }

    #[test]
    fn no_capability_grants_updater_permissions() {
        for (path, capability) in capabilities() {
            let permissions = capability["permissions"].as_array().expect("permissions");
            for permission in permissions {
                let id = permission
                    .as_str()
                    .or_else(|| permission["identifier"].as_str())
                    .unwrap_or_default();
                assert!(
                    !id.starts_with("updater:"),
                    "{} grants {id}",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn secondary_windows_cannot_invoke_shell_commands() {
        let label = crate::links::secondary_label(1);
        for cmd in SHELL_COMMANDS {
            for url in [
                "http://127.0.0.1:51234/preview/site/",
                "tauri://localhost/error.html",
                "tauri://localhost/update.html",
            ] {
                assert!(invoke_from(&label, cmd, url).is_err(), "{cmd} from {url}");
            }
        }
    }

    #[test]
    fn commands_outside_the_manifest_are_rejected() {
        assert!(invoke("unlisted", "tauri://localhost/error.html").is_err());
    }
}

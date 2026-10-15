mod commands;
pub mod logs;
pub mod supervisor;

use tauri::{Manager, RunEvent};

use supervisor::Supervisor;

pub fn run() {
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
        .manage(Supervisor::default())
        .invoke_handler(tauri::generate_handler![
            commands::collect_logs,
            commands::retry_daemon
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                match window.url() {
                    Ok(url) => logs::shell(&format!("main window showing {url}")),
                    Err(e) => logs::shell(&format!("main window URL unavailable: {e}")),
                }
            }
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                if let Err(e) = Supervisor::start(&handle) {
                    logs::shell(&format!("daemon unavailable: {e}"));
                }
            });
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
    fn unlisted() -> &'static str {
        "ran"
    }

    fn invoke(cmd: &str, url: &str) -> Result<String, serde_json::Value> {
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![collect_logs, retry_daemon, unlisted])
            .build(tauri::generate_context!(test = true))
            .expect("mock app");
        let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
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
        .map(|body| body.deserialize::<String>().expect("string response"))
    }

    #[test]
    fn bundled_pages_can_invoke_shell_commands() {
        for cmd in ["collect_logs", "retry_daemon"] {
            assert_eq!(invoke(cmd, "tauri://localhost/error.html"), Ok("ran".into()), "{cmd}");
        }
    }

    #[test]
    fn daemon_origin_cannot_invoke_shell_commands() {
        for cmd in ["collect_logs", "retry_daemon"] {
            assert!(invoke(cmd, "http://127.0.0.1:51234/").is_err(), "{cmd}");
        }
    }

    #[test]
    fn commands_outside_the_manifest_are_rejected() {
        assert!(invoke("unlisted", "tauri://localhost/error.html").is_err());
    }
}

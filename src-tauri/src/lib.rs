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
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                if let Err(e) = Supervisor::start(&handle) {
                    eprintln!("workbench-desktop: daemon unavailable: {e}");
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

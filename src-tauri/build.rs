fn main() {
    // Declaring the app's commands puts them under the ACL even for local
    // pages; `capabilities/` grants them to the bundled pages only.
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&["collect_logs", "retry_daemon"]),
    ))
    .expect("failed to run tauri-build");
}

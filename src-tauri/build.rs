fn main() {
    // The web UI is served by the daemon (a remote origin to Tauri), so an app command must be
    // granted by a capability: declaring it here generates the `allow-capture-region` permission
    // that `capabilities/default.json` gives the daemon's origin.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(&["capture_region"])),
    )
    .expect("tauri build");
}

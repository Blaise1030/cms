//! Durable first-run setup dismissal.
//!
//! The desktop UI is served from an ephemeral `http://127.0.0.1:<port>` origin, so
//! `localStorage` does not survive relaunch. Store the dismiss flag next to managed
//! runtimes under the per-user orso data directory.

use std::path::{Path, PathBuf};

use crate::config::default_runtime_dir;

/// Marker file written when the user skips or finishes onboarding.
const DISMISS_FILE: &str = "setup-dismissed";

/// Directory that holds app-wide prefs (parent of `runtimes/`).
pub fn prefs_dir() -> PathBuf {
    default_runtime_dir()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(default_runtime_dir)
}

fn dismiss_path(dir: &Path) -> PathBuf {
    dir.join(DISMISS_FILE)
}

/// Whether onboarding was dismissed on this machine.
pub fn is_dismissed(dir: &Path) -> bool {
    dismiss_path(dir).is_file()
}

/// Persist dismissal so the wizard stays hidden across daemon restarts.
pub fn dismiss(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dismiss_path(dir), b"1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn dismiss_persists_across_reads() {
        let dir = tempdir().unwrap();
        assert!(!is_dismissed(dir.path()));
        dismiss(dir.path()).unwrap();
        assert!(is_dismissed(dir.path()));
    }
}

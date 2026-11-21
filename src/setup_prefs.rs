//! Durable app-wide setup preferences: the first-run dismissal and the assistant order.
//!
//! The desktop UI is served from an ephemeral `http://127.0.0.1:<port>` origin, so
//! `localStorage` does not survive relaunch. Store these next to managed runtimes
//! under the per-user orso data directory.

use std::path::{Path, PathBuf};

use crate::config::default_runtime_dir;

/// Marker file written when the user skips or finishes onboarding.
const DISMISS_FILE: &str = "setup-dismissed";
/// The user's assistant order (App settings → Assistants), as a JSON array of ids.
const ASSISTANT_ORDER_FILE: &str = "assistant-order.json";

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

/// The saved assistant order, first choice first; `None` until the user reorders
/// (or when the file is unreadable, which reads as "no order").
pub fn assistant_order(dir: &Path) -> Option<Vec<String>> {
    let bytes = std::fs::read(dir.join(ASSISTANT_ORDER_FILE)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Persist the assistant order. Written to a temp file and renamed, so a crash
/// never leaves half an order behind.
pub fn save_assistant_order(dir: &Path, order: &[String]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(ASSISTANT_ORDER_FILE);
    let tmp = dir.join(format!("{ASSISTANT_ORDER_FILE}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec(order)?)?;
    std::fs::rename(tmp, path)
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

    #[test]
    fn assistant_order_round_trips() {
        let dir = tempdir().unwrap();
        assert_eq!(assistant_order(dir.path()), None);
        let order = vec!["claude".to_string(), "opencode".to_string()];
        save_assistant_order(dir.path(), &order).unwrap();
        assert_eq!(assistant_order(dir.path()), Some(order));
    }

    #[test]
    fn unreadable_assistant_order_reads_as_none() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(ASSISTANT_ORDER_FILE), b"{not json").unwrap();
        assert_eq!(assistant_order(dir.path()), None);
    }
}

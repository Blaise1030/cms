//! Main-window close must quit the process so `RunEvent::Exit` runs and the
//! supervised daemon is stopped. On macOS, closing the traffic-light otherwise
//! leaves the Dock process (and sidecar) alive.

/// What the main window's event handler should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainWindowAction {
    /// Sync traffic-light / fullscreen chrome after a resize.
    SyncFullscreen,
    /// Quit the whole app so Exit → `Supervisor::shutdown` runs.
    QuitApp,
    /// No lifecycle action.
    Ignore,
}

/// Classifies a main-window event for lifecycle purposes.
///
/// `close_requested` is true for `WindowEvent::CloseRequested`. Secondary
/// preview windows are not wired through this helper.
pub fn main_window_action(resized: bool, close_requested: bool) -> MainWindowAction {
    if close_requested {
        MainWindowAction::QuitApp
    } else if resized {
        MainWindowAction::SyncFullscreen
    } else {
        MainWindowAction::Ignore
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_the_main_window_quits_the_app() {
        assert_eq!(
            main_window_action(false, true),
            MainWindowAction::QuitApp
        );
    }

    #[test]
    fn resize_only_syncs_fullscreen() {
        assert_eq!(
            main_window_action(true, false),
            MainWindowAction::SyncFullscreen
        );
    }

    #[test]
    fn close_requested_wins_over_resized() {
        assert_eq!(
            main_window_action(true, true),
            MainWindowAction::QuitApp
        );
    }

    #[test]
    fn other_events_are_ignored() {
        assert_eq!(main_window_action(false, false), MainWindowAction::Ignore);
    }
}

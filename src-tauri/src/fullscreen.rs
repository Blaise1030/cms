//! Marks `<html data-fullscreen>` while the main window is full screen. The traffic lights are
//! hidden there, so the room pages keep for them (`web/src/lib/desktop.ts`) can go.

use tauri::WebviewWindow;

use crate::logs;

pub fn script(fullscreen: bool) -> String {
    format!("document.documentElement.toggleAttribute(\"data-fullscreen\", {fullscreen});")
}

/// Called after each page load and on every resize (entering or leaving full screen is one).
pub fn sync(window: &WebviewWindow) {
    let on = window.is_fullscreen().unwrap_or(false);
    if let Err(e) = window.eval(script(on)) {
        logs::shell(&format!("could not mark full screen: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_sets_and_clears_the_marker() {
        assert_eq!(
            script(true),
            "document.documentElement.toggleAttribute(\"data-fullscreen\", true);"
        );
        assert_eq!(
            script(false),
            "document.documentElement.toggleAttribute(\"data-fullscreen\", false);"
        );
    }
}

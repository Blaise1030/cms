//! The `POST /__orso` endpoint the orso Astro integration mounts on a project's dev server.

/// Where to reach a project's running integration. Deliberately has no `Debug` derive:
/// `token` authenticates the daemon to the dev server.
#[derive(Clone)]
pub struct ContentEndpoint {
    pub url: String,
    pub token: String,
}

pub const PREVIEW_REASON: &str = "Start the preview to edit content";
pub const PREVIEW_SAVE_REASON: &str = "Preview is not running; start the preview to save content";

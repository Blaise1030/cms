//! Built, immutable previews of the three sides of a conflict.
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use serde::Serialize;
use tokio::sync::Mutex;

use crate::error::ApiError;
use crate::snapshot_build::{self, BuildError};
pub(crate) use crate::snapshot_build::{link_node_modules, BuildGroup};

/// How long one side's preview build may run.
const BUILD_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Source {
    Yours,
    Latest,
    Result,
}
impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Yours => "yours",
            Self::Latest => "latest",
            Self::Result => "result",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "yours" => Some(Self::Yours),
            "latest" => Some(Self::Latest),
            "result" => Some(Self::Result),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum PreviewState {
    Ready { url: String },
    Unavailable { reason: String },
    Invalid { errors: String },
}

struct Snapshot {
    revision: String,
    state: PreviewState,
    /// The built site; no checkout is kept once the build ends.
    dir: Option<PathBuf>,
}
#[derive(Clone, Default)]
pub struct ConflictPreviewManager {
    entries: Arc<Mutex<HashMap<(String, Source), Snapshot>>>,
}

impl ConflictPreviewManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn prepare(
        &self,
        project_dir: &Path,
        project_id: &str,
        session_id: &str,
        source: Source,
        revision: &str,
    ) -> PreviewState {
        let key = (session_id.to_string(), source);
        let mut entries = self.entries.lock().await;
        if let Some(old) = entries.get(&key) {
            if old.revision == revision {
                return old.state.clone();
            }
        }
        if let Some(old) = entries.remove(&key) {
            remove_snapshot(old).await;
        }
        let url = format!(
            "/preview/{project_id}/conflicts/{session_id}/{}/",
            source.as_str()
        );
        let path = std::env::temp_dir()
            .join("orso-conflict-previews")
            .join(format!("{:016x}", rand::random::<u64>()));
        let state =
            match snapshot_build::build(project_dir, revision, &url, &path, BUILD_TIMEOUT).await {
                Ok(()) => PreviewState::Ready { url },
                Err(BuildError::Unavailable(reason)) => PreviewState::Unavailable { reason },
                Err(BuildError::Invalid(errors)) => PreviewState::Invalid { errors },
            };
        entries.insert(
            key,
            Snapshot {
                revision: revision.to_owned(),
                state: state.clone(),
                dir: path.exists().then_some(path),
            },
        );
        state
    }

    pub async fn remove_session(&self, id: &str) {
        let mut entries = self.entries.lock().await;
        for source in [Source::Yours, Source::Latest, Source::Result] {
            if let Some(old) = entries.remove(&(id.to_owned(), source)) {
                remove_snapshot(old).await;
            }
        }
    }

    pub async fn remove_source(&self, id: &str, source: Source) {
        if let Some(old) = self.entries.lock().await.remove(&(id.to_owned(), source)) {
            remove_snapshot(old).await;
        }
    }

    pub async fn prune(&self, active: &[String]) {
        let active: HashSet<&str> = active.iter().map(String::as_str).collect();
        let mut entries = self.entries.lock().await;
        let stale: Vec<_> = entries
            .keys()
            .filter(|(id, _)| !active.contains(id.as_str()))
            .cloned()
            .collect();
        for key in stale {
            if let Some(old) = entries.remove(&key) {
                remove_snapshot(old).await;
            }
        }
    }

    pub async fn shutdown(&self) {
        let mut entries = self.entries.lock().await;
        for (_, old) in entries.drain() {
            remove_snapshot(old).await;
        }
    }

    pub async fn is_invalid(&self, id: &str, source: Source) -> bool {
        self.entries
            .lock()
            .await
            .get(&(id.to_owned(), source))
            .is_some_and(|s| matches!(s.state, PreviewState::Invalid { .. }))
    }

    pub async fn serve(
        &self,
        session: &str,
        source: Source,
        rest: &str,
        section: Option<&str>,
    ) -> Result<Response, ApiError> {
        let entries = self.entries.lock().await;
        let snapshot = entries
            .get(&(session.to_owned(), source))
            .ok_or_else(|| ApiError::enoent("preview not prepared"))?;
        if !matches!(snapshot.state, PreviewState::Ready { .. }) {
            return Err(ApiError::enoent("preview unavailable"));
        }
        let dir = snapshot
            .dir
            .as_ref()
            .ok_or_else(|| ApiError::enoent("preview unavailable"))?;
        let safe = safe_route(rest).ok_or_else(|| ApiError::badreq("invalid preview path"))?;
        let mut file = dir.join(safe);
        if file.is_dir() {
            file = file.join("index.html");
        }
        if !file.exists() && file.extension().is_none() {
            file = file.join("index.html");
        }
        let mut bytes = tokio::fs::read(&file)
            .await
            .map_err(|_| ApiError::enoent("preview page not found"))?;
        if file.extension().is_some_and(|e| e == "html") {
            if let Some(id) = section.filter(|s| {
                !s.is_empty()
                    && s.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            }) {
                let html = String::from_utf8_lossy(&bytes);
                if html.matches(&format!("id=\"{id}\"")).count() == 1 {
                    let style = format!(
                        // A band as wide as the preview, so the changed section reads edge to edge
                        // even when the site insets it with margins.
                        r#"<style data-conflict-highlight>html{{overflow-x:clip!important}}section[id="{id}"]{{position:relative!important}}section[id="{id}"]::after{{content:"";position:absolute;top:-4px;bottom:-4px;left:calc(50% - 50vw);width:100vw;box-sizing:border-box;border:3px solid #f59e0b;pointer-events:none;z-index:2147483647}}</style>"#
                    );
                    bytes = html
                        .replace("</head>", &format!("{style}</head>"))
                        .into_bytes();
                    if !String::from_utf8_lossy(&bytes).contains(&style) {
                        bytes.extend_from_slice(style.as_bytes());
                    }
                }
            }
        }
        let content_type = match file.extension().and_then(|s| s.to_str()).unwrap_or("") {
            "html" => "text/html; charset=utf-8",
            "js" | "mjs" => "text/javascript; charset=utf-8",
            "css" => "text/css; charset=utf-8",
            "svg" => "image/svg+xml",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "woff2" => "font/woff2",
            "json" => "application/json",
            _ => "application/octet-stream",
        };
        let mut response = Response::new(Body::from(bytes));
        *response.status_mut() = StatusCode::OK;
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        Ok(response)
    }
}

fn safe_route(rest: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for segment in rest.split('/') {
        if segment.is_empty() {
            continue;
        }
        if segment == "." || segment == ".." || segment.contains('\\') || segment.contains('%') {
            return None;
        }
        path.push(segment);
    }
    Some(path)
}

async fn remove_snapshot(old: Snapshot) {
    if let Some(dir) = old.dir {
        let _ = tokio::fs::remove_dir_all(dir).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_traversal() {
        assert!(safe_route("../secret").is_none());
        assert!(safe_route("a/%2e%2e/b").is_none());
        assert_eq!(safe_route("page/").unwrap(), PathBuf::from("page"));
    }
}

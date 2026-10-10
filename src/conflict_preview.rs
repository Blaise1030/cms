//! Built, immutable previews of the three sides of a conflict.
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use serde::Serialize;
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::changes::conflicts;
use crate::error::ApiError;

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
    dir: Option<PathBuf>,
    repo: PathBuf,
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
        let path = std::env::temp_dir().join(format!("wb-preview-{:016x}", rand::random::<u64>()));
        let state = match build_snapshot(project_dir, &path, revision, &url).await {
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
                repo: project_dir.to_owned(),
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
        let mut file = dir.join("dist").join(safe);
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
                let script = format!(
                    r#"<script>addEventListener('load',()=>{{let m=document.querySelectorAll('section[id="{id}"]');if(m.length!==1)return;let e=m[0];e.style.outline='3px solid #f59e0b';e.style.outlineOffset='4px';e.scrollIntoView({{block:'center'}})}})</script>"#
                );
                bytes = html
                    .replace("</body>", &format!("{script}</body>"))
                    .into_bytes();
                if !String::from_utf8_lossy(&bytes).contains(&script) {
                    bytes.extend_from_slice(script.as_bytes());
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

enum BuildError {
    Unavailable(String),
    Invalid(String),
}
async fn build_snapshot(
    repo: &Path,
    dir: &Path,
    revision: &str,
    url: &str,
) -> Result<(), BuildError> {
    if !repo.join("package.json").is_file() {
        return Err(BuildError::Unavailable(
            "This project has no preview build command".into(),
        ));
    }
    let output = Command::new("git")
        .args(["worktree", "add", "--detach", "--quiet"])
        .arg(dir)
        .arg(revision)
        .current_dir(repo)
        .output()
        .await
        .map_err(|e| BuildError::Unavailable(e.to_string()))?;
    if !output.status.success() {
        return Err(BuildError::Unavailable(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    #[cfg(unix)]
    if repo.join("node_modules").exists() {
        tokio::fs::symlink(repo.join("node_modules"), dir.join("node_modules"))
            .await
            .map_err(|e| BuildError::Unavailable(e.to_string()))?;
    }
    let mut build = Command::new("npm");
    build
        .args(["run", "build"])
        .env("PUBLIC_BASE", url)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .process_group(0);
    let child = build
        .spawn()
        .map_err(|e| BuildError::Unavailable(e.to_string()))?;
    let group = BuildGroup(child.id().map(|pid| pid as i32));
    let output = tokio::time::timeout(Duration::from_secs(90), child.wait_with_output())
        .await
        .map_err(|_| BuildError::Unavailable("Preview build timed out".into()))?
        .map_err(|e| BuildError::Unavailable(e.to_string()))?;
    drop(group);
    if !output.status.success() {
        let output_text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let message: String = output_text
            .chars()
            .rev()
            .take(1800)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if message.contains("Missing script: \"build\"")
            || message.contains("command not found")
            || message.contains("not found: astro")
        {
            return Err(BuildError::Unavailable(
                "This project does not have a working preview build command".into(),
            ));
        }
        return Err(BuildError::Invalid(message));
    }
    if !dir.join("dist").is_dir() {
        return Err(BuildError::Unavailable(
            "Preview build produced no dist directory".into(),
        ));
    }
    Ok(())
}

struct BuildGroup(Option<i32>);
impl Drop for BuildGroup {
    fn drop(&mut self) {
        if let Some(group) = self.0 {
            // SAFETY: the child was spawned in its own process group.
            unsafe {
                libc::killpg(group, libc::SIGKILL);
            }
        }
    }
}

async fn remove_snapshot(old: Snapshot) {
    if let Some(dir) = old.dir {
        let _ = conflicts::git(
            &old.repo,
            &["worktree", "remove", "--force", &dir.to_string_lossy()],
            &[],
            None,
        )
        .await;
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

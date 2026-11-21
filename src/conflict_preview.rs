//! Built, immutable previews of the three sides of a conflict.
//!
//! Each built side is served at the root of an origin of its own (see [`crate::preview`]),
//! so it builds with base `/` like the live site and its root-relative links resolve. The
//! app frames these cross-origin (and without scripts), so it cannot read the frame's
//! document: it asks [`ConflictPreviewManager::has_page`] whether a page exists instead.
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Router;
use serde::Serialize;
use tokio::sync::Mutex;

use crate::config::Config;
use crate::error::ApiError;
use crate::preview::OriginServer;
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
    /// Served at the root of `http://<daemon host>:{port}/`; `token` is that origin's
    /// first-load token (absent without auth).
    Ready {
        port: u16,
        token: Option<String>,
    },
    Unavailable {
        reason: String,
    },
    Invalid {
        errors: String,
    },
}

struct Snapshot {
    revision: String,
    state: PreviewState,
    /// The built site; no checkout is kept once the build ends.
    dir: Option<PathBuf>,
    /// The origin serving `dir`; closed with the snapshot, which frees its port.
    origin: Option<OriginServer>,
}
#[derive(Clone)]
pub struct ConflictPreviewManager {
    entries: Arc<Mutex<HashMap<(String, Source), Snapshot>>>,
    /// Where built sides live; owned by this manager (swept at startup).
    base: PathBuf,
    /// Auth and Host rules for the snapshot origins, and the app origins their pages report
    /// to. The default accepts only each origin's own token.
    config: Arc<Config>,
    /// Ports to stay off when opening an origin (see [`Self::with_projects`]).
    projects: Option<Arc<crate::projects::ProjectRegistry>>,
}

impl Default for ConflictPreviewManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConflictPreviewManager {
    /// A manager building under a fresh folder of its own in the system temp dir.
    pub fn new() -> Self {
        Self::with_base(std::env::temp_dir().join(format!(
            "orso-conflict-previews-{:016x}",
            rand::random::<u64>()
        )))
    }

    /// A manager building under `base` (the daemon's state folder).
    pub fn with_base(base: PathBuf) -> Self {
        Self {
            entries: Arc::default(),
            base,
            config: Arc::new(Config::default()),
            projects: None,
        }
    }

    /// Serve snapshots under the daemon's auth and Host rules.
    /// Keep conflict-preview origins off the ports projects have claimed.
    pub fn with_projects(mut self, projects: Arc<crate::projects::ProjectRegistry>) -> Self {
        self.projects = Some(projects);
        self
    }

    pub fn with_config(mut self, config: Arc<Config>) -> Self {
        self.config = config;
        self
    }

    /// At daemon start, when no conflict session is live yet: removes every built side a
    /// previous run left behind, plus legacy temp-dir builds, then prunes the checkouts'
    /// stale worktrees.
    pub async fn startup(&self, repos: &[PathBuf]) {
        startup_sweep(&self.base, &std::env::temp_dir(), repos).await;
    }

    pub async fn prepare(
        &self,
        project_dir: &Path,
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
        let path = self.base.join(format!("{:016x}", rand::random::<u64>()));
        let mut origin = None;
        let state = match snapshot_build::build(project_dir, revision, &path, BUILD_TIMEOUT).await {
            Ok(()) => match self.open_origin(&path).await {
                Ok(server) => {
                    let access = server.access();
                    origin = Some(server);
                    PreviewState::Ready {
                        port: access.port,
                        token: access.token,
                    }
                }
                Err(e) => PreviewState::Unavailable {
                    reason: format!("Couldn't open a preview port: {e}"),
                },
            },
            Err(BuildError::Unavailable(reason)) => PreviewState::Unavailable { reason },
            Err(BuildError::Invalid(errors)) => PreviewState::Invalid { errors },
        };
        entries.insert(
            key,
            Snapshot {
                revision: revision.to_owned(),
                state: state.clone(),
                dir: path.exists().then_some(path),
                origin,
            },
        );
        state
    }

    /// An origin serving the built site at `dir` from its root.
    async fn open_origin(&self, dir: &Path) -> std::io::Result<OriginServer> {
        let site = Arc::new(SnapshotSite {
            dir: dir.to_owned(),
        });
        let app = Router::new().fallback(serve_snapshot).with_state(site);
        let avoid = match &self.projects {
            Some(projects) => projects.claimed_ports().await,
            None => Default::default(),
        };
        OriginServer::bind(self.config.clone(), app, &avoid).await
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
        serve_dir(dir, rest, section).await
    }

    /// Whether the built side has a page at `route` (what its origin would answer `200`).
    pub async fn has_page(&self, session: &str, source: Source, route: &str) -> bool {
        let dir = {
            let entries = self.entries.lock().await;
            match entries.get(&(session.to_owned(), source)) {
                Some(Snapshot {
                    state: PreviewState::Ready { .. },
                    dir: Some(dir),
                    ..
                }) => dir.clone(),
                _ => return false,
            }
        };
        read_page(&dir, route.trim_start_matches('/')).await.is_ok()
    }
}

/// What one snapshot origin serves.
struct SnapshotSite {
    dir: PathBuf,
}

/// Router fallback of a snapshot origin: every path is a file of the built site.
async fn serve_snapshot(State(site): State<Arc<SnapshotSite>>, req: Request) -> Response {
    let rest = req.uri().path().trim_start_matches('/').to_owned();
    let section = req
        .uri()
        .query()
        .and_then(|q| q.split('&').find_map(|p| p.strip_prefix("section=")))
        .map(str::to_owned);
    match serve_dir(&site.dir, &rest, section.as_deref()).await {
        Ok(res) => res,
        Err(e) if e.code == crate::error::ErrorCode::Enoent && is_page_route(&rest) => {
            missing_page()
        }
        Err(e) => e.into_response(),
    }
}

/// A route that names a page rather than an asset (a folder, an extensionless path or HTML).
fn is_page_route(rest: &str) -> bool {
    let last = rest.rsplit('/').next().unwrap_or("");
    !last.contains('.') || last.ends_with(".html")
}

/// A readable `404` page for a route the built site does not have.
fn missing_page() -> Response {
    let html = r#"<!doctype html><html><head><meta charset="utf-8"></head><body>This page is missing from the preview.</body></html>"#;
    let mut res = page_response(Path::new("missing.html"), html.as_bytes().to_vec());
    *res.status_mut() = StatusCode::NOT_FOUND;
    res
}

/// One file of the built site at `dir`; HTML pages get the conflict highlight for `section`.
async fn serve_dir(dir: &Path, rest: &str, section: Option<&str>) -> Result<Response, ApiError> {
    let (file, mut bytes) = read_page(dir, rest).await?;
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
                bytes = inject_head(html.into_owned().into_bytes(), &style);
            }
        }
    }
    Ok(page_response(&file, bytes))
}

/// `html` with `snippet` before its first `</head>`, or at the end when it has none.
fn inject_head(html: Vec<u8>, snippet: &str) -> Vec<u8> {
    let text = String::from_utf8_lossy(&html);
    match text.find("</head>") {
        Some(at) => {
            let mut out = String::with_capacity(text.len() + snippet.len());
            out.push_str(&text[..at]);
            out.push_str(snippet);
            out.push_str(&text[at..]);
            out.into_bytes()
        }
        None => {
            let mut out = html.clone();
            out.extend_from_slice(snippet.as_bytes());
            out
        }
    }
}

/// Reads the file a preview route names inside the built site `dir`: a folder or an
/// extensionless route means its `index.html`; `..` and encoded segments are refused.
pub(crate) async fn read_page(dir: &Path, rest: &str) -> Result<(PathBuf, Vec<u8>), ApiError> {
    let safe = safe_route(rest).ok_or_else(|| ApiError::badreq("invalid preview path"))?;
    let mut file = dir.join(safe);
    if file.is_dir() {
        file = file.join("index.html");
    }
    if !file.exists() && file.extension().is_none() {
        file = file.join("index.html");
    }
    let bytes = tokio::fs::read(&file)
        .await
        .map_err(|_| ApiError::enoent("preview page not found"))?;
    Ok((file, bytes))
}

/// An uncached `200` carrying `bytes`, typed by `file`'s extension.
pub(crate) fn page_response(file: &Path, bytes: Vec<u8>) -> Response {
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
    response
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

/// Removes `base` (this daemon's conflict previews), `tmp/orso-conflict-previews` and
/// `tmp/wb-preview-*` (where older versions built), then runs `git worktree prune` in each
/// of `repos` so worktrees whose folders went with them are forgotten.
pub async fn startup_sweep(base: &Path, tmp: &Path, repos: &[PathBuf]) {
    let _ = tokio::fs::remove_dir_all(base).await;
    let _ = tokio::fs::remove_dir_all(tmp.join("orso-conflict-previews")).await;
    if let Ok(mut entries) = tokio::fs::read_dir(tmp).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let legacy = entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.starts_with("wb-preview-"));
            if legacy && entry.file_type().await.is_ok_and(|t| t.is_dir()) {
                let _ = tokio::fs::remove_dir_all(entry.path()).await;
            }
        }
    }
    for repo in repos {
        let _ = tokio::process::Command::new("git")
            .args(["-c", "core.hooksPath=/dev/null", "worktree", "prune"])
            .current_dir(repo)
            .stdin(std::process::Stdio::null())
            .output()
            .await;
    }
}

async fn remove_snapshot(old: Snapshot) {
    if let Some(origin) = old.origin {
        origin.close().await;
    }
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

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    #[tokio::test]
    async fn startup_sweep_removes_leftovers_and_prunes_worktrees() {
        let temp = tempfile::tempdir().unwrap();
        let tmp = temp.path().join("tmp");
        let base = temp.path().join("state/conflict-previews");
        let repo = temp.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["commit", "-q", "--allow-empty", "-m", "base"]);
        // A worktree a crashed build left under the legacy temp base.
        let stale = tmp.join("orso-conflict-previews/.build-1");
        git(
            &repo,
            &[
                "worktree",
                "add",
                "--detach",
                "-q",
                stale.to_str().unwrap(),
                "HEAD",
            ],
        );
        std::fs::create_dir_all(base.join("0123/assets")).unwrap();
        std::fs::create_dir_all(tmp.join("wb-preview-abc/dist")).unwrap();
        std::fs::create_dir_all(tmp.join("unrelated")).unwrap();
        std::fs::write(tmp.join("wb-preview-file"), "not a dir").unwrap();

        startup_sweep(&base, &tmp, std::slice::from_ref(&repo)).await;

        assert!(!base.exists());
        assert!(!tmp.join("orso-conflict-previews").exists());
        assert!(!tmp.join("wb-preview-abc").exists());
        assert!(tmp.join("unrelated").is_dir(), "swept an unrelated folder");
        assert!(tmp.join("wb-preview-file").is_file(), "swept a file");
        let worktrees = git(&repo, &["worktree", "list", "--porcelain"]);
        assert_eq!(worktrees.matches("worktree ").count(), 1, "{worktrees}");
    }
}

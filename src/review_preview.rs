//! Built previews of teammates' changes for the Review inbox.
//!
//! Kept small on disk: one build per `(project, PR number)` holding a single head
//! SHA, at most [`MAX_PER_PROJECT`] per project (least recently viewed evicted),
//! none older than [`MAX_AGE`], and none for a PR that is no longer open.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use axum::response::Response;
use futures_util::future::BoxFuture;
use serde::Serialize;
use tokio::task::JoinHandle;

use crate::conflict_preview;
use crate::error::ApiError;
use crate::snapshot_build::{self, BuildError};

/// Previews kept per project; the least recently viewed goes first.
pub const MAX_PER_PROJECT: usize = 3;
/// A preview older than this is dropped.
pub const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ReviewPreview {
    Building,
    Ready { url: String },
    Failed { message: String },
}

/// Builds `(repo, sha, base_url, out)`; `snapshot_build::build` outside tests.
pub type Builder = Arc<
    dyn Fn(PathBuf, String, String, PathBuf) -> BoxFuture<'static, Result<(), BuildError>>
        + Send
        + Sync,
>;

/// How long one review preview build may run.
const BUILD_TIMEOUT: Duration = Duration::from_secs(90);

struct Entry {
    sha: String,
    state: ReviewPreview,
    /// The built site, under the project's folder; absent until a build succeeds.
    dir: PathBuf,
    created: SystemTime,
    /// Tick of the last prepare or page view; the lowest is evicted first.
    viewed: u64,
    /// Tick that started this entry's build; a finished build whose entry was
    /// replaced or dropped meanwhile deletes its own output.
    generation: u64,
}

#[derive(Default)]
struct State {
    entries: HashMap<(String, u64), Entry>,
    /// Each project's checkout, for `git worktree prune` at shutdown.
    repos: HashMap<String, PathBuf>,
    builds: Vec<JoinHandle<()>>,
    tick: u64,
}

impl State {
    fn next_tick(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    /// Drops every entry older than [`MAX_AGE`], returning their folders.
    fn expire(&mut self, now: SystemTime) -> Vec<PathBuf> {
        self.remove_where(|_, entry| {
            now.duration_since(entry.created)
                .is_ok_and(|age| age > MAX_AGE)
        })
    }

    /// Drops `project`'s least recently viewed entries beyond [`MAX_PER_PROJECT`].
    fn evict(&mut self, project: &str) -> Vec<PathBuf> {
        let mut views: Vec<u64> = self
            .entries
            .iter()
            .filter(|((p, _), _)| p == project)
            .map(|(_, e)| e.viewed)
            .collect();
        if views.len() <= MAX_PER_PROJECT {
            return Vec::new();
        }
        views.sort_unstable_by(|a, b| b.cmp(a));
        let oldest_kept = views[MAX_PER_PROJECT - 1];
        self.remove_where(|(p, _), e| p == project && e.viewed < oldest_kept)
    }

    fn remove_where(&mut self, doomed: impl Fn(&(String, u64), &Entry) -> bool) -> Vec<PathBuf> {
        let keys: Vec<_> = self
            .entries
            .iter()
            .filter(|(key, entry)| doomed(key, entry))
            .map(|(key, _)| key.clone())
            .collect();
        keys.into_iter()
            .filter_map(|key| self.entries.remove(&key))
            .map(|entry| entry.dir)
            .collect()
    }
}

/// Built review previews under `base/{project}/`, one folder per project so
/// sweeping one project's leftovers never touches another's builds.
pub struct ReviewPreviews {
    base: PathBuf,
    clock: fn() -> SystemTime,
    builder: Builder,
    state: Arc<Mutex<State>>,
}

impl ReviewPreviews {
    pub fn new(base: PathBuf, clock: fn() -> SystemTime) -> Self {
        // One spelling of the path (macOS `/var` is `/private/var`) for every
        // folder this manager names and every directory listing it compares.
        let _ = std::fs::create_dir_all(&base);
        let base = std::fs::canonicalize(&base).unwrap_or(base);
        Self {
            base,
            clock,
            builder: Arc::new(|repo, sha, base_url, out| {
                Box::pin(async move {
                    snapshot_build::build(&repo, &sha, &base_url, &out, BUILD_TIMEOUT).await
                })
            }),
            state: Arc::default(),
        }
    }

    /// Replaces the site build (tests build without npm).
    pub fn with_builder(mut self, builder: Builder) -> Self {
        self.builder = builder;
        self
    }

    /// Starts building `sha` of PR `number`, or reports the build already there.
    ///
    /// The same `(project, number, sha)` joins the existing build (a failed one is
    /// retried); a new head replaces and deletes the previous build. Then the
    /// per-project and age limits apply.
    pub async fn prepare(
        &self,
        project: &str,
        repo: &Path,
        number: u64,
        sha: &str,
    ) -> ReviewPreview {
        if !valid_segment(project) {
            return ReviewPreview::Failed {
                message: "Invalid project".into(),
            };
        }
        if !valid_sha(sha) {
            return ReviewPreview::Failed {
                message: "Invalid head commit".into(),
            };
        }
        let now = (self.clock)();
        let key = (project.to_owned(), number);
        let (doomed, current) = {
            let mut state = self.state.lock().unwrap();
            let mut doomed = state.expire(now);
            let tick = state.next_tick();
            let joined = state
                .entries
                .get_mut(&key)
                .filter(|entry| {
                    entry.sha == sha && !matches!(entry.state, ReviewPreview::Failed { .. })
                })
                .map(|entry| {
                    entry.viewed = tick;
                    entry.state.clone()
                });
            if let Some(current) = joined {
                (doomed, current)
            } else {
                doomed.extend(state.entries.remove(&key).map(|old| old.dir));
                let dir = self.base.join(project).join(format!("{number}-{tick}"));
                state.entries.insert(
                    key.clone(),
                    Entry {
                        sha: sha.to_owned(),
                        state: ReviewPreview::Building,
                        dir: dir.clone(),
                        created: now,
                        viewed: tick,
                        generation: tick,
                    },
                );
                state.repos.insert(project.to_owned(), repo.to_owned());
                doomed.extend(state.evict(project));
                let url = format!("/preview/{project}/reviews/{number}/{sha}/");
                let build =
                    (self.builder)(repo.to_owned(), sha.to_owned(), url.clone(), dir.clone());
                let shared = self.state.clone();
                let handle = tokio::spawn(async move {
                    let result = build.await;
                    let orphaned = {
                        let mut state = shared.lock().unwrap();
                        match state.entries.get_mut(&key) {
                            Some(entry) if entry.generation == tick => {
                                entry.state = match result {
                                    Ok(()) => ReviewPreview::Ready { url },
                                    Err(BuildError::Unavailable(message))
                                    | Err(BuildError::Invalid(message)) => {
                                        ReviewPreview::Failed { message }
                                    }
                                };
                                !matches!(entry.state, ReviewPreview::Ready { .. })
                            }
                            _ => true,
                        }
                    };
                    if orphaned {
                        let _ = tokio::fs::remove_dir_all(&dir).await;
                    }
                });
                state.builds.retain(|build| !build.is_finished());
                state.builds.push(handle);
                (doomed, ReviewPreview::Building)
            }
        };
        remove_dirs(doomed).await;
        current
    }

    /// The preview of `sha` for PR `number`, if one is building or built.
    pub async fn status(&self, project: &str, number: u64, sha: &str) -> Option<ReviewPreview> {
        let (doomed, current) = {
            let mut state = self.state.lock().unwrap();
            let doomed = state.expire((self.clock)());
            let current = state
                .entries
                .get(&(project.to_owned(), number))
                .filter(|entry| entry.sha == sha)
                .map(|entry| entry.state.clone());
            (doomed, current)
        };
        remove_dirs(doomed).await;
        current
    }

    /// Serves a page of a ready preview and marks it viewed.
    pub async fn serve(
        &self,
        project: &str,
        number: u64,
        sha: &str,
        rest: &str,
    ) -> Result<Response, ApiError> {
        let (doomed, dir) = {
            let mut state = self.state.lock().unwrap();
            let doomed = state.expire((self.clock)());
            let tick = state.next_tick();
            let dir = state
                .entries
                .get_mut(&(project.to_owned(), number))
                .filter(|entry| {
                    entry.sha == sha && matches!(entry.state, ReviewPreview::Ready { .. })
                })
                .map(|entry| {
                    entry.viewed = tick;
                    entry.dir.clone()
                });
            (doomed, dir)
        };
        remove_dirs(doomed).await;
        let dir = dir.ok_or_else(|| ApiError::enoent("preview not prepared"))?;
        let (file, bytes) = conflict_preview::read_page(&dir, rest).await?;
        Ok(conflict_preview::page_response(&file, bytes))
    }

    /// Drops `project`'s previews of PRs that are no longer open.
    pub async fn prune(&self, project: &str, open: &[u64]) {
        let doomed = {
            let mut state = self.state.lock().unwrap();
            let mut doomed = state.expire((self.clock)());
            doomed.extend(
                state.remove_where(|(p, number), _| p == project && !open.contains(number)),
            );
            doomed
        };
        remove_dirs(doomed).await;
    }

    /// At daemon start, before any build: removes every folder a previous run
    /// left behind (`projects` is each project's `(id, checkout)`), then prunes
    /// the checkouts' stale worktrees.
    pub async fn startup(&self, projects: &[(String, PathBuf)]) {
        if let Ok(mut entries) = tokio::fs::read_dir(&self.base).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name();
                if !projects.iter().any(|(id, _)| name == id.as_str()) {
                    let _ = tokio::fs::remove_dir_all(entry.path()).await;
                }
            }
        }
        for (id, repo) in projects {
            if valid_segment(id) {
                snapshot_build::sweep(repo, &self.base.join(id), &[]).await;
            }
        }
    }

    /// Stops running builds and removes every preview and leftover folder.
    pub async fn shutdown(&self) {
        let (builds, repos, doomed) = {
            let mut state = self.state.lock().unwrap();
            let doomed: Vec<PathBuf> = state.entries.drain().map(|(_, e)| e.dir).collect();
            (
                std::mem::take(&mut state.builds),
                std::mem::take(&mut state.repos),
                doomed,
            )
        };
        // Cancelling a build removes its worktree; only then is a sweep safe.
        for build in &builds {
            build.abort();
        }
        for build in builds {
            let _ = build.await;
        }
        remove_dirs(doomed).await;
        for (project, repo) in repos {
            snapshot_build::sweep(&repo, &self.base.join(project), &[]).await;
        }
    }
}

async fn remove_dirs(dirs: Vec<PathBuf>) {
    for dir in dirs {
        let _ = tokio::fs::remove_dir_all(dir).await;
    }
}

/// One plain path segment: never empty, `.`, `..`, or containing a separator.
fn valid_segment(s: &str) -> bool {
    !s.is_empty() && s != "." && s != ".." && !s.contains(['/', '\\'])
}

/// A commit id (never an option or ref expression handed to git).
fn valid_sha(s: &str) -> bool {
    (1..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::UNIX_EPOCH;
    use tokio::sync::Semaphore;

    thread_local! {
        static NOW: Cell<u64> = const { Cell::new(1_000_000) };
    }
    fn clock() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(NOW.with(Cell::get))
    }
    fn advance(by: Duration) {
        NOW.with(|now| now.set(now.get() + by.as_secs()));
    }

    #[derive(Default)]
    struct Fake {
        calls: AtomicUsize,
        fail: AtomicBool,
        gate: Option<Arc<Semaphore>>,
    }

    fn fake(fake: Arc<Fake>) -> Builder {
        Arc::new(move |_repo, sha, base_url, out| {
            let fake = fake.clone();
            Box::pin(async move {
                fake.calls.fetch_add(1, Ordering::SeqCst);
                if let Some(gate) = &fake.gate {
                    gate.acquire().await.unwrap().forget();
                }
                if fake.fail.load(Ordering::SeqCst) {
                    return Err(BuildError::Invalid(format!("broken page in {sha}")));
                }
                tokio::fs::create_dir_all(&out).await.unwrap();
                tokio::fs::write(out.join("index.html"), format!("{sha} at {base_url}"))
                    .await
                    .unwrap();
                Ok(())
            })
        })
    }

    fn manager(base: &tempfile::TempDir, f: &Arc<Fake>) -> ReviewPreviews {
        ReviewPreviews::new(base.path().to_owned(), clock).with_builder(fake(f.clone()))
    }

    async fn settle(m: &ReviewPreviews, number: u64, sha: &str) -> ReviewPreview {
        for _ in 0..400 {
            match m.status("p", number, sha).await {
                Some(ReviewPreview::Building) => tokio::time::sleep(Duration::from_millis(5)).await,
                Some(other) => return other,
                None => panic!("no preview for #{number} {sha}"),
            }
        }
        panic!("build for #{number} never finished");
    }

    async fn ready(m: &ReviewPreviews, number: u64, sha: &str) {
        m.prepare("p", Path::new("/repo"), number, sha).await;
        assert!(matches!(
            settle(m, number, sha).await,
            ReviewPreview::Ready { .. }
        ));
    }

    /// Built output dirs currently on disk for project `p`.
    fn built_dirs(base: &tempfile::TempDir) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(base.path().join("p"))
            .map(|rd| {
                rd.flatten()
                    .map(|e| std::fs::read_to_string(e.path().join("index.html")).unwrap())
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out
    }

    #[tokio::test]
    async fn prepare_twice_builds_once() {
        let base = tempfile::tempdir().unwrap();
        let gate = Arc::new(Semaphore::new(0));
        let f = Arc::new(Fake {
            gate: Some(gate.clone()),
            ..Fake::default()
        });
        let m = manager(&base, &f);
        let repo = Path::new("/repo");
        assert_eq!(
            m.prepare("p", repo, 7, "abc").await,
            ReviewPreview::Building
        );
        assert_eq!(
            m.prepare("p", repo, 7, "abc").await,
            ReviewPreview::Building
        );
        gate.add_permits(2);
        assert_eq!(
            settle(&m, 7, "abc").await,
            ReviewPreview::Ready {
                url: "/preview/p/reviews/7/abc/".into()
            }
        );
        assert_eq!(
            m.prepare("p", repo, 7, "abc").await,
            settle(&m, 7, "abc").await
        );
        assert_eq!(f.calls.load(Ordering::SeqCst), 1);
        assert_eq!(built_dirs(&base), ["abc at /preview/p/reviews/7/abc/"]);
    }

    #[tokio::test]
    async fn new_head_replaces_old_build() {
        let base = tempfile::tempdir().unwrap();
        let f = Arc::new(Fake::default());
        let m = manager(&base, &f);
        ready(&m, 7, "aaa").await;
        ready(&m, 7, "bbb").await;
        assert_eq!(m.status("p", 7, "aaa").await, None);
        assert_eq!(built_dirs(&base), ["bbb at /preview/p/reviews/7/bbb/"]);
    }

    #[tokio::test]
    async fn fourth_build_evicts_least_recently_viewed() {
        let base = tempfile::tempdir().unwrap();
        let f = Arc::new(Fake::default());
        let m = manager(&base, &f);
        ready(&m, 1, "a1").await;
        ready(&m, 2, "a2").await;
        ready(&m, 3, "a3").await;
        // #1 is the oldest build but was just looked at; #2 is now the stalest view.
        m.serve("p", 1, "a1", "").await.unwrap();
        ready(&m, 4, "a4").await;
        assert_eq!(m.status("p", 2, "a2").await, None);
        for (n, sha) in [(1, "a1"), (3, "a3"), (4, "a4")] {
            assert!(m.status("p", n, sha).await.is_some(), "#{n} evicted");
        }
        assert_eq!(built_dirs(&base).len(), MAX_PER_PROJECT);
        assert!(!built_dirs(&base).iter().any(|d| d.starts_with("a2 ")));
    }

    #[tokio::test]
    async fn builds_older_than_a_day_are_dropped() {
        let base = tempfile::tempdir().unwrap();
        let f = Arc::new(Fake::default());
        let m = manager(&base, &f);
        ready(&m, 1, "0dd").await;
        advance(MAX_AGE - Duration::from_secs(60));
        assert!(m.status("p", 1, "0dd").await.is_some());
        assert!(m.serve("p", 1, "0dd", "").await.is_ok());
        advance(Duration::from_secs(120));
        assert_eq!(m.status("p", 1, "0dd").await, None);
        assert!(m.serve("p", 1, "0dd", "").await.is_err());
        assert!(built_dirs(&base).is_empty());
    }

    #[tokio::test]
    async fn prune_drops_closed_prs() {
        let base = tempfile::tempdir().unwrap();
        let f = Arc::new(Fake::default());
        let m = manager(&base, &f);
        ready(&m, 1, "a1").await;
        ready(&m, 2, "a2").await;
        m.prune("p", &[2, 5]).await;
        assert_eq!(m.status("p", 1, "a1").await, None);
        assert!(m.status("p", 2, "a2").await.is_some());
        assert_eq!(built_dirs(&base), ["a2 at /preview/p/reviews/2/a2/"]);
    }

    #[tokio::test]
    async fn failed_build_reports_message_and_can_retry() {
        let base = tempfile::tempdir().unwrap();
        let f = Arc::new(Fake::default());
        f.fail.store(true, Ordering::SeqCst);
        let m = manager(&base, &f);
        m.prepare("p", Path::new("/repo"), 7, "abc").await;
        assert_eq!(
            settle(&m, 7, "abc").await,
            ReviewPreview::Failed {
                message: "broken page in abc".into()
            }
        );
        f.fail.store(false, Ordering::SeqCst);
        assert_eq!(
            m.prepare("p", Path::new("/repo"), 7, "abc").await,
            ReviewPreview::Building
        );
        assert!(matches!(
            settle(&m, 7, "abc").await,
            ReviewPreview::Ready { .. }
        ));
        assert_eq!(f.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn serve_returns_the_built_page() {
        let base = tempfile::tempdir().unwrap();
        let f = Arc::new(Fake::default());
        let m = manager(&base, &f);
        ready(&m, 7, "abc").await;
        let response = m.serve("p", 7, "abc", "").await.unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], b"abc at /preview/p/reviews/7/abc/");
        assert!(m.serve("p", 7, "abc", "../x").await.is_err());
        assert!(m.serve("p", 7, "other", "").await.is_err());
    }

    #[tokio::test]
    async fn shutdown_removes_every_build() {
        let base = tempfile::tempdir().unwrap();
        let f = Arc::new(Fake::default());
        let m = manager(&base, &f);
        ready(&m, 7, "abc").await;
        m.shutdown().await;
        assert!(built_dirs(&base).is_empty());
        assert_eq!(m.status("p", 7, "abc").await, None);
    }

    #[tokio::test]
    async fn startup_removes_leftovers_of_every_project() {
        let base = tempfile::tempdir().unwrap();
        let f = Arc::new(Fake::default());
        let m = manager(&base, &f);
        std::fs::create_dir_all(base.path().join("p/7-3")).unwrap();
        std::fs::create_dir_all(base.path().join("p/.build-0123")).unwrap();
        std::fs::create_dir_all(base.path().join("deleted-project/1-1")).unwrap();
        m.startup(&[("p".into(), PathBuf::from("/repo"))]).await;
        let left: Vec<_> = walk(base.path());
        assert_eq!(left, [base.path().join("p").canonicalize().unwrap()]);
    }

    fn walk(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            out.push(entry.path().canonicalize().unwrap());
            if entry.path().is_dir() {
                out.extend(walk(&entry.path()));
            }
        }
        out
    }

    #[test]
    fn state_serializes_with_a_state_tag() {
        assert_eq!(
            serde_json::to_value(ReviewPreview::Building).unwrap(),
            serde_json::json!({ "state": "building" })
        );
        assert_eq!(
            serde_json::to_value(ReviewPreview::Failed {
                message: "m".into()
            })
            .unwrap(),
            serde_json::json!({ "state": "failed", "message": "m" })
        );
    }
}

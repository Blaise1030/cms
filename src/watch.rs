//! Per-project filesystem watcher → `fs.change` events.
//!
//! A `notify` watcher runs for each project's lifetime (started on create / daemon resume,
//! stopped on delete / shutdown). Raw OS events are coalesced over a short window and
//! emitted as `{ projectId, path, kind }` with `path` project-relative and `/`-separated.
//! VCS and build-output directories are ignored: they churn constantly (dev server, git
//! checkpoints) and are hidden from the file tree anyway.

use std::collections::{BTreeMap, HashMap};
use std::path::{Component, Path};
use std::sync::Mutex;
use std::time::Duration;

use notify::{recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::json;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::events::EventBus;

/// Raw events are batched for this long before being emitted.
const COALESCE: Duration = Duration::from_millis(100);
/// Above this many distinct paths in a batch, emit one project-wide event instead.
const MAX_PER_BATCH: usize = 200;

const IGNORED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "dist",
    ".astro",
    ".vite",
    ".wrangler",
    ".cache",
];

struct Running {
    /// Dropping the watcher closes the raw-event channel and ends the task.
    _watcher: RecommendedWatcher,
    task: JoinHandle<()>,
}

pub struct WatchManager {
    events: EventBus,
    running: Mutex<HashMap<String, Running>>,
}

impl WatchManager {
    pub fn new(events: EventBus) -> Self {
        Self {
            events,
            running: Mutex::new(HashMap::new()),
        }
    }

    /// (Re)start watching `dir` for project `id`. Failures are returned, not fatal: the
    /// UI still works (it just will not auto-refresh the file tree).
    pub fn start(&self, id: &str, dir: &Path) -> notify::Result<()> {
        // FSEvents reports canonical paths (/var → /private/var); strip against the same.
        let root = dir.canonicalize().map_err(notify::Error::io)?;
        let (raw_tx, raw_rx) = mpsc::unbounded_channel::<Event>();
        let mut watcher = recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(ev) = res {
                let _ = raw_tx.send(ev);
            }
        })?;
        watcher.watch(&root, RecursiveMode::Recursive)?;
        let task = tokio::spawn(pump(self.events.clone(), id.to_string(), root, raw_rx));
        let old = self
            .running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                id.to_string(),
                Running {
                    _watcher: watcher,
                    task,
                },
            );
        if let Some(old) = old {
            old.task.abort();
        }
        Ok(())
    }

    pub fn stop(&self, id: &str) {
        let old = self
            .running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id);
        if let Some(old) = old {
            old.task.abort();
        }
    }

    pub fn shutdown(&self) {
        let all: Vec<Running> = self
            .running
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .drain()
            .map(|(_, r)| r)
            .collect();
        for r in all {
            r.task.abort();
        }
    }
}

async fn pump(
    events: EventBus,
    id: String,
    root: std::path::PathBuf,
    mut rx: mpsc::UnboundedReceiver<Event>,
) {
    while let Some(first) = rx.recv().await {
        let mut batch: BTreeMap<String, &'static str> = BTreeMap::new();
        absorb(&root, &first, &mut batch);
        let deadline = tokio::time::sleep(COALESCE);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                _ = &mut deadline => break,
                next = rx.recv() => match next {
                    Some(ev) => absorb(&root, &ev, &mut batch),
                    None => break,
                },
            }
        }
        if batch.len() > MAX_PER_BATCH {
            events.emit(
                "fs.change",
                json!({ "projectId": id, "path": "", "kind": "modify" }),
            );
            continue;
        }
        for (path, mut kind) in batch {
            // FSEvents merges flags (a file created then removed reports both), so the
            // reported kind can be stale: what is on disk now is the truth.
            if std::fs::symlink_metadata(root.join(&path)).is_err() {
                kind = "remove";
            } else if kind == "remove" {
                kind = "modify";
            }
            events.emit(
                "fs.change",
                json!({ "projectId": id, "path": path, "kind": kind }),
            );
        }
    }
}

fn absorb(root: &Path, ev: &Event, batch: &mut BTreeMap<String, &'static str>) {
    let Some(kind) = kind_of(&ev.kind) else {
        return;
    };
    for p in &ev.paths {
        if let Some(rel) = relative(root, p) {
            batch.insert(rel, kind);
        }
    }
}

fn kind_of(kind: &EventKind) -> Option<&'static str> {
    match kind {
        EventKind::Create(_) => Some("create"),
        EventKind::Remove(_) => Some("remove"),
        EventKind::Modify(_) => Some("modify"),
        // Access / Any / Other carry no content change.
        _ => None,
    }
}

/// Project-relative `/`-separated path, or `None` for the root itself, paths outside it,
/// and anything under an ignored directory.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for c in rel.components() {
        match c {
            Component::Normal(s) => {
                let s = s.to_str()?;
                if IGNORED_DIRS.contains(&s) {
                    return None;
                }
                parts.push(s);
            }
            _ => return None,
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_and_ignores() {
        let root = Path::new("/p/site");
        assert_eq!(
            relative(root, Path::new("/p/site/src/pages/index.astro")).as_deref(),
            Some("src/pages/index.astro")
        );
        assert_eq!(relative(root, root), None);
        assert_eq!(relative(root, Path::new("/elsewhere/x")), None);
        for ignored in [
            "/p/site/.git/index",
            "/p/site/node_modules/a/b.js",
            "/p/site/dist/index.html",
            "/p/site/src/.astro/types.d.ts",
        ] {
            assert_eq!(relative(root, Path::new(ignored)), None, "{ignored}");
        }
    }

    #[test]
    fn access_events_are_not_changes() {
        assert_eq!(
            kind_of(&EventKind::Access(notify::event::AccessKind::Any)),
            None
        );
        assert_eq!(
            kind_of(&EventKind::Create(notify::event::CreateKind::File)),
            Some("create")
        );
    }
}

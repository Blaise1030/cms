//! Project registry: id → directory under `--projects-dir`.
//!
//! `create` copies the embedded template, runs `npm install`, then `git init` + initial
//! commit. `delete` removes the project directory (the HTTP layer guards it with an
//! explicit confirm flag). Dev-server supervision is a later task: for now a project
//! gets an unused port and `devserver: "starting"`.

use std::collections::HashSet;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::error::ApiError;
use crate::static_files;

/// Longest accepted project name (chars).
pub const MAX_NAME_LEN: usize = 64;
/// Longest generated id slug.
const MAX_SLUG_LEN: usize = 48;
/// Upper bound for `npm install` at scaffold time.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(600);
/// Daemon state dir (inside projects-dir; slugs never start with `.`, so no collisions).
const STATE_DIR: &str = ".workbench";
const REGISTRY_FILE: &str = "projects.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DevserverStatus {
    Starting,
    Ready,
    Crashed,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub dir: PathBuf,
    pub port: u16,
    pub devserver: DevserverStatus,
}

/// API view of a project: `{ id, name, port, devserver }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    pub port: u16,
    pub devserver: DevserverStatus,
}

impl From<&Project> for ProjectInfo {
    fn from(p: &Project) -> Self {
        Self {
            id: p.id.clone(),
            name: p.name.clone(),
            port: p.port,
            devserver: p.devserver,
        }
    }
}

/// On-disk record (`{projects-dir}/.workbench/projects.json`). Dir is derived from the id;
/// devserver status is runtime-only.
#[derive(Debug, Serialize, Deserialize)]
struct StoredProject {
    id: String,
    name: String,
    port: u16,
}

#[derive(Default)]
struct Inner {
    projects: Vec<Project>,
    /// Ids / ports claimed by in-flight `create` calls.
    reserved_ids: HashSet<String>,
    reserved_ports: HashSet<u16>,
}

pub struct ProjectRegistry {
    projects_dir: PathBuf,
    /// Run `npm install` during `create`. Tests turn this off via
    /// [`crate::state::AppState::with_scaffold_install`]; the daemon always runs it.
    install: bool,
    inner: Mutex<Inner>,
}

impl ProjectRegistry {
    /// Open the registry, loading any persisted projects whose directory still exists.
    pub fn open(projects_dir: PathBuf, install: bool) -> Self {
        let projects = load_stored(&projects_dir)
            .into_iter()
            .filter(|s| is_valid_id(&s.id))
            .map(|s| Project {
                dir: projects_dir.join(&s.id),
                id: s.id,
                name: s.name,
                port: s.port,
                devserver: DevserverStatus::Starting,
            })
            .filter(|p| p.dir.is_dir())
            .collect();
        Self {
            projects_dir,
            install,
            inner: Mutex::new(Inner {
                projects,
                ..Inner::default()
            }),
        }
    }

    pub async fn list(&self) -> Vec<ProjectInfo> {
        self.inner
            .lock()
            .await
            .projects
            .iter()
            .map(ProjectInfo::from)
            .collect()
    }

    pub async fn get(&self, id: &str) -> Option<Project> {
        self.inner
            .lock()
            .await
            .projects
            .iter()
            .find(|p| p.id == id)
            .cloned()
    }

    /// Scaffold a new project from the embedded template.
    pub async fn create(&self, name: &str) -> Result<ProjectInfo, ApiError> {
        let name = validate_name(name)?;
        let slug = slugify(&name);
        if slug.is_empty() {
            return Err(ApiError::badreq(
                "project name must contain at least one letter or digit",
            ));
        }

        tokio::fs::create_dir_all(&self.projects_dir).await?;

        // Reserve id + port so concurrent creates never collide.
        let (id, port) = {
            let mut inner = self.inner.lock().await;
            let id = (1..)
                .map(|n| {
                    if n == 1 {
                        slug.clone()
                    } else {
                        format!("{slug}-{n}")
                    }
                })
                .find(|c| {
                    !inner.projects.iter().any(|p| &p.id == c)
                        && !inner.reserved_ids.contains(c)
                        && !self.projects_dir.join(c).exists()
                })
                .expect("infinite iterator");
            let port = allocate_port(&inner)?;
            inner.reserved_ids.insert(id.clone());
            inner.reserved_ports.insert(port);
            (id, port)
        };

        let dir = self.projects_dir.join(&id);
        let built = self.scaffold(&dir).await;

        let mut inner = self.inner.lock().await;
        inner.reserved_ids.remove(&id);
        inner.reserved_ports.remove(&port);
        if let Err(e) = built {
            drop(inner);
            let _ = tokio::fs::remove_dir_all(&dir).await;
            return Err(e);
        }
        let project = Project {
            id,
            name,
            dir,
            port,
            devserver: DevserverStatus::Starting,
        };
        let info = ProjectInfo::from(&project);
        inner.projects.push(project);
        if let Err(e) = self.persist(&inner.projects).await {
            let dir = inner.projects.pop().map(|p| p.dir);
            drop(inner);
            if let Some(dir) = dir {
                let _ = tokio::fs::remove_dir_all(dir).await;
            }
            return Err(e);
        }
        Ok(info)
    }

    /// Remove the project and its directory. The caller is responsible for confirmation.
    pub async fn delete(&self, id: &str) -> Result<(), ApiError> {
        let mut inner = self.inner.lock().await;
        let idx = inner
            .projects
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| ApiError::enoent(format!("no such project: {id}")))?;
        let project = inner.projects.remove(idx);

        let result = async {
            self.ensure_inside_projects_dir(&project.dir).await?;
            match tokio::fs::remove_dir_all(&project.dir).await {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(ApiError::from(e)),
            }
            // Daemon-side state (history etc.) lives outside the repo.
            let state = self.projects_dir.join(STATE_DIR).join(&project.id);
            match tokio::fs::remove_dir_all(&state).await {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(ApiError::from(e)),
            }
            self.persist(&inner.projects).await
        }
        .await;

        if let Err(e) = result {
            // Keep the registry truthful if anything survived.
            if project.dir.exists() {
                inner.projects.insert(idx, project);
            }
            return Err(e);
        }
        Ok(())
    }

    /// Defense in depth: the directory must be a direct child of projects-dir, not a symlink.
    async fn ensure_inside_projects_dir(&self, dir: &Path) -> Result<(), ApiError> {
        let meta = match tokio::fs::symlink_metadata(dir).await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        if meta.file_type().is_symlink() {
            return Err(ApiError::sandbox(
                "refusing to delete a symlinked project dir",
            ));
        }
        let root = tokio::fs::canonicalize(&self.projects_dir).await?;
        let canon = tokio::fs::canonicalize(dir).await?;
        if canon.parent() != Some(root.as_path()) {
            return Err(ApiError::sandbox("project dir is outside projects-dir"));
        }
        Ok(())
    }

    async fn scaffold(&self, dir: &Path) -> Result<(), ApiError> {
        // `create_dir` (not `_all`) fails if something raced us into existence.
        tokio::fs::create_dir(dir).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                ApiError::conflict("project directory already exists")
            } else {
                e.into()
            }
        })?;

        let dest = dir.to_path_buf();
        tokio::task::spawn_blocking(move || static_files::extract_template(&dest))
            .await
            .map_err(|e| ApiError::io(format!("template copy task failed: {e}")))??;

        if self.install {
            npm_install(dir).await?;
        }

        git(dir, &["init", "-b", "main"]).await?;
        git(dir, &["add", "-A"]).await?;
        git(
            dir,
            &[
                "commit",
                "-m",
                "Initial commit from astro-marketing template",
            ],
        )
        .await?;
        Ok(())
    }

    async fn persist(&self, projects: &[Project]) -> Result<(), ApiError> {
        let stored: Vec<StoredProject> = projects
            .iter()
            .map(|p| StoredProject {
                id: p.id.clone(),
                name: p.name.clone(),
                port: p.port,
            })
            .collect();
        let state_dir = self.projects_dir.join(STATE_DIR);
        tokio::fs::create_dir_all(&state_dir).await?;
        let tmp = state_dir.join(format!("{REGISTRY_FILE}.tmp"));
        let json = serde_json::to_vec_pretty(&stored)
            .map_err(|e| ApiError::io(format!("serialize registry: {e}")))?;
        tokio::fs::write(&tmp, json).await?;
        tokio::fs::rename(&tmp, state_dir.join(REGISTRY_FILE)).await?;
        Ok(())
    }
}

fn load_stored(projects_dir: &Path) -> Vec<StoredProject> {
    let path = projects_dir.join(STATE_DIR).join(REGISTRY_FILE);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => {
            // Never silently overwrite a registry we could not read.
            let backup = path.with_extension("json.corrupt");
            eprintln!(
                "workbench: unreadable project registry ({e}); moved to {}",
                backup.display()
            );
            let _ = std::fs::rename(&path, backup);
            Vec::new()
        }
    }
}

/// Trim and validate a user-supplied project name.
pub fn validate_name(name: &str) -> Result<String, ApiError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ApiError::badreq("project name is required"));
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(ApiError::badreq(format!(
            "project name must be at most {MAX_NAME_LEN} characters"
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(ApiError::badreq(
            "project name must not contain control characters",
        ));
    }
    Ok(name.to_string())
}

/// Lowercase ASCII slug: runs of non-alphanumerics become a single `-`.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.truncate(MAX_SLUG_LEN);
    out.trim_end_matches('-').to_string()
}

/// Ids we would ever generate: `[a-z0-9-]+`, no leading `-`/`.`.
fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Ask the OS for an unused loopback port that no project has claimed.
fn allocate_port(inner: &Inner) -> Result<u16, ApiError> {
    for _ in 0..32 {
        let port = TcpListener::bind(("127.0.0.1", 0))
            .and_then(|l| l.local_addr())
            .map(|a| a.port())
            .map_err(|e| ApiError::io(format!("could not allocate a port: {e}")))?;
        if !inner.projects.iter().any(|p| p.port == port) && !inner.reserved_ports.contains(&port) {
            return Ok(port);
        }
    }
    Err(ApiError::io("could not allocate an unused port"))
}

async fn npm_install(dir: &Path) -> Result<(), ApiError> {
    let child = Command::new("npm")
        .args(["install", "--no-audit", "--no-fund", "--no-progress"])
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(INSTALL_TIMEOUT, child)
        .await
        .map_err(|_| ApiError::io("npm install timed out"))?
        .map_err(|e| ApiError::io(format!("failed to run npm install: {e}")))?;
    if !out.status.success() {
        return Err(ApiError::io(format!(
            "npm install failed ({}): {}",
            out.status,
            tail(&String::from_utf8_lossy(&out.stderr))
        )));
    }
    Ok(())
}

/// Run git in `dir` with a fixed identity so scaffolding works without global git config.
async fn git(dir: &Path, args: &[&str]) -> Result<(), ApiError> {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=Workbench",
            "-c",
            "user.email=workbench@localhost",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| ApiError::git(format!("failed to run git: {e}")))?;
    if !out.status.success() {
        return Err(ApiError::git(format!(
            "git {} failed ({}): {}",
            args.first().copied().unwrap_or(""),
            out.status,
            tail(&String::from_utf8_lossy(&out.stderr))
        )));
    }
    Ok(())
}

/// Last few hundred chars of command output, for error messages.
fn tail(s: &str) -> String {
    let s = s.trim();
    let start = s.char_indices().rev().nth(399).map_or(0, |(i, _)| i);
    s[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_cases() {
        assert_eq!(slugify("My Cool Site!"), "my-cool-site");
        assert_eq!(slugify("  --a__b--  "), "a-b");
        assert_eq!(slugify("../../etc"), "etc");
        assert_eq!(slugify("日本語"), "");
        assert!(slugify(&"a".repeat(200)).len() <= MAX_SLUG_LEN);
    }

    #[test]
    fn name_validation() {
        assert_eq!(validate_name("  Hi  ").unwrap(), "Hi");
        assert!(validate_name("   ").is_err());
        assert!(validate_name("a\nb").is_err());
        assert!(validate_name(&"x".repeat(MAX_NAME_LEN + 1)).is_err());
    }

    #[test]
    fn id_validity() {
        assert!(is_valid_id("my-site-2"));
        for bad in ["", "-x", ".workbench", "a/b", "A", "a b", ".."] {
            assert!(!is_valid_id(bad), "{bad}");
        }
    }
}

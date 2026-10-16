//! In-memory conflict sessions; the branch remains untouched until Finish.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::Mutex;

use super::conflicts::{self, Candidate, Choice, ConflictFile};
use crate::conflict_preview::{ConflictPreviewManager, PreviewState, Source};
use crate::error::ApiError;
use crate::git::{self, Identity};

const TTL_SECONDS: u64 = 60 * 60;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictSession {
    pub id: String,
    pub project_id: String,
    pub branch: String,
    pub tip: String,
    pub incoming: String,
    pub base: String,
    pub files: Vec<ConflictFile>,
    pub expires_at: u64,
    pub choices: BTreeMap<String, Choice>,
    #[serde(skip)]
    pub dir: PathBuf,
    #[serde(skip)]
    pub merged_tree: String,
}

#[derive(Default)]
pub struct ConflictSessions {
    entries: Arc<Mutex<HashMap<String, ConflictSession>>>,
}

impl ConflictSessions {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn active_ids(&self) -> Vec<String> {
        let mut entries = self.entries.lock().await;
        entries.retain(|_, s| s.expires_at > Self::now());
        entries.keys().cloned().collect()
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    pub async fn start(
        &self,
        dir: &Path,
        project_id: &str,
        branch: &str,
        incoming: &str,
    ) -> Result<ConflictSession, ApiError> {
        if !branch.starts_with("content/")
            || !incoming.bytes().all(|b| b.is_ascii_hexdigit())
            || incoming.len() != 40
        {
            return Err(ApiError::badreq("invalid branch or incoming commit"));
        }
        if self
            .list(project_id)
            .await
            .iter()
            .any(|s| s.branch == branch)
        {
            return Err(ApiError::conflict(
                "Finish or abandon the open conflict before running Get latest again",
            ));
        }
        let tip = conflicts::text(
            dir,
            &["rev-parse", "--verify", &format!("refs/heads/{branch}")],
        )
        .await?;
        let base = conflicts::text(dir, &["merge-base", &tip, incoming]).await?;
        let (merged_tree, files) = conflicts::discover(dir, &tip, incoming).await?;
        let session = ConflictSession {
            id: format!(
                "{:016x}{:016x}",
                rand::random::<u64>(),
                rand::random::<u64>()
            ),
            project_id: project_id.to_string(),
            branch: branch.to_string(),
            tip,
            incoming: incoming.to_string(),
            base,
            files,
            expires_at: Self::now() + TTL_SECONDS,
            choices: BTreeMap::new(),
            dir: dir.to_path_buf(),
            merged_tree,
        };
        let mut entries = self.entries.lock().await;
        entries.retain(|_, s| s.expires_at > Self::now());
        if entries
            .values()
            .any(|s| s.project_id == project_id && s.branch == branch)
        {
            return Err(ApiError::conflict(
                "Finish or abandon the open conflict before running Get latest again",
            ));
        }
        entries.insert(session.id.clone(), session.clone());
        Ok(session)
    }

    pub async fn get(&self, id: &str) -> Result<ConflictSession, ApiError> {
        let mut entries = self.entries.lock().await;
        entries.retain(|_, s| s.expires_at > Self::now());
        entries
            .get(id)
            .cloned()
            .ok_or_else(|| ApiError::enoent("conflict session expired or not found"))
    }

    pub async fn list(&self, project_id: &str) -> Vec<ConflictSession> {
        let mut entries = self.entries.lock().await;
        entries.retain(|_, s| s.expires_at > Self::now());
        entries
            .values()
            .filter(|s| s.project_id == project_id)
            .cloned()
            .collect()
    }

    pub async fn choose(
        &self,
        id: &str,
        path: &str,
        choice: Choice,
    ) -> Result<Candidate, ApiError> {
        let mut session = self.get(id).await?;
        let lock = git::lock_for(&session.dir);
        let _guard = lock.lock().await;
        session = self.get(id).await?;
        let file = session
            .files
            .iter()
            .find(|f| f.path == path)
            .ok_or_else(|| ApiError::badreq("file is not in this conflict"))?;
        if (file.binary || file.ours_deleted || file.theirs_deleted) && choice == Choice::Both {
            return Err(ApiError::badreq(
                "Keep both is unavailable when a side is deleted or binary",
            ));
        }
        session.choices.insert(path.to_string(), choice);
        let candidate = conflicts::candidate_tree(
            &session.dir,
            &session.merged_tree,
            &session.files,
            &session.choices,
        )
        .await?;
        self.entries.lock().await.insert(id.to_string(), session);
        Ok(candidate)
    }

    pub async fn finish(&self, id: &str, author: &Identity) -> Result<String, ApiError> {
        self.finish_inner(id, author, None).await
    }

    pub async fn finish_validated(
        &self,
        id: &str,
        author: &Identity,
        previews: &ConflictPreviewManager,
    ) -> Result<String, ApiError> {
        self.finish_inner(id, author, Some(previews)).await
    }

    async fn finish_inner(
        &self,
        id: &str,
        author: &Identity,
        previews: Option<&ConflictPreviewManager>,
    ) -> Result<String, ApiError> {
        let session = self.get(id).await?;
        let lock = git::lock_for(&session.dir);
        let _guard = lock.lock().await;
        let session = self.get(id).await?;
        conflicts::ensure_tip(&session.dir, &session.branch, &session.tip).await?;
        let candidate = conflicts::candidate_tree(
            &session.dir,
            &session.merged_tree,
            &session.files,
            &session.choices,
        )
        .await?;
        if !candidate.complete {
            return Err(ApiError::conflict("Choose a resolution for every file"));
        }
        if let Some(previews) = previews {
            let preview_commit =
                conflicts::preview_commit(&session.dir, &candidate.tree, &session.tip).await?;
            match previews
                .prepare(
                    &session.dir,
                    &session.project_id,
                    id,
                    Source::Result,
                    &preview_commit,
                )
                .await
            {
                PreviewState::Invalid { errors } => {
                    return Err(ApiError::conflict(format!(
                        "The proposed result has a build error: {errors}"
                    )));
                }
                PreviewState::Ready { .. } | PreviewState::Unavailable { .. } => {}
            }
        }
        let commit = conflicts::merge_commit(
            &session.dir,
            &candidate.tree,
            &session.tip,
            &session.incoming,
            author,
            "Get latest from main",
        )
        .await?;
        conflicts::advance_branch(&session.dir, &session.branch, &session.tip, &commit).await?;
        self.entries.lock().await.remove(id);
        Ok(commit)
    }

    pub async fn abandon(&self, id: &str) -> Result<(), ApiError> {
        self.get(id).await?;
        self.entries.lock().await.remove(id);
        Ok(())
    }
}

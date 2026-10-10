//! Git plumbing for an immutable, checkout-free conflict candidate.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::error::ApiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Choice {
    Yours,
    Latest,
    Both,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictFile {
    pub path: String,
    pub binary: bool,
    pub ours_oid: String,
    pub theirs_oid: String,
    pub base_oid: Option<String>,
    #[serde(skip)]
    pub mode: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub tree: String,
    pub complete: bool,
}

pub(crate) async fn git(
    dir: &Path,
    args: &[&str],
    env: &[(&str, &str)],
    input: Option<&[u8]>,
) -> Result<(i32, Vec<u8>, Vec<u8>), ApiError> {
    use tokio::io::AsyncWriteExt;
    let mut cmd = Command::new("git");
    cmd.args([
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "core.quotepath=false",
    ])
    .args(args)
    .current_dir(dir)
    .stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    for (key, value) in env {
        cmd.env(key, value);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| ApiError::git(format!("failed to run git: {e}")))?;
    if let Some(bytes) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(bytes)
            .await
            .map_err(ApiError::from)?;
    }
    let out = child.wait_with_output().await.map_err(ApiError::from)?;
    Ok((out.status.code().unwrap_or(-1), out.stdout, out.stderr))
}

pub(crate) async fn checked(
    dir: &Path,
    args: &[&str],
    env: &[(&str, &str)],
    input: Option<&[u8]>,
) -> Result<Vec<u8>, ApiError> {
    let (status, stdout, stderr) = git(dir, args, env, input).await?;
    if status != 0 {
        return Err(ApiError::git(format!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&stderr).trim()
        )));
    }
    Ok(stdout)
}

pub(crate) async fn text(dir: &Path, args: &[&str]) -> Result<String, ApiError> {
    Ok(
        String::from_utf8_lossy(&checked(dir, args, &[], None).await?)
            .trim()
            .to_string(),
    )
}

pub(crate) async fn blob(dir: &Path, oid: &str) -> Result<Vec<u8>, ApiError> {
    checked(dir, &["cat-file", "blob", oid], &[], None).await
}

pub(crate) async fn discover(
    dir: &Path,
    tip: &str,
    incoming: &str,
) -> Result<(String, Vec<ConflictFile>), ApiError> {
    let (status, stdout, stderr) = git(
        dir,
        &["merge-tree", "--write-tree", tip, incoming],
        &[],
        None,
    )
    .await?;
    if status != 1 {
        return Err(if status == 0 {
            ApiError::badreq("these commits do not conflict")
        } else {
            ApiError::git(format!(
                "git merge-tree failed: {}",
                String::from_utf8_lossy(&stderr)
            ))
        });
    }
    let output = String::from_utf8(stdout)
        .map_err(|_| ApiError::git("merge-tree returned non-UTF-8 metadata"))?;
    let mut lines = output.lines();
    let tree = lines
        .next()
        .ok_or_else(|| ApiError::git("merge-tree returned no tree"))?
        .to_string();
    let mut stages: BTreeMap<String, [Option<(String, String)>; 3]> = BTreeMap::new();
    for line in lines {
        if line.is_empty() {
            break;
        }
        let (meta, path) = line
            .split_once('\t')
            .ok_or_else(|| ApiError::git("invalid merge-tree conflict entry"))?;
        let mut parts = meta.split_whitespace();
        let mode = parts.next().ok_or_else(|| ApiError::git("missing mode"))?;
        let oid = parts.next().ok_or_else(|| ApiError::git("missing blob"))?;
        let stage = parts
            .next()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|s| (1..=3).contains(s))
            .ok_or_else(|| ApiError::git("invalid conflict stage"))?;
        stages.entry(path.to_string()).or_default()[stage - 1] =
            Some((mode.to_string(), oid.to_string()));
    }
    let mut files = Vec::new();
    for (path, [base, ours, theirs]) in stages {
        let ours = ours.ok_or_else(|| {
            ApiError::badreq(format!(
                "delete/modify conflict needs developer help: {path}"
            ))
        })?;
        let theirs = theirs.ok_or_else(|| {
            ApiError::badreq(format!(
                "delete/modify conflict needs developer help: {path}"
            ))
        })?;
        let ours_bytes = blob(dir, &ours.1).await?;
        let theirs_bytes = blob(dir, &theirs.1).await?;
        let binary = ours_bytes.contains(&0)
            || theirs_bytes.contains(&0)
            || std::str::from_utf8(&ours_bytes).is_err()
            || std::str::from_utf8(&theirs_bytes).is_err();
        files.push(ConflictFile {
            path,
            binary,
            mode: ours.0,
            ours_oid: ours.1,
            theirs_oid: theirs.1,
            base_oid: base.map(|(_, oid)| oid),
        });
    }
    Ok((tree, files))
}

pub(crate) async fn both(dir: &Path, file: &ConflictFile) -> Result<Vec<u8>, ApiError> {
    if file.binary {
        return Err(ApiError::badreq(
            "Keep both is unavailable for binary files",
        ));
    }
    let base = file
        .base_oid
        .as_deref()
        .ok_or_else(|| ApiError::badreq("Keep both needs a common file version"))?;
    let ours = blob(dir, &file.ours_oid).await?;
    let theirs = blob(dir, &file.theirs_oid).await?;
    let base = blob(dir, base).await?;
    // `merge-file --union` combines conflict regions and preserves clean merged edits.
    // Temporary files are outside the project, and all three are removed on return.
    let dir_tmp = std::env::temp_dir().join(format!("wb-conflict-{:016x}", rand::random::<u64>()));
    tokio::fs::create_dir(&dir_tmp)
        .await
        .map_err(ApiError::from)?;
    let result = async {
        for (name, bytes) in [("ours", ours), ("base", base), ("theirs", theirs)] {
            tokio::fs::write(dir_tmp.join(name), bytes)
                .await
                .map_err(ApiError::from)?;
        }
        let (status, stdout, stderr) = git(
            &dir_tmp,
            &["merge-file", "-p", "--union", "ours", "base", "theirs"],
            &[],
            None,
        )
        .await?;
        if status != 0 && status != 1 {
            return Err(ApiError::git(format!(
                "git merge-file failed: {}",
                String::from_utf8_lossy(&stderr)
            )));
        }
        Ok(stdout)
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&dir_tmp).await;
    result
}

pub(crate) async fn candidate_tree(
    dir: &Path,
    merged_tree: &str,
    files: &[ConflictFile],
    choices: &BTreeMap<String, Choice>,
) -> Result<Candidate, ApiError> {
    let index = std::env::temp_dir().join(format!("wb-index-{:016x}", rand::random::<u64>()));
    let index_str = index
        .to_str()
        .ok_or_else(|| ApiError::io("temporary index path is not UTF-8"))?;
    let env = [("GIT_INDEX_FILE", index_str)];
    let result = async {
        checked(dir, &["read-tree", merged_tree], &env, None).await?;
        for file in files {
            let Some(choice) = choices.get(&file.path) else {
                continue;
            };
            let oid = match choice {
                Choice::Yours => file.ours_oid.clone(),
                Choice::Latest => file.theirs_oid.clone(),
                Choice::Both => {
                    let bytes = both(dir, file).await?;
                    String::from_utf8_lossy(
                        &checked(dir, &["hash-object", "-w", "--stdin"], &[], Some(&bytes)).await?,
                    )
                    .trim()
                    .to_string()
                }
            };
            checked(
                dir,
                &[
                    "update-index",
                    "--add",
                    "--cacheinfo",
                    &file.mode,
                    &oid,
                    &file.path,
                ],
                &env,
                None,
            )
            .await?;
        }
        let tree = String::from_utf8_lossy(&checked(dir, &["write-tree"], &env, None).await?)
            .trim()
            .to_string();
        Ok(Candidate {
            tree,
            complete: files.iter().all(|f| choices.contains_key(&f.path)),
        })
    }
    .await;
    let _ = tokio::fs::remove_file(index).await;
    result
}

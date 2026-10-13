//! The project's `DESIGN.md`: YAML front matter (design tokens) plus a Markdown body.
//!
//! Tokens are kept as an order-preserving JSON object so keys the editor does not know
//! about survive a round trip. Saves follow the content-edit rules: an active change, the
//! branch the editor opened, and the hash of the file it read.

use crate::content::catalog;
use crate::content::lifecycle::{Journal, Undo};
use crate::content::mutation::{
    emit_checkpoint, lock_for, project_root, read_current, require_content_change, resolve, stale,
    write_atomic,
};
use crate::{error::ApiError, git, state::AppState, static_files};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

pub const DESIGN_FILE: &str = "DESIGN.md";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignDoc {
    pub exists: bool,
    pub tokens: Value,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    pub branch: String,
    /// The template's DESIGN.md, offered when the project has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starter: Option<Starter>,
}

#[derive(Serialize)]
pub struct Starter {
    pub tokens: Value,
    pub body: String,
}

#[derive(Deserialize)]
pub struct SaveDesign {
    pub project: String,
    pub branch: String,
    /// Hash of the file as read; `None` only when creating it.
    pub hash: Option<String>,
    pub tokens: Value,
    pub body: String,
}

fn invalid(message: impl std::fmt::Display) -> ApiError {
    ApiError::badreq(format!("DESIGN.md front matter is invalid: {message}"))
        .with_reason("invalid_design")
}

/// Attach the raw `source` to an `invalid_design` error as `[{path: ["source"], message}]`
/// (the error envelope's existing validation-detail shape) so the tab can show it read-only.
fn with_source(err: ApiError, source: &str) -> ApiError {
    err.with_details(json!([{ "path": ["source"], "message": source }]))
}

/// Split `source` into front-matter tokens and the body. Front matter is present only when
/// the source starts with `---\n` and a later line is exactly `---`; the first such line
/// closes it, so a horizontal rule in the body is left alone.
pub fn split(source: &str) -> Result<(Value, &str), ApiError> {
    let Some(rest) = source
        .strip_prefix("---\r\n")
        .or_else(|| source.strip_prefix("---\n"))
    else {
        return Ok((json!({}), source));
    };
    let mut start = 0;
    for line in rest.split_inclusive('\n') {
        let text = line
            .strip_suffix("\r\n")
            .or_else(|| line.strip_suffix('\n'))
            .unwrap_or(line);
        if text == "---" {
            let tokens = match serde_yaml::from_str::<Value>(&rest[..start]).map_err(invalid)? {
                Value::Null => json!({}),
                tokens @ Value::Object(_) => tokens,
                _ => return Err(invalid("expected a mapping")),
            };
            return Ok((tokens, &rest[start + line.len()..]));
        }
        start += line.len();
    }
    Ok((json!({}), source))
}

/// The inverse of [`split`]: no tokens means no front matter, unless the body itself opens
/// with a `---` line, which would otherwise be read back as front matter.
pub fn render(tokens: &Map<String, Value>, body: &str) -> Result<String, ApiError> {
    if tokens.is_empty() {
        let opens_with_rule = body.starts_with("---\n") || body.starts_with("---\r\n");
        return Ok(if opens_with_rule {
            format!("---\n---\n{body}")
        } else {
            body.to_string()
        });
    }
    let yaml = serde_yaml::to_string(tokens).map_err(invalid)?;
    Ok(format!("---\n{yaml}---\n{body}"))
}

fn starter() -> Result<Option<Starter>, ApiError> {
    let Some(file) = static_files::Template::get(DESIGN_FILE) else {
        return Ok(None);
    };
    let text = String::from_utf8_lossy(&file.data);
    let (tokens, body) = split(&text)?;
    Ok(Some(Starter {
        tokens,
        body: body.to_string(),
    }))
}

pub async fn read(state: &AppState, project: &str) -> Result<DesignDoc, ApiError> {
    let root = project_root(state, project).await?;
    let path = resolve(&root, DESIGN_FILE)?;
    let branch = catalog::current_branch(&root).await?;
    if std::fs::symlink_metadata(&path).is_err() {
        return Ok(DesignDoc {
            exists: false,
            tokens: json!({}),
            body: String::new(),
            hash: None,
            branch,
            starter: starter()?,
        });
    }
    let source = read_current(&path)?;
    let (tokens, body) = split(&source).map_err(|e| with_source(e, &source))?;
    Ok(DesignDoc {
        exists: true,
        tokens,
        body: body.to_string(),
        hash: Some(catalog::hash(&source)),
        branch,
        starter: None,
    })
}

pub async fn save(state: &AppState, req: SaveDesign) -> Result<DesignDoc, ApiError> {
    let root = project_root(state, &req.project).await?;
    let lock = lock_for(&root);
    let _guard = lock.lock().await;
    require_content_change(&root, &req.branch).await?;
    let Value::Object(tokens) = &req.tokens else {
        return Err(ApiError::badreq("tokens must be an object"));
    };
    let source = render(tokens, &req.body)?;

    let path = resolve(&root, DESIGN_FILE)?;
    let (old, mode) = match (std::fs::symlink_metadata(&path), &req.hash) {
        (Err(_), None) => (None, 0o644),
        (Ok(meta), Some(hash)) if meta.is_file() => {
            let old = read_current(&path)?;
            if &catalog::hash(&old) != hash {
                return Err(stale());
            }
            // Never overwrite a document that does not parse; it must be repaired by hand.
            split(&old).map_err(|e| with_source(e, &old))?;
            (Some(old), meta.permissions().mode() & 0o7777)
        }
        _ => return Err(stale()),
    };
    write_atomic(&path, source.as_bytes(), mode)?;
    // Like content saves: a failed checkpoint must not leave an uncheckpointed write.
    let mut journal = Journal::default();
    journal.0.push(match old {
        Some(bytes) => Undo::Write {
            path: path.clone(),
            bytes,
            mode,
        },
        None => Undo::Remove(path.clone()),
    });
    let checkpoint =
        match git::checkpoint_paths(&root, "Updated design", &[PathBuf::from(DESIGN_FILE)]).await {
            Ok(cp) => cp,
            Err(e) => return Err(journal.fail(e)),
        };
    emit_checkpoint(state, &req.project, checkpoint);
    read(state, &req.project).await
}

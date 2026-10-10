//! Runs the project's template-local content helper (`scripts/workbench-content.mjs`).
//!
//! The helper never writes project files; it describes, validates, and patches values.
use crate::{error::ApiError, fs};
use serde_json::{json, Map, Value};
use std::path::Path;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;

const SCRIPT: &str = "scripts/workbench-content.mjs";

/// Send one request. The outer error means the helper could not run; the inner error is
/// the helper's own refusal message (stderr of a failed request).
pub async fn run(root: &Path, request: &Value) -> Result<Result<Value, String>, ApiError> {
    // Verify helper containment before executing project code.
    fs::read_file(root, SCRIPT).await?;
    let mut child = tokio::process::Command::new("node")
        .arg(SCRIPT)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| ApiError::io(format!("content helper: {e}")))?;
    let mut line = serde_json::to_vec(request).map_err(|e| ApiError::io(e.to_string()))?;
    line.push(b'\n');
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(&line).await?;
    drop(stdin);
    let output = tokio::time::timeout(std::time::Duration::from_secs(10), child.wait_with_output())
        .await
        .map_err(|_| ApiError::io("content helper timed out"))??;
    if !output.status.success() {
        return Ok(Err(String::from_utf8_lossy(&output.stderr)
            .trim()
            .to_string()));
    }
    serde_json::from_slice(&output.stdout)
        .map(Ok)
        .map_err(|e| ApiError::io(format!("content helper invalid response: {e}")))
}

/// Validate the merged candidate and return the source-preserving patched text.
/// Schema issues become `invalid_schema` with `[{path, message}]` details.
pub async fn patch(
    root: &Path,
    collection: &str,
    source: &str,
    fields: &Map<String, Value>,
    unset: &[String],
    body: Option<&str>,
) -> Result<String, ApiError> {
    let mut request =
        json!({"op":"patch","collection":collection,"source":source,"fields":fields,"unset":unset});
    if let Some(body) = body {
        request["body"] = json!(body);
    }
    match run(root, &request).await? {
        Ok(response) => response
            .get("source")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| ApiError::io("content helper returned no source")),
        Err(message) => Err(match serde_json::from_str::<Value>(&message) {
            Ok(issues @ Value::Array(_)) => {
                ApiError::badreq("Content does not match the collection schema")
                    .with_status(422)
                    .with_reason("invalid_schema")
                    .with_details(issues)
            }
            _ => ApiError::badreq(format!("content helper: {message}")),
        }),
    }
}

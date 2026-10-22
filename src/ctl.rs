//! `orso ctl`: app state for the agent in a orso terminal. Posts the session's
//! hook token to `/api/ctl/*` and prints the daemon's JSON answer.

use std::process::ExitCode;
use std::time::Duration;

use serde_json::{json, Value};

const NOT_IN_SESSION: &str = "orso ctl: run this inside a orso agent session";

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::Subcommand)]
pub enum CtlCommand {
    /// Preview state: devserver starting / ready / crashed, port, preview path.
    PreviewStatus,
    /// Restart the preview's dev server (after a crash), then print its state.
    RestartPreview,
    /// The active change, unsaved edits, and other open changes.
    ChangeStatus,
    /// Remount the preview iframe in the app UI.
    RefreshPreview,
    /// Reload Content collections, entry lists, and the open entry in the app UI.
    RefreshContent,
}

impl CtlCommand {
    fn route(self) -> &'static str {
        match self {
            Self::PreviewStatus => "preview-status",
            Self::RestartPreview => "restart-preview",
            Self::ChangeStatus => "change-status",
            Self::RefreshPreview => "refresh-preview",
            Self::RefreshContent => "refresh-content",
        }
    }
}

/// Endpoint URL and JSON body for `cmd`.
pub fn request_for(cmd: CtlCommand, base: &str, project: &str, token: &str) -> (String, Value) {
    (
        format!("{}/api/ctl/{}", base.trim_end_matches('/'), cmd.route()),
        json!({ "project": project, "token": token }),
    )
}

/// Run `cmd` with the process environment.
pub async fn run(cmd: CtlCommand) -> ExitCode {
    run_with(cmd, |k| std::env::var(k).ok()).await
}

/// Run `cmd` reading `ORSO_*` through `var`: pretty JSON on stdout, errors on stderr.
pub async fn run_with(cmd: CtlCommand, var: impl Fn(&str) -> Option<String>) -> ExitCode {
    let (Some(base), Some(project), Some(token)) =
        (var("ORSO_URL"), var("ORSO_PROJECT"), var("ORSO_HOOK_TOKEN"))
    else {
        eprintln!("{NOT_IN_SESSION}");
        return ExitCode::FAILURE;
    };
    let (url, body) = request_for(cmd, &base, &project, &token);
    match send(&url, &body).await {
        Ok(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("orso ctl: {msg}");
            ExitCode::FAILURE
        }
    }
}

async fn send(url: &str, body: &Value) -> Result<Value, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let res = client
        .post(url)
        .json(body)
        .send()
        .await
        .map_err(|e| format!("cannot reach orso: {e}"))?;
    let status = res.status();
    let v: Value = res.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(v);
    }
    let msg = v["error"]["message"].as_str().unwrap_or("request failed");
    Err(format!("{msg} ({status})"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::process::ExitCode;
    use std::sync::{Arc, Mutex};

    use axum::http::StatusCode;
    use serde_json::{json, Value};

    #[test]
    fn request_targets_the_ctl_route_with_project_and_token() {
        for (cmd, route) in [
            (CtlCommand::PreviewStatus, "preview-status"),
            (CtlCommand::RestartPreview, "restart-preview"),
            (CtlCommand::ChangeStatus, "change-status"),
            (CtlCommand::RefreshPreview, "refresh-preview"),
            (CtlCommand::RefreshContent, "refresh-content"),
        ] {
            let (url, body) = request_for(cmd, "http://127.0.0.1:7800/", "site", "tok");
            assert_eq!(url, format!("http://127.0.0.1:7800/api/ctl/{route}"));
            assert_eq!(body, json!({ "project": "site", "token": "tok" }));
        }
    }

    /// Stub daemon answering every ctl route with `status` + `reply`; records bodies.
    async fn stub(status: StatusCode, reply: Value) -> (String, Arc<Mutex<Vec<Value>>>) {
        let seen: Arc<Mutex<Vec<Value>>> = Arc::default();
        let s2 = seen.clone();
        let app = axum::Router::new().route(
            "/api/ctl/{route}",
            axum::routing::post(move |axum::Json(v): axum::Json<Value>| {
                let (s, reply) = (s2.clone(), reply.clone());
                async move {
                    s.lock().unwrap().push(v);
                    (status, axum::Json(reply))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{addr}"), seen)
    }

    fn env(url: &str) -> impl Fn(&str) -> Option<String> {
        let vars: HashMap<&str, String> = [
            ("ORSO_URL", url.to_string()),
            ("ORSO_PROJECT", "site".to_string()),
            ("ORSO_HOOK_TOKEN", "tok".to_string()),
        ]
        .into();
        move |k| vars.get(k).cloned()
    }

    #[tokio::test]
    async fn run_succeeds_on_a_json_answer() {
        let (url, seen) = stub(StatusCode::OK, json!({ "devserver": "ready" })).await;
        let code = run_with(CtlCommand::PreviewStatus, env(&url)).await;
        assert_eq!(code, ExitCode::SUCCESS);
        assert_eq!(
            *seen.lock().unwrap(),
            vec![json!({ "project": "site", "token": "tok" })]
        );
    }

    #[tokio::test]
    async fn run_fails_when_the_daemon_refuses() {
        let reply = json!({ "error": { "code": "EUNAUTH", "message": "invalid hook token" } });
        let (url, _) = stub(StatusCode::FORBIDDEN, reply).await;
        let code = run_with(CtlCommand::ChangeStatus, env(&url)).await;
        assert_eq!(code, ExitCode::FAILURE);
    }

    #[tokio::test]
    async fn run_without_session_env_fails_and_sends_nothing() {
        let (url, seen) = stub(StatusCode::OK, json!({})).await;
        let full = env(&url);
        for missing in ["ORSO_URL", "ORSO_PROJECT", "ORSO_HOOK_TOKEN"] {
            let partial = |k: &str| (k != missing).then(|| full(k)).flatten();
            let code = run_with(CtlCommand::PreviewStatus, partial).await;
            assert_eq!(code, ExitCode::FAILURE, "{missing}");
        }
        assert!(seen.lock().unwrap().is_empty());
    }
}

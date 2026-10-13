//! `workbench hook turn-end`: invoked by a coding CLI's turn-end hook; tells the daemon.

use std::io::{IsTerminal, Read};
use std::process::ExitCode;
use std::time::Duration;

/// Extract a turn summary from a hook payload: last `input-messages` string (Codex),
/// else `summary` (OpenCode plugin), else `last_assistant_message` (Claude).
pub fn summary_from_payload(payload: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(payload).ok()?;
    let obj = v.as_object()?;
    let non_empty = |s: &str| (!s.trim().is_empty()).then(|| s.to_string());
    let last_input = obj
        .get("input-messages")
        .and_then(|m| m.as_array())
        .and_then(|a| a.iter().rev().find_map(|x| x.as_str()))
        .and_then(non_empty);
    last_input
        .or_else(|| {
            obj.get("summary")
                .and_then(|s| s.as_str())
                .and_then(non_empty)
        })
        .or_else(|| {
            obj.get("last_assistant_message")
                .and_then(|s| s.as_str())
                .and_then(non_empty)
        })
}

/// The CLI's conversation id from a hook payload: `thread-id` (Codex), else `session_id`
/// (Claude, OpenCode plugin).
pub fn session_id_from_payload(payload: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(payload).ok()?;
    ["thread-id", "session_id"]
        .iter()
        .find_map(|k| v.get(k)?.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn read_stdin() -> String {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return String::new();
    }
    let mut buf = String::new();
    let _ = stdin.lock().read_to_string(&mut buf);
    buf
}

/// Notify the daemon that a turn ended. Never fails the calling CLI: always `SUCCESS`.
pub async fn turn_end(payload_arg: Option<String>) -> ExitCode {
    let (Ok(url), Ok(project), Ok(token)) = (
        std::env::var("WORKBENCH_URL"),
        std::env::var("WORKBENCH_PROJECT"),
        std::env::var("WORKBENCH_HOOK_TOKEN"),
    ) else {
        return ExitCode::SUCCESS;
    };
    let payload = match payload_arg {
        Some(p) => p,
        None => tokio::task::spawn_blocking(read_stdin)
            .await
            .unwrap_or_default(),
    };
    let summary = summary_from_payload(&payload);
    let mut body = serde_json::json!({"project": project, "token": token, "summary": summary});
    if let Some(id) = session_id_from_payload(&payload) {
        body["sessionId"] = id.into();
    }
    let endpoint = format!("{}/api/terminal/turn-end", url.trim_end_matches('/'));
    let result = async {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        client
            .post(endpoint)
            .json(&body)
            .send()
            .await?
            .error_for_status()
    }
    .await;
    if let Err(e) = result {
        eprintln!("workbench hook: {e}");
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn codex_payload_uses_last_input_message() {
        let p = r#"{"type":"agent-turn-complete","input-messages":["a","b"]}"#;
        assert_eq!(summary_from_payload(p), Some("b".into()));
    }

    #[test]
    fn claude_payload_uses_last_assistant_message() {
        let p = r#"{"last_assistant_message":"done it"}"#;
        assert_eq!(summary_from_payload(p), Some("done it".into()));
    }

    #[test]
    fn opencode_payload_uses_summary() {
        assert_eq!(
            summary_from_payload(r#"{"session_id":"x","summary":"s"}"#),
            Some("s".into())
        );
        assert_eq!(summary_from_payload(r#"{"session_id":"x"}"#), None);
    }

    #[test]
    fn garbage_payload_is_none() {
        for p in ["not json", "", "[]"] {
            assert_eq!(summary_from_payload(p), None, "{p:?}");
            assert_eq!(session_id_from_payload(p), None, "{p:?}");
        }
    }

    #[test]
    fn session_id_from_codex_thread_or_session_id() {
        assert_eq!(
            session_id_from_payload(r#"{"thread-id":"t1","session_id":"s1"}"#),
            Some("t1".into())
        );
        assert_eq!(session_id_from_payload(r#"{"session_id":"s1"}"#), Some("s1".into()));
        assert_eq!(session_id_from_payload(r#"{"session_id":" "}"#), None);
    }

    fn clear_env() {
        for k in ["WORKBENCH_URL", "WORKBENCH_PROJECT", "WORKBENCH_HOOK_TOKEN"] {
            unsafe { std::env::remove_var(k) };
        }
    }

    #[tokio::test]
    async fn missing_env_exits_zero_without_request() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_env();
        assert_eq!(turn_end(Some("{}".into())).await, ExitCode::SUCCESS);
    }

    #[tokio::test]
    async fn posts_to_daemon() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::default();
        let s2 = seen.clone();
        let app = axum::Router::new().route(
            "/api/terminal/turn-end",
            axum::routing::post(move |axum::Json(v): axum::Json<serde_json::Value>| {
                let s = s2.clone();
                async move {
                    s.lock().unwrap().push(v);
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        unsafe {
            std::env::set_var("WORKBENCH_URL", format!("http://{addr}"));
            std::env::set_var("WORKBENCH_PROJECT", "proj");
            std::env::set_var("WORKBENCH_HOOK_TOKEN", "tok");
        }
        let code = turn_end(Some(r#"{"input-messages":["a","b"]}"#.into())).await;
        clear_env();
        assert_eq!(code, ExitCode::SUCCESS);
        let got = seen.lock().unwrap().clone();
        assert_eq!(
            got,
            vec![serde_json::json!({"project":"proj","token":"tok","summary":"b"})]
        );
    }
}

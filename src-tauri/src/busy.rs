//! Whether any terminal session is mid-turn, asked of the daemon's own HTTP
//! API with its token, so an update never interrupts an agent.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use serde_json::Value;
use tauri::Url;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// Asks the daemon at `base_url` whether any project's terminal is busy.
/// Anything unreadable counts as busy.
pub fn daemon_busy(base_url: &str, token: &str) -> bool {
    let Some(ids) = get_json(base_url, &["api", "projects"], token).and_then(|v| project_ids(&v))
    else {
        return true;
    };
    any_busy(
        ids.iter()
            .map(|id| get_json(base_url, &["api", "terminal", id], token)),
    )
}

/// Project ids from a `GET /api/projects` body (`{ projects: [{ id, .. }] }`).
pub fn project_ids(projects: &Value) -> Option<Vec<String>> {
    projects
        .get("projects")?
        .as_array()?
        .iter()
        .map(|p| p.get("id")?.as_str().map(str::to_owned))
        .collect()
}

/// Folds `GET /api/terminal/{project}` bodies. A status that couldn't be read
/// (`None`) counts as busy: what can't be seen can't be interrupted safely.
pub fn any_busy(statuses: impl IntoIterator<Item = Option<Value>>) -> bool {
    statuses
        .into_iter()
        .any(|status| match status.as_ref().and_then(Value::as_object) {
            None => true,
            Some(s) => s.get("busy").and_then(Value::as_bool) == Some(true),
        })
}

/// `GET` a JSON endpoint on the loopback daemon. `None` on any failure.
fn get_json(base_url: &str, segments: &[&str], token: &str) -> Option<Value> {
    let mut url = Url::parse(base_url).ok()?;
    let port = url.port()?;
    url.path_segments_mut().ok()?.clear().extend(segments);
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut stream = TcpStream::connect_timeout(&addr, REQUEST_TIMEOUT).ok()?;
    stream.set_read_timeout(Some(REQUEST_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(REQUEST_TIMEOUT)).ok()?;
    // HTTP/1.0 keeps the response un-chunked and closes the connection.
    let request = format!(
        "GET {} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {token}\r\n\r\n",
        url.path()
    );
    stream.write_all(request.as_bytes()).ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    json_body(&response)
}

/// The JSON body of a 200 response.
fn json_body(response: &str) -> Option<Value> {
    let (head, body) = response.split_once("\r\n\r\n")?;
    let status = head.lines().next()?.split_whitespace().nth(1)?;
    (status == "200").then(|| serde_json::from_str(body).ok())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn project_ids_come_from_the_projects_list() {
        let body = json!({ "projects": [{ "id": "a", "name": "A" }, { "id": "b" }] });
        assert_eq!(project_ids(&body), Some(vec!["a".into(), "b".into()]));
        assert_eq!(project_ids(&json!({ "projects": [] })), Some(vec![]));
    }

    #[test]
    fn malformed_projects_lists_are_unreadable() {
        assert_eq!(project_ids(&json!({})), None);
        assert_eq!(
            project_ids(&json!({ "projects": [{ "name": "no id" }] })),
            None
        );
    }

    #[test]
    fn idle_when_no_session_is_busy() {
        assert!(!any_busy([]));
        assert!(!any_busy([
            Some(json!({ "running": false })),
            Some(json!({ "running": true, "provider": "claude", "busy": false })),
        ]));
    }

    #[test]
    fn busy_when_any_session_is_busy() {
        assert!(any_busy([
            Some(json!({ "running": true, "busy": false })),
            Some(json!({ "running": true, "busy": true })),
        ]));
    }

    #[test]
    fn unreadable_statuses_count_as_busy() {
        assert!(any_busy([Some(json!({ "running": false })), None]));
        assert!(any_busy([Some(json!("not an object"))]));
    }

    #[test]
    fn only_200_json_bodies_are_read() {
        assert_eq!(
            json_body(
                "HTTP/1.0 200 OK\r\ncontent-type: application/json\r\n\r\n{\"running\":false}"
            ),
            Some(json!({ "running": false }))
        );
        assert_eq!(
            json_body("HTTP/1.0 401 Unauthorized\r\n\r\n{\"error\":\"x\"}"),
            None
        );
        assert_eq!(json_body("HTTP/1.0 200 OK\r\n\r\nnot json"), None);
        assert_eq!(json_body(""), None);
    }
}

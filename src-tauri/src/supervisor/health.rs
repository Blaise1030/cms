use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

pub const POLL_INTERVAL: Duration = Duration::from_millis(250);
pub const TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(1);

/// Polls `GET /api/health` until it reports ok. Returns false on timeout or
/// as soon as `abort` returns true.
pub fn wait_healthy(port: u16, abort: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + TIMEOUT;
    while Instant::now() < deadline {
        if abort() {
            return false;
        }
        if check(port) {
            return true;
        }
        thread::sleep(POLL_INTERVAL);
    }
    false
}

fn check(port: u16) -> bool {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, REQUEST_TIMEOUT) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(REQUEST_TIMEOUT));
    let _ = stream.set_write_timeout(Some(REQUEST_TIMEOUT));
    // HTTP/1.0 keeps the response un-chunked and closes the connection.
    let request = format!("GET /api/health HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = String::new();
    stream.read_to_string(&mut response).is_ok() && is_healthy_response(&response)
}

pub fn is_healthy_response(response: &str) -> bool {
    let Some((head, body)) = response.split_once("\r\n\r\n") else {
        return false;
    };
    let status_ok = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        == Some("200");
    status_ok
        && serde_json::from_str::<serde_json::Value>(body)
            .is_ok_and(|v| v.get("status").and_then(|s| s.as_str()) == Some("ok"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_status_with_extra_fields_is_healthy() {
        let resp = "HTTP/1.0 200 OK\r\ncontent-type: application/json\r\n\r\n{\"status\":\"ok\",\"version\":\"0.1.0\"}";
        assert!(is_healthy_response(resp));
    }

    #[test]
    fn non_200_or_bad_status_is_unhealthy() {
        assert!(!is_healthy_response("HTTP/1.0 503 Service Unavailable\r\n\r\n{\"status\":\"ok\"}"));
        assert!(!is_healthy_response("HTTP/1.0 200 OK\r\n\r\n{\"status\":\"starting\"}"));
        assert!(!is_healthy_response("HTTP/1.0 200 OK\r\n\r\nnot json"));
        assert!(!is_healthy_response(""));
    }
}

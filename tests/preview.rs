//! `/preview/{id}/*` reverse proxy, exercised against a fake dev server (HTTP + WS) bound
//! to the project's port. No npm or Astro needed.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;

use axum::extract::ws::{Message as AxMessage, WebSocketUpgrade};
use axum::extract::Request;
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{any, get};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use workbench::config::Config;
use workbench::http;
use workbench::projects::DevserverStatus;
use workbench::state::AppState;

/// Deliberately hostile to rewriting: absolute prefixed URLs, a `<base>`, odd bytes.
const INDEX: &str = "<!doctype html><html><head><base href=\"/preview/site/\">\
<script type=\"module\" src=\"/preview/site/@vite/client\"></script></head>\
<body data-x='/absolute'>héllo  \t wörld</body></html>\n";

fn config(dir: &Path, token: Option<&str>) -> Config {
    Config {
        port: 0,
        origin: None,
        projects_dir: dir.to_path_buf(),
        no_auth: token.is_none(),
        token: token.map(str::to_string),
        agents_config: None,
    }
}

struct Env {
    _tmp: tempfile::TempDir,
    state: AppState,
    daemon: SocketAddr,
    upstream_port: u16,
}

fn upstream_router() -> Router {
    Router::new()
        .route(
            "/preview/site/",
            get(|| async { ([("x-upstream", "yes")], Html(INDEX)) }),
        )
        .route("/preview/site/echo", any(echo))
        .route("/preview/site/hmr", get(hmr))
}

async fn echo(req: Request) -> Json<Value> {
    let (parts, body) = req.into_parts();
    let body = axum::body::to_bytes(body, 1 << 20).await.unwrap();
    let headers: HashMap<String, String> = parts
        .headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    Json(json!({
        "method": parts.method.as_str(),
        "uri": parts.uri.path_and_query().map(|p| p.as_str()),
        "headers": headers,
        "body": String::from_utf8_lossy(&body),
    }))
}

async fn hmr(ws: WebSocketUpgrade, headers: HeaderMap) -> Response {
    let origin = headers
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_string();
    ws.protocols(["vite-hmr"])
        .on_upgrade(move |mut s| async move {
            let hello = json!({ "type": "connected", "origin": origin }).to_string();
            let _ = s.send(AxMessage::Text(hello.into())).await;
            while let Some(Ok(m)) = s.recv().await {
                let reply = match m {
                    AxMessage::Text(t) => AxMessage::Text(format!("echo:{t}").into()),
                    AxMessage::Binary(b) => AxMessage::Binary(b),
                    AxMessage::Close(_) => break,
                    _ => continue,
                };
                if s.send(reply).await.is_err() {
                    break;
                }
            }
        })
        .into_response()
}

/// Project `site` with the fake dev server bound to its port, plus the daemon on a real
/// socket (WebSocket upgrades need one).
async fn env(token: Option<&str>) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::with_scaffold_install(config(tmp.path(), token), false);
    let info = state.projects.create("site").await.unwrap();
    assert_eq!(info.id, "site");
    let up = TcpListener::bind(("127.0.0.1", info.port)).await.unwrap();
    tokio::spawn(async move { axum::serve(up, upstream_router()).await.unwrap() });

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let daemon = listener.local_addr().unwrap();
    let app = http::router(state.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Env {
        _tmp: tmp,
        state,
        daemon,
        upstream_port: info.port,
    }
}

struct Reply {
    status: u16,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

/// Minimal HTTP/1.1 client: one request, `Connection: close`, read to EOF.
async fn raw(addr: SocketAddr, method: &str, path: &str, extra: &[&str], body: &str) -> Reply {
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n");
    for h in extra {
        req.push_str(h);
        req.push_str("\r\n");
    }
    req.push_str(&format!("Content-Length: {}\r\n\r\n{body}", body.len()));
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(req.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).await.unwrap();
    let split = buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&buf[..split]).into_owned();
    let mut lines = head.lines();
    let status = lines
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    Reply {
        status,
        headers,
        body: buf[split + 4..].to_vec(),
    }
}

#[tokio::test]
async fn html_is_forwarded_byte_for_byte_under_the_prefix() {
    let env = env(None).await;
    let r = raw(env.daemon, "GET", "/preview/site/", &[], "").await;
    assert_eq!(r.status, 200);
    assert_eq!(r.headers["x-upstream"], "yes");
    assert!(r.headers["content-type"].starts_with("text/html"));
    assert_eq!(String::from_utf8(r.body).unwrap(), INDEX);
}

#[tokio::test]
async fn request_is_forwarded_with_path_query_and_body_but_without_daemon_credentials() {
    let env = env(Some("abc123")).await;
    let r = raw(
        env.daemon,
        "POST",
        "/preview/site/echo?y=1&t=abc123",
        &[
            "Authorization: Bearer abc123",
            "Cookie: a=1; wb_session=abc123",
            "Content-Type: text/plain",
        ],
        "payload",
    )
    .await;
    assert_eq!(r.status, 200, "{}", String::from_utf8_lossy(&r.body));
    let v = r.json();
    assert_eq!(v["method"], "POST");
    assert_eq!(
        v["uri"], "/preview/site/echo?y=1",
        "prefix kept, token gone"
    );
    assert_eq!(v["body"], "payload");
    let h = &v["headers"];
    assert!(h.get("authorization").is_none(), "{h}");
    assert_eq!(h["cookie"], "a=1");
    assert_eq!(h["host"], format!("127.0.0.1:{}", env.upstream_port));
    assert_eq!(h["x-forwarded-host"], "localhost");
}

#[tokio::test]
async fn preview_requires_auth() {
    let env = env(Some("abc123")).await;
    let r = raw(env.daemon, "GET", "/preview/site/", &[], "").await;
    assert_eq!(r.status, 401);
    let ok = raw(
        env.daemon,
        "GET",
        "/preview/site/",
        &["Cookie: wb_session=abc123"],
        "",
    )
    .await;
    assert_eq!(ok.status, 200);
}

#[tokio::test]
async fn websocket_hmr_passes_through_with_subprotocol() {
    let env = env(None).await;
    let mut req = format!("ws://{}/preview/site/hmr", env.daemon)
        .into_client_request()
        .unwrap();
    req.headers_mut()
        .insert("sec-websocket-protocol", "vite-hmr".parse().unwrap());
    req.headers_mut()
        .insert("origin", format!("http://{}", env.daemon).parse().unwrap());
    let (mut ws, res) = tokio_tungstenite::connect_async(req).await.unwrap();
    assert_eq!(res.status(), 101);
    assert_eq!(res.headers()["sec-websocket-protocol"], "vite-hmr");

    let hello: Value = match ws.next().await.unwrap().unwrap() {
        Message::Text(t) => serde_json::from_str(t.as_str()).unwrap(),
        m => panic!("unexpected {m:?}"),
    };
    assert_eq!(hello["type"], "connected");
    // Origin reaches the dev server untouched.
    assert_eq!(hello["origin"], format!("http://{}", env.daemon));

    ws.send(Message::text("ping")).await.unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap(),
        Message::text("echo:ping")
    );
    ws.send(Message::binary(vec![0u8, 1, 2, 255]))
        .await
        .unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap(),
        Message::binary(vec![0u8, 1, 2, 255])
    );
    ws.close(None).await.unwrap();
}

#[tokio::test]
async fn websocket_with_foreign_origin_is_refused() {
    let env = env(None).await;
    let r = raw(
        env.daemon,
        "GET",
        "/preview/site/hmr",
        &[
            "Upgrade: websocket",
            "Connection: Upgrade",
            "Sec-WebSocket-Version: 13",
            "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==",
            "Origin: https://evil.example",
        ],
        "",
    )
    .await;
    assert_eq!(r.status, 403);
}

#[tokio::test]
async fn unknown_project_is_404_and_dead_dev_server_is_503_then_502() {
    let env = env(None).await;
    let r = raw(env.daemon, "GET", "/preview/nope/", &[], "").await;
    assert_eq!(r.status, 404);
    assert_eq!(r.json()["error"]["code"], "ENOENT");

    // A project whose dev server never came up (nothing listens on its port).
    env.state.projects.create("down").await.unwrap();
    let r = raw(env.daemon, "GET", "/preview/down/", &[], "").await;
    assert_eq!(r.status, 503, "still starting");
    assert_eq!(r.json()["error"]["code"], "DEVSERVER_ERR");

    env.state
        .projects
        .set_devserver("down", DevserverStatus::Crashed)
        .await;
    let r = raw(env.daemon, "GET", "/preview/down/x.js", &[], "").await;
    assert_eq!(r.status, 502);
    assert_eq!(r.json()["error"]["code"], "DEVSERVER_ERR");
}

#[tokio::test]
async fn large_request_body_survives_the_proxy() {
    let env = env(None).await;
    let payload = "x".repeat(300_000);
    let r = raw(env.daemon, "POST", "/preview/site/echo", &[], &payload).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.json()["body"].as_str().unwrap().len(), payload.len());
}

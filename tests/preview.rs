//! Preview origins: each project's preview is a loopback port that reverse-proxies its dev
//! server at `/`. Exercised against a fake dev server (HTTP + WS) bound to the project's port.
//! No npm or Astro needed.

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
use orso::config::Config;
use orso::devserver::DevServerOptions;
use orso::http;
use orso::preview::PreviewAccess;
use orso::projects::DevserverStatus;
use orso::state::AppState;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

/// Deliberately hostile to rewriting: absolute root URLs, odd bytes.
const INDEX: &str = "<!doctype html><html><head>\
<script type=\"module\" src=\"/@vite/client\"></script></head>\
<body data-x='/absolute'><a href=\"/pricing\">héllo  \t wörld</a></body></html>\n";

const TOKEN: &str = "abc123";

fn config(dir: &Path, token: Option<&str>) -> Config {
    Config {
        port: 0,
        origin: None,
        projects_dir: dir.to_path_buf(),
        no_auth: token.is_none(),
        token: token.map(str::to_string),
        agents_config: None,
        ..Default::default()
    }
}

struct Env {
    _tmp: tempfile::TempDir,
    state: AppState,
    daemon: SocketAddr,
    upstream_port: u16,
    /// The `site` project's preview origin.
    preview: SocketAddr,
    access: PreviewAccess,
}

impl Env {
    /// The cookie the preview origin mints after a first load with its token.
    fn cookie(&self) -> String {
        format!(
            "Cookie: orso_preview_{}={}",
            self.access.port,
            self.access.token.as_deref().unwrap_or_default()
        )
    }
}

fn upstream_router() -> Router {
    Router::new()
        .route(
            "/",
            get(|| async { ([("x-upstream", "yes")], Html(INDEX)) }),
        )
        .route("/echo", any(echo))
        .route("/hmr", get(hmr))
        .route(
            "/@vite/client",
            get(|| async { ([("x-vite", "client")], "import.meta.hot=true") }),
        )
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
    let cookie = headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_string();
    ws.protocols(["vite-hmr"])
        .on_upgrade(move |mut s| async move {
            let hello = json!({ "type": "connected", "origin": origin, "cookie": cookie });
            let _ = s.send(AxMessage::Text(hello.to_string().into())).await;
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

/// Project `site` with the fake dev server bound to its port, the daemon on a real socket,
/// and the project's preview origin opened.
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
    let access = state.preview_origins.ensure("site").await.unwrap();
    Env {
        _tmp: tmp,
        state,
        daemon,
        upstream_port: info.port,
        preview: SocketAddr::from(([127, 0, 0, 1], access.port)),
        access,
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
    let host = if extra.iter().any(|h| h.starts_with("Host:")) {
        String::new()
    } else {
        "Host: localhost\r\n".to_string()
    };
    let mut req = format!("{method} {path} HTTP/1.1\r\n{host}Connection: close\r\n");
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
async fn html_is_forwarded_byte_for_byte_at_the_origin_root() {
    let env = env(None).await;
    assert_eq!(env.access.token, None, "no token without auth");
    let r = raw(env.preview, "GET", "/", &[], "").await;
    assert_eq!(r.status, 200);
    assert_eq!(r.headers["x-upstream"], "yes");
    assert!(r.headers["content-type"].starts_with("text/html"));
    assert_eq!(String::from_utf8(r.body).unwrap(), INDEX);
    // Vite's root modules are plain paths on this origin; nothing pins them to a project.
    let r = raw(env.preview, "GET", "/@vite/client", &[], "").await;
    assert_eq!(r.status, 200);
    assert_eq!(r.headers["x-vite"], "client");
    assert!(!r.headers.contains_key("set-cookie"));
}

#[tokio::test]
async fn unauthenticated_requests_are_refused() {
    let env = env(Some(TOKEN)).await;
    let token = env.access.token.clone().unwrap();
    assert_ne!(token, TOKEN, "each origin has a token of its own");

    let r = raw(env.preview, "GET", "/", &[], "").await;
    assert_eq!(r.status, 401);
    let r = raw(env.preview, "GET", "/?t=wrong", &[], "").await;
    assert_eq!(r.status, 401);
    // Another origin's cookie (same host, other port) is not this one's.
    let r = raw(
        env.preview,
        "GET",
        "/",
        &[&format!("Cookie: orso_preview_1={token}")],
        "",
    )
    .await;
    assert_eq!(r.status, 401);
    let r = raw(
        env.preview,
        "GET",
        "/hmr",
        &[
            "Upgrade: websocket",
            "Connection: Upgrade",
            "Sec-WebSocket-Version: 13",
            "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==",
        ],
        "",
    )
    .await;
    assert_eq!(r.status, 401, "no HMR without auth");
    // DNS rebinding: a foreign Host is refused even with a valid cookie.
    let r = raw(
        env.preview,
        "GET",
        "/",
        &["Host: evil.example", &env.cookie()],
        "",
    )
    .await;
    assert_eq!(r.status, 403);
}

#[tokio::test]
async fn first_load_token_becomes_a_cookie_and_leaves_the_address() {
    let env = env(Some(TOKEN)).await;
    let token = env.access.token.clone().unwrap();
    let r = raw(
        env.preview,
        "GET",
        &format!("/about/?x=1&t={token}#top"),
        &[],
        "",
    )
    .await;
    assert_eq!(r.status, 302);
    assert_eq!(r.headers["location"], "/about/?x=1");
    let cookie = &r.headers["set-cookie"];
    assert!(
        cookie.starts_with(&format!("orso_preview_{}={token};", env.access.port)),
        "{cookie}"
    );
    assert!(cookie.contains("HttpOnly"), "{cookie}");
    assert!(cookie.contains("SameSite=Lax"), "{cookie}");

    let r = raw(env.preview, "GET", "/", &[&env.cookie()], "").await;
    assert_eq!(r.status, 200);
    assert_eq!(String::from_utf8(r.body).unwrap(), INDEX);
}

#[tokio::test]
async fn daemon_credentials_also_open_the_preview() {
    let env = env(Some(TOKEN)).await;
    let r = raw(
        env.preview,
        "GET",
        "/",
        &[&format!("Authorization: Bearer {TOKEN}")],
        "",
    )
    .await;
    assert_eq!(r.status, 200);
    let r = raw(
        env.preview,
        "GET",
        "/",
        &[&format!("Cookie: wb_session={TOKEN}")],
        "",
    )
    .await;
    assert_eq!(r.status, 200);
}

#[tokio::test]
async fn request_is_forwarded_with_path_query_and_body_but_without_credentials() {
    let env = env(Some(TOKEN)).await;
    let token = env.access.token.clone().unwrap();
    // POST is not redirected: it is forwarded with the token removed.
    let r = raw(
        env.preview,
        "POST",
        &format!("/echo?y=1&t={token}"),
        &[
            &format!("Authorization: Bearer {TOKEN}"),
            &format!(
                "Cookie: a=1; wb_session={TOKEN}; orso_preview_{}={token}; orso_preview_9=x",
                env.access.port
            ),
            "Content-Type: text/plain",
        ],
        "payload",
    )
    .await;
    assert_eq!(r.status, 200, "{}", String::from_utf8_lossy(&r.body));
    let v = r.json();
    assert_eq!(v["method"], "POST");
    assert_eq!(v["uri"], "/echo?y=1", "path kept, token gone");
    assert_eq!(v["body"], "payload");
    let h = &v["headers"];
    assert!(h.get("authorization").is_none(), "{h}");
    assert_eq!(h["cookie"], "a=1", "only the site's own cookies reach it");
    assert_eq!(h["host"], format!("127.0.0.1:{}", env.upstream_port));
    assert_eq!(h["x-forwarded-host"], "localhost");
}

#[tokio::test]
async fn websocket_hmr_is_tunnelled_with_subprotocol() {
    let env = env(Some(TOKEN)).await;
    let mut req = format!("ws://{}/hmr", env.preview)
        .into_client_request()
        .unwrap();
    let h = req.headers_mut();
    h.insert("sec-websocket-protocol", "vite-hmr".parse().unwrap());
    h.insert("origin", format!("http://{}", env.preview).parse().unwrap());
    let cookie = env.cookie();
    h.insert(
        "cookie",
        format!("{}; keep=1", cookie.trim_start_matches("Cookie: "))
            .parse()
            .unwrap(),
    );
    let (mut ws, res) = tokio_tungstenite::connect_async(req).await.unwrap();
    assert_eq!(res.status(), 101);
    assert_eq!(res.headers()["sec-websocket-protocol"], "vite-hmr");

    let hello: Value = match ws.next().await.unwrap().unwrap() {
        Message::Text(t) => serde_json::from_str(t.as_str()).unwrap(),
        m => panic!("unexpected {m:?}"),
    };
    assert_eq!(hello["type"], "connected");
    // Origin reaches the dev server untouched; the preview cookie does not.
    assert_eq!(hello["origin"], format!("http://{}", env.preview));
    assert_eq!(hello["cookie"], "keep=1");

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
        env.preview,
        "GET",
        "/hmr",
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
async fn the_app_learns_the_origin_from_the_api() {
    let env = env(Some(TOKEN)).await;
    let r = raw(env.daemon, "GET", "/api/projects/site/preview", &[], "").await;
    assert_eq!(r.status, 401, "only an authenticated app gets the token");
    let r = raw(
        env.daemon,
        "GET",
        "/api/projects/site/preview",
        &[&format!("Authorization: Bearer {TOKEN}")],
        "",
    )
    .await;
    assert_eq!(r.status, 200);
    assert_eq!(
        r.json(),
        json!({ "port": env.access.port, "token": env.access.token })
    );
    let r = raw(
        env.daemon,
        "GET",
        "/api/projects/nope/preview",
        &[&format!("Authorization: Bearer {TOKEN}")],
        "",
    )
    .await;
    assert_eq!(r.status, 404);
}

#[tokio::test]
async fn old_preview_links_redirect_to_the_origin() {
    let env = env(Some(TOKEN)).await;
    let r = raw(env.daemon, "GET", "/preview/site/about/?x=1", &[], "").await;
    assert_eq!(
        r.status, 401,
        "the redirect hands out a token, so it needs auth"
    );
    let r = raw(
        env.daemon,
        "GET",
        &format!("/preview/site/about/?x=1&t={TOKEN}"),
        &["Host: orso.localhost:7800"],
        "",
    )
    .await;
    assert_eq!(r.status, 302);
    let token = env.access.token.clone().unwrap();
    assert_eq!(
        r.headers["location"],
        format!(
            "http://orso.localhost:{}/about/?x=1&t={token}",
            env.access.port
        ),
        "same host as the app, the daemon token dropped, the origin's own added"
    );
    let r = raw(
        env.daemon,
        "GET",
        "/preview/nope/",
        &[&format!("Authorization: Bearer {TOKEN}")],
        "",
    )
    .await;
    assert_eq!(r.status, 404);
}

#[tokio::test]
async fn dead_dev_server_is_503_while_starting_then_502() {
    let env = env(None).await;
    // A project whose dev server never came up (nothing listens on its port).
    env.state.projects.create("down").await.unwrap();
    let down = env.state.preview_origins.ensure("down").await.unwrap();
    let addr = SocketAddr::from(([127, 0, 0, 1], down.port));
    let r = raw(addr, "GET", "/", &[], "").await;
    assert_eq!(r.status, 503, "still starting");
    assert_eq!(r.json()["error"]["code"], "DEVSERVER_ERR");

    env.state
        .projects
        .set_devserver("down", DevserverStatus::Crashed)
        .await;
    let r = raw(addr, "GET", "/x.js", &[], "").await;
    assert_eq!(r.status, 502);
    assert_eq!(r.json()["error"]["code"], "DEVSERVER_ERR");
}

#[tokio::test]
async fn large_request_body_survives_the_proxy() {
    let env = env(None).await;
    let payload = "x".repeat(300_000);
    let r = raw(env.preview, "POST", "/echo", &[], &payload).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.json()["body"].as_str().unwrap().len(), payload.len());
}

#[tokio::test]
async fn releasing_an_origin_frees_its_port() {
    let env = env(None).await;
    assert_eq!(
        env.state.preview_origins.ensure("site").await.unwrap(),
        env.access,
        "ensure reuses the open origin"
    );
    env.state.preview_origins.release("site").await;
    assert_eq!(env.state.preview_origins.port("site").await, None);
    let rebound = TcpListener::bind(env.preview).await;
    assert!(rebound.is_ok(), "{rebound:?}");
}

#[tokio::test]
async fn supervision_opens_the_origin_and_stopping_closes_it() {
    let tmp = tempfile::tempdir().unwrap();
    let opts = DevServerOptions::custom(|_| {
        let mut c = tokio::process::Command::new("sleep");
        c.arg("30");
        c
    });
    let state = AppState::with_options(config(tmp.path(), Some(TOKEN)), false, opts);
    state.projects.create("site").await.unwrap();
    state.devservers.start("site").await.unwrap();
    let port = state
        .preview_origins
        .port("site")
        .await
        .expect("origin open");
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    assert_eq!(raw(addr, "GET", "/", &[], "").await.status, 401);

    state.devservers.stop("site").await;
    assert_eq!(state.preview_origins.port("site").await, None);
    assert!(TcpStream::connect(addr).await.is_err(), "port closed");
}

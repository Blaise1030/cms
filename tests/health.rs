use std::path::PathBuf;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;
use workbench::config::Config;
use workbench::http;
use workbench::state::AppState;

fn config(no_auth: bool, token: Option<&str>, origin: Option<&str>) -> Config {
    Config {
        port: 0,
        origin: origin.map(str::to_string),
        projects_dir: PathBuf::from("/tmp/workbench-test-projects"),
        no_auth,
        token: token.map(str::to_string),
        agents_config: None,
        ..Default::default()
    }
}

fn app(cfg: Config) -> axum::Router {
    http::router(AppState::new(cfg))
}

async fn json_of(res: axum::response::Response) -> Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::HOST, "localhost:7800")
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn health_over_real_socket_returns_200_json() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = AppState::new(config(false, Some("abc123"), None));
    tokio::spawn(async move {
        workbench::serve_on(listener, state).await.unwrap();
    });

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let req = format!("GET /api/health HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut buf = String::new();
    stream.read_to_string(&mut buf).await.unwrap();

    assert!(buf.starts_with("HTTP/1.1 200"), "got: {buf}");
    assert!(
        buf.to_lowercase().contains("application/json"),
        "got: {buf}"
    );
    let body = buf.split("\r\n\r\n").nth(1).unwrap();
    // chunked or not, the JSON object must be present
    assert!(body.contains("\"status\":\"ok\""), "got: {buf}");
}

#[tokio::test]
async fn health_returns_ok_json_with_version() {
    let res = app(config(true, None, None))
        .oneshot(get("/api/health"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_of(res).await;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["version"], workbench::version());
}

#[tokio::test]
async fn health_needs_no_auth_even_when_auth_enabled() {
    let res = app(config(false, Some("abc123"), None))
        .oneshot(get("/api/health"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn unknown_route_uses_error_envelope() {
    let res = app(config(true, None, None))
        .oneshot(get("/api/nope"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = json_of(res).await;
    assert_eq!(body["error"]["code"], "ENOENT");
    assert!(body["error"]["message"].is_string());
}

#[tokio::test]
async fn protected_route_without_token_is_unauth_envelope() {
    let res = app(config(false, Some("abc123"), None))
        .oneshot(get("/api/nope"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let body = json_of(res).await;
    assert_eq!(body["error"]["code"], "UNAUTH");
}

#[tokio::test]
async fn bearer_token_is_accepted() {
    let req = Request::builder()
        .uri("/api/nope")
        .header(header::HOST, "localhost")
        .header(header::AUTHORIZATION, "Bearer abc123")
        .body(Body::empty())
        .unwrap();
    let res = app(config(false, Some("abc123"), None))
        .oneshot(req)
        .await
        .unwrap();
    // passes auth, then hits fallback 404
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn wrong_bearer_token_is_rejected() {
    let req = Request::builder()
        .uri("/api/nope")
        .header(header::HOST, "localhost")
        .header(header::AUTHORIZATION, "Bearer beef")
        .body(Body::empty())
        .unwrap();
    let res = app(config(false, Some("abc123"), None))
        .oneshot(req)
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn query_token_sets_session_cookie_and_cookie_authenticates() {
    let router = app(config(false, Some("abc123"), None));
    let res = router.clone().oneshot(get("/?t=abc123")).await.unwrap();
    let cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .expect("set-cookie")
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie.starts_with("wb_session=abc123"), "{cookie}");

    let req = Request::builder()
        .uri("/api/nope")
        .header(header::HOST, "localhost")
        .header(header::COOKIE, "wb_session=abc123")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND); // authenticated, no such API route
}

#[tokio::test]
async fn foreign_host_header_is_rejected() {
    let req = Request::builder()
        .uri("/api/health")
        .header(header::HOST, "evil.example.com")
        .body(Body::empty())
        .unwrap();
    let res = app(config(true, None, None)).oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_of(res).await["error"]["code"], "UNAUTH");
}

#[tokio::test]
async fn configured_origin_host_is_allowed() {
    let req = Request::builder()
        .uri("/api/health")
        .header(header::HOST, "app.example.com")
        .body(Body::empty())
        .unwrap();
    let res = app(config(true, None, Some("https://app.example.com")))
        .oneshot(req)
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

async fn assert_forbidden_unauth(res: axum::response::Response) {
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_of(res).await["error"]["code"], "UNAUTH");
}

#[tokio::test]
async fn post_health_uses_error_envelope() {
    let req = Request::builder()
        .method("POST")
        .uri("/api/health")
        .header(header::HOST, "localhost")
        .body(Body::empty())
        .unwrap();
    let res = app(config(true, None, None)).oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert!(res
        .headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap()
        .contains("application/json"));
    let body = json_of(res).await;
    assert_eq!(body["error"]["code"], "BADREQ");
    assert!(body["error"]["message"].is_string());
}

#[tokio::test]
async fn missing_host_is_rejected_even_for_health() {
    let req = Request::builder()
        .uri("/api/health")
        .body(Body::empty())
        .unwrap();
    assert_forbidden_unauth(app(config(true, None, None)).oneshot(req).await.unwrap()).await;
    let req = Request::builder()
        .uri("/api/health")
        .body(Body::empty())
        .unwrap();
    assert_forbidden_unauth(
        app(config(false, Some("abc123"), None))
            .oneshot(req)
            .await
            .unwrap(),
    )
    .await;
}

#[tokio::test]
async fn non_utf8_host_is_rejected() {
    let req = Request::builder()
        .uri("/api/health")
        .header(
            header::HOST,
            header::HeaderValue::from_bytes(b"local\xffhost").unwrap(),
        )
        .body(Body::empty())
        .unwrap();
    assert_forbidden_unauth(app(config(true, None, None)).oneshot(req).await.unwrap()).await;
}

#[tokio::test]
async fn malformed_hosts_are_rejected() {
    for host in [
        "",
        "localhost.evil.com",
        "localhost@evil.com",
        "evil.com:7800",
        "127.0.0.1.evil.com",
    ] {
        let req = Request::builder()
            .uri("/api/health")
            .header(header::HOST, host)
            .body(Body::empty())
            .unwrap();
        assert_forbidden_unauth(app(config(true, None, None)).oneshot(req).await.unwrap()).await;
    }
}

#[test]
fn cli_rejects_token_with_reserved_characters() {
    for bad in ["bad;token", "a b", "x&t=y", "secret"] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_workbench"))
            .args(["serve", "--port", "0", "--token", bad])
            .output()
            .unwrap();
        assert!(!out.status.success(), "accepted {bad:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("hex"),
            "{bad:?}"
        );
    }
}

#[tokio::test]
async fn serve_rejects_invalid_token_in_config() {
    let err = workbench::serve(config(false, Some("a;b"), None))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}

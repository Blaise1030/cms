//! Embedded SPA hosting: shell is public, daemon paths stay protected, SPA fallback works.

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use workbench::config::Config;
use workbench::http;
use workbench::state::AppState;
use workbench::static_files::WebApp;

const TOKEN: &str = "abc123";

fn app() -> axum::Router {
    http::router(AppState::new(Config {
        port: 0,
        origin: None,
        projects_dir: PathBuf::from("/tmp/workbench-test-projects"),
        no_auth: false,
        token: Some(TOKEN.to_string()),
        agents_config: None,
        ..Default::default()
    }))
}

fn req(method: Method, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "localhost:7800")
        .body(Body::empty())
        .unwrap()
}

async fn send(r: Request<Body>) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let res = app().oneshot(r).await.unwrap();
    let (parts, body) = res.into_parts();
    (
        parts.status,
        parts.headers,
        body.collect().await.unwrap().to_bytes().to_vec(),
    )
}

/// The SPA is embedded at compile time from `web/dist`; content tests need it built.
fn built() -> bool {
    let ok = WebApp::get("index.html").is_some();
    if !ok {
        eprintln!("skipping: web/dist not built (run `pnpm -C web build`)");
    }
    ok
}

#[tokio::test]
async fn shell_is_public_but_api_and_preview_still_require_a_token() {
    let (status, _, _) = send(req(Method::GET, "/api/projects")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = send(req(Method::GET, "/preview/x/")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = send(req(Method::GET, "/ws/events")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // Non-GET on a shell path is not public.
    let (status, _, _) = send(req(Method::POST, "/")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// Auth is an allowlist, not a denylist: anything that is not the embedded shell needs a token,
/// so a route added to the router later (or a typo'd one) is never accidentally public.
#[tokio::test]
async fn paths_outside_the_shell_allowlist_require_a_token() {
    for path in [
        "/metrics",
        "/healthz",
        "/events",
        "/admin",
        "/debug/pprof",
        "/apiary",
        "/preview-notes",
        "/some-unknown-route",
        "/secret.txt",
    ] {
        let (status, _, _) = send(req(Method::GET, path)).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "GET {path} must not be public"
        );
        let (status, _, _) = send(req(Method::HEAD, path)).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "HEAD {path} must not be public"
        );
    }
    // With a token the same path gets past auth (SPA fallback / 404), it is not rejected.
    let mut r = req(Method::GET, "/metrics");
    r.headers_mut().insert(
        header::AUTHORIZATION,
        format!("Bearer {TOKEN}").parse().unwrap(),
    );
    let (status, _, _) = send(r).await;
    assert_ne!(status, StatusCode::UNAUTHORIZED);
}

#[test]
fn public_shell_allowlist_is_explicit() {
    use workbench::static_files::is_public_shell_request as public;
    assert!(public(&Method::GET, "/"));
    assert!(public(&Method::HEAD, "/p/some-project"));
    assert!(public(&Method::GET, "/assets/anything.js"));
    assert!(public(&Method::GET, "/icons/icon-192.png"));
    for p in [
        "/api/projects",
        "/ws/events",
        "/preview/x/",
        "/metrics",
        "/healthz",
    ] {
        assert!(!public(&Method::GET, p), "{p}");
    }
    assert!(!public(&Method::POST, "/"));
    assert!(!public(&Method::DELETE, "/assets/x.js"));
}

#[tokio::test]
async fn root_serves_index_html_without_a_token_and_never_caches_it() {
    if !built() {
        return;
    }
    let (status, headers, body) = send(req(Method::GET, "/")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("text/html"));
    assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
    assert!(String::from_utf8_lossy(&body).contains("<div id=\"app\">"));
}

#[tokio::test]
async fn token_link_on_root_sets_session_cookie() {
    if !built() {
        return;
    }
    let (status, headers, _) = send(req(Method::GET, &format!("/?t={TOKEN}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .starts_with("wb_session="));
    // A wrong token on the public shell works but mints no cookie.
    let (status, headers, _) = send(req(Method::GET, "/?t=bad")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers.get(header::SET_COOKIE).is_none());
}

#[tokio::test]
async fn pwa_files_are_served_with_right_types_and_cache_policy() {
    if !built() {
        return;
    }
    let (status, headers, body) = send(req(Method::GET, "/manifest.webmanifest")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .contains("manifest"));
    assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
    serde_json::from_slice::<serde_json::Value>(&body).unwrap();

    let (status, headers, _) = send(req(Method::GET, "/sw.js")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .contains("javascript"));
    assert_eq!(headers[header::CACHE_CONTROL], "no-cache");

    let (status, headers, _) = send(req(Method::GET, "/icons/icon-192.png")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
}

#[tokio::test]
async fn fingerprinted_assets_are_immutable_and_support_etag() {
    if !built() {
        return;
    }
    let asset = WebApp::iter()
        .find(|p| p.starts_with("assets/") && p.ends_with(".css"))
        .expect("a css asset");
    let uri = format!("/{asset}");
    let (status, headers, _) = send(req(Method::GET, &uri)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers[header::CACHE_CONTROL]
        .to_str()
        .unwrap()
        .contains("immutable"));
    let etag = headers[header::ETAG].clone();

    let mut conditional = req(Method::GET, &uri);
    conditional
        .headers_mut()
        .insert(header::IF_NONE_MATCH, etag);
    let (status, _, body) = send(conditional).await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    assert!(body.is_empty());
}

#[tokio::test]
async fn client_routes_fall_back_to_index_but_missing_files_and_daemon_paths_404() {
    if built() {
        let (status, headers, body) = send(req(Method::GET, "/p/some-project")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/html"));
        assert!(String::from_utf8_lossy(&body).contains("<div id=\"app\">"));
        // Project routes can end in a file name (an open file in the Code tab).
        let (status, _, body) = send(req(
            Method::GET,
            "/p/some-project/code/src/pages/index.astro",
        ))
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(String::from_utf8_lossy(&body).contains("<div id=\"app\">"));
    }
    let (status, _, _) = send(req(Method::GET, "/assets/missing-abc.js")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Authenticated unknown API routes keep the JSON envelope, never index.html.
    let mut r = req(Method::GET, "/api/nope");
    r.headers_mut().insert(
        header::AUTHORIZATION,
        format!("Bearer {TOKEN}").parse().unwrap(),
    );
    let (status, headers, body) = send(r).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(headers[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .contains("json"));
    assert!(String::from_utf8_lossy(&body).contains("\"error\""));
}

#[tokio::test]
async fn host_allowlist_still_applies_to_the_shell() {
    let r = Request::builder()
        .uri("/")
        .header(header::HOST, "evil.example.com")
        .body(Body::empty())
        .unwrap();
    let (status, _, _) = send(r).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test]
fn content_editor_template_files_are_embedded() {
    use workbench::static_files::Template;
    for path in [
        "src/content-schemas.mjs",
        "scripts/workbench-content.mjs",
        "src/pages/blog/index.astro",
        "src/pages/blog/[slug].astro",
        "src/pages/help/index.astro",
        "src/pages/help/[slug].astro",
        "src/content/blog/en/welcome.md",
        "src/content/helpDocs/en/getting-started.md",
    ] {
        assert!(
            Template::get(path).is_some(),
            "missing template file: {path}"
        );
    }
}

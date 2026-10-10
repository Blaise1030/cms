//! Reverse proxy `/preview/{id}/*` → `127.0.0.1:{port}` of the project's dev server.
//!
//! The dev server runs with `PUBLIC_BASE=/preview/{id}/`, so page URLs and most assets are
//! under the prefix and are forwarded unchanged (no HTML rewriting). Astro/Vite still
//! injects a few absolute root paths (`/@vite/client`, `/@id/…`, `/src/…`, …); those are
//! handled by [`handle_vite_root`], keyed by a short-lived `wb_vite` cookie set on every
//! successful preview response.
//!
//! Upgrade requests (Vite/Astro HMR WebSocket) are tunnelled at the byte level: the upgrade
//! handshake is forwarded, and after the dev server answers `101` both connections are
//! spliced together, so subprotocols, extensions and frames pass through exactly as sent.
//!
//! The daemon's own credentials (`Authorization`, the `wb_session` / `wb_vite` cookies, the
//! `?t=` query token) are stripped before forwarding: dev-server code never sees them.

use std::time::Duration;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{self, HeaderName};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, Version};
use axum::response::{IntoResponse, Response};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::{TokioExecutor, TokioIo};

use crate::auth;
use crate::error::ApiError;
use crate::projects::DevserverStatus;
use crate::state::AppState;

pub type UpstreamClient = Client<HttpConnector, Body>;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Cookie that pins root-absolute Vite/Astro module requests (`/@vite/client`, …) to the
/// project whose preview last answered. Path=/ so the browser sends it on those root URLs.
pub const VITE_COOKIE: &str = "wb_vite";

/// Paths Astro/Vite serve from the site root even when `base` is `/preview/{id}/`.
pub fn is_vite_dev_path(path: &str) -> bool {
    const PREFIXES: [&str; 5] = ["/@vite", "/@id/", "/@fs/", "/node_modules/", "/src/"];
    PREFIXES.iter().any(|p| {
        path == p.trim_end_matches('/')
            || path.strip_prefix(p).is_some_and(|rest| {
                // `/@vite` matches `/@vite` and `/@vite/…`; the others already end with `/`.
                rest.is_empty() || rest.starts_with('/') || p.ends_with('/')
            })
    })
}

/// HTTP client shared by all preview requests (connection-pooled, loopback only).
pub fn new_client() -> UpstreamClient {
    let mut http = HttpConnector::new();
    http.set_nodelay(true);
    http.set_connect_timeout(Some(CONNECT_TIMEOUT));
    Client::builder(TokioExecutor::new())
        .pool_idle_timeout(Duration::from_secs(2))
        .build(http)
}

/// Router handler for `/preview/{id}`, `/preview/{id}/` and `/preview/{id}/{*rest}`.
pub async fn handle(State(state): State<AppState>, req: Request) -> Response {
    match forward(&state, req, None).await {
        Ok(res) => res,
        Err(e) => e.into_response(),
    }
}

/// Router handler for Astro/Vite root-absolute module URLs (`/@vite/client`, …).
pub async fn handle_vite_root(State(state): State<AppState>, req: Request) -> Response {
    if !is_vite_dev_path(req.uri().path()) {
        return ApiError::enoent("not a vite dev path").into_response();
    }
    let Some(id) = vite_project_id(req.headers()) else {
        return ApiError::badreq(
            "open a project preview first so Vite modules can resolve (missing wb_vite cookie)",
        )
        .into_response();
    };
    match forward(&state, req, Some(id)).await {
        Ok(res) => res,
        Err(e) => e.into_response(),
    }
}

/// `/preview/{id}[/...]` → `id`.
fn project_id(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/preview/")?;
    let id = rest.split('/').next()?;
    (!id.is_empty()).then_some(id)
}

/// Project id from `wb_vite` cookie, else from a `/preview/{id}/…` Referer.
fn vite_project_id(headers: &HeaderMap) -> Option<String> {
    let prefix = format!("{VITE_COOKIE}=");
    for jar in headers.get_all(header::COOKIE) {
        let Ok(s) = jar.to_str() else { continue };
        for part in s.split(';') {
            let part = part.trim();
            if let Some(v) = part.strip_prefix(&prefix) {
                if !v.is_empty() && !v.contains('/') {
                    return Some(v.to_string());
                }
            }
        }
    }
    let referer = headers.get(header::REFERER)?.to_str().ok()?;
    let path = referer
        .strip_prefix("http://")
        .or_else(|| referer.strip_prefix("https://"))
        .and_then(|rest| rest.split_once('/').map(|(_, p)| format!("/{p}")))
        .unwrap_or_else(|| referer.to_string());
    project_id(&path).map(str::to_string)
}

/// `project_override` is set for root Vite paths; otherwise the id comes from the URL.
async fn forward(
    state: &AppState,
    mut req: Request,
    project_override: Option<String>,
) -> Result<Response, ApiError> {
    let id = match project_override {
        Some(id) => id,
        None => project_id(req.uri().path())
            .ok_or_else(|| ApiError::enoent("no such project"))?
            .to_string(),
    };
    let project = state
        .projects
        .get(&id)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {id}")))?;

    let upgrade = is_upgrade(req.headers());
    if upgrade {
        // Cross-site WebSocket hijacking protection, as for `/ws/events`.
        let origin = req
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok());
        if !auth::ws_origin_allowed(origin, state.config.origin.as_deref()) {
            return Err(ApiError::unauth("origin not allowed").with_status(403));
        }
    }
    let client_upgrade = upgrade.then(|| hyper::upgrade::on(&mut req));

    let (parts, body) = req.into_parts();
    let uri = upstream_uri(project.port, &parts.uri)
        .ok_or_else(|| ApiError::badreq("invalid preview URL"))?;
    let mut headers = parts.headers;
    let original_host = headers.get(header::HOST).cloned();
    let upgrade_value = headers.get(header::UPGRADE).cloned();
    strip_hop_by_hop(&mut headers);
    strip_credentials(&mut headers);
    // The dev server sees itself as the origin (its host check accepts loopback only).
    if let Ok(host) = HeaderValue::from_str(&format!("127.0.0.1:{}", project.port)) {
        headers.insert(header::HOST, host);
    }
    if let Some(h) = original_host {
        headers.insert(HeaderName::from_static("x-forwarded-host"), h);
    }
    if let Some(u) = upgrade_value.filter(|_| upgrade) {
        headers.insert(header::CONNECTION, HeaderValue::from_static("upgrade"));
        headers.insert(header::UPGRADE, u);
    }

    let mut up = Request::new(if upgrade { Body::empty() } else { body });
    *up.method_mut() = parts.method;
    *up.uri_mut() = uri;
    *up.version_mut() = Version::HTTP_11;
    *up.headers_mut() = headers;

    let mut res = state.preview.request(up).await.map_err(|e| {
        let msg = format!("dev server for {id} is not reachable: {e}");
        // While (re)starting the preview is expected to come up shortly.
        let starting = project.devserver == DevserverStatus::Starting;
        ApiError::new(crate::error::ErrorCode::DevserverErr, msg).with_status(if starting {
            503
        } else {
            502
        })
    })?;

    if let (Some(client_upgrade), StatusCode::SWITCHING_PROTOCOLS) = (client_upgrade, res.status())
    {
        let upstream_upgrade = hyper::upgrade::on(&mut res);
        let mut out = Response::new(Body::empty());
        *out.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
        *out.headers_mut() = res.headers().clone();
        tokio::spawn(async move {
            let (Ok(client), Ok(upstream)) = (client_upgrade.await, upstream_upgrade.await) else {
                return;
            };
            let mut client = TokioIo::new(client);
            let mut upstream = TokioIo::new(upstream);
            let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
        });
        return Ok(out);
    }

    let (mut parts, body) = res.into_parts();
    strip_hop_by_hop(&mut parts.headers);
    let mut out = Response::from_parts(parts, Body::new(body));
    if out.status() == StatusCode::SWITCHING_PROTOCOLS {
        // Upgrade the client never asked for: not something we can relay.
        *out.status_mut() = StatusCode::BAD_GATEWAY;
    }
    if out.status() == StatusCode::SERVICE_UNAVAILABLE
        && out.headers().get(header::RETRY_AFTER).is_none()
    {
        out.headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
    }
    // Pin subsequent `/@vite/…` fetches to this project (Astro hardcodes those at `/`).
    if out.status().is_success() || out.status().is_redirection() {
        if let Ok(v) = HeaderValue::from_str(&format!("{VITE_COOKIE}={id}; Path=/; SameSite=Lax")) {
            out.headers_mut().append(header::SET_COOKIE, v);
        }
    }
    Ok(out)
}

/// `http://127.0.0.1:{port}{path}?{query minus t=}`.
fn upstream_uri(port: u16, uri: &Uri) -> Option<Uri> {
    let mut pq = uri.path().to_string();
    if let Some(q) = uri.query() {
        let kept: Vec<&str> = q.split('&').filter(|kv| !kv.starts_with("t=")).collect();
        if !kept.is_empty() {
            pq.push('?');
            pq.push_str(&kept.join("&"));
        }
    }
    Uri::builder()
        .scheme("http")
        .authority(format!("127.0.0.1:{port}"))
        .path_and_query(pq)
        .build()
        .ok()
}

fn is_upgrade(headers: &HeaderMap) -> bool {
    headers.contains_key(header::UPGRADE)
        && headers
            .get_all(header::CONNECTION)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(','))
            .any(|t| t.trim().eq_ignore_ascii_case("upgrade"))
}

const HOP_BY_HOP: [&str; 9] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "proxy-connection",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

/// Drop hop-by-hop headers, including any named by `Connection`.
fn strip_hop_by_hop(headers: &mut HeaderMap) {
    let named: Vec<HeaderName> = headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(|t| HeaderName::from_bytes(t.trim().as_bytes()).ok())
        .collect();
    for n in named {
        headers.remove(n);
    }
    for n in HOP_BY_HOP {
        headers.remove(n);
    }
}

/// Remove the daemon's auth material from a request headed for the dev server.
fn strip_credentials(headers: &mut HeaderMap) {
    headers.remove(header::AUTHORIZATION);
    let session = format!("{}=", auth::SESSION_COOKIE);
    let vite = format!("{VITE_COOKIE}=");
    let kept: Vec<String> = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .map(str::trim)
        .filter(|c| !c.is_empty() && !c.starts_with(&session) && !c.starts_with(&vite))
        .map(str::to_string)
        .collect();
    headers.remove(header::COOKIE);
    if !kept.is_empty() {
        if let Ok(v) = HeaderValue::from_str(&kept.join("; ")) {
            headers.insert(header::COOKIE, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_from_path() {
        assert_eq!(project_id("/preview/site"), Some("site"));
        assert_eq!(project_id("/preview/site/"), Some("site"));
        assert_eq!(project_id("/preview/site/a/b.js"), Some("site"));
        assert_eq!(project_id("/preview/"), None);
        assert_eq!(project_id("/other/site"), None);
    }

    #[test]
    fn token_query_is_stripped() {
        let u: Uri = "/preview/a/x?t=abc&y=1&tt=2".parse().unwrap();
        assert_eq!(
            upstream_uri(9, &u).unwrap().to_string(),
            "http://127.0.0.1:9/preview/a/x?y=1&tt=2"
        );
        let u: Uri = "/preview/a/?t=abc".parse().unwrap();
        assert_eq!(
            upstream_uri(9, &u).unwrap().to_string(),
            "http://127.0.0.1:9/preview/a/"
        );
    }

    #[test]
    fn credentials_are_stripped() {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, "Bearer x".parse().unwrap());
        h.insert(header::COOKIE, "a=1; wb_session=tok; b=2".parse().unwrap());
        strip_credentials(&mut h);
        assert!(h.get(header::AUTHORIZATION).is_none());
        assert_eq!(h.get(header::COOKIE).unwrap(), "a=1; b=2");
        h.insert(header::COOKIE, "wb_session=tok".parse().unwrap());
        strip_credentials(&mut h);
        assert!(h.get(header::COOKIE).is_none());
    }

    #[test]
    fn upgrade_detection() {
        let mut h = HeaderMap::new();
        h.insert(header::UPGRADE, "websocket".parse().unwrap());
        assert!(!is_upgrade(&h));
        h.insert(header::CONNECTION, "keep-alive, Upgrade".parse().unwrap());
        assert!(is_upgrade(&h));
    }

    #[test]
    fn vite_dev_paths() {
        assert!(is_vite_dev_path("/@vite/client"));
        assert!(is_vite_dev_path(
            "/@id/astro/runtime/client/dev-toolbar/entrypoint.js"
        ));
        assert!(is_vite_dev_path("/@fs/Users/x/project/foo.ts"));
        assert!(is_vite_dev_path("/node_modules/vite/dist/client/env.mjs"));
        assert!(is_vite_dev_path("/src/layouts/Base.astro"));
        assert!(!is_vite_dev_path("/preview/site/"));
        assert!(!is_vite_dev_path("/assets/index.js"));
        assert!(!is_vite_dev_path("/api/health"));
    }

    #[test]
    fn vite_project_from_cookie_or_referer() {
        let mut h = HeaderMap::new();
        h.insert(header::COOKIE, "a=1; wb_vite=site-1; b=2".parse().unwrap());
        assert_eq!(vite_project_id(&h).as_deref(), Some("site-1"));
        h.clear();
        h.insert(
            header::REFERER,
            "http://127.0.0.1:7800/preview/site-2/@vite/client"
                .parse()
                .unwrap(),
        );
        assert_eq!(vite_project_id(&h).as_deref(), Some("site-2"));
    }
}

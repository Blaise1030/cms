//! Preview origins: every preview is served at the root of an origin of its own.
//!
//! Each running dev server gets a loopback listener on a free port that reverse-proxies
//! everything at `/` to it, and each built snapshot (conflict previews) gets one that
//! serves its files. A site therefore runs with base `/`, exactly as in production: plain
//! root-relative URLs in Markdown, components, islands, scripts and CSS all resolve, and
//! sites need no orso-specific link handling. (Previously everything lived under
//! `/preview/{id}/` on the daemon's own port, which every site had to work around.)
//!
//! The origin is not the daemon's, so the daemon session does not carry over. Each origin
//! has its own random token: the app gets it from an authenticated API call, the first load
//! carries it as `?t=` (answered with a redirect that drops it, so the page never sees it),
//! and the origin mints its own `HttpOnly` cookie for the rest. Daemon credentials are
//! accepted too, so a request from the daemon's own pages or tests still works. Every request
//! is checked like the main port (Host allowlist, token) and loses all credentials
//! (`Authorization`, the daemon and preview cookies, `?t=`) before it reaches site code.
//!
//! Upgrade requests (Vite/Astro HMR WebSocket) are tunnelled at the byte level: the upgrade
//! handshake is forwarded, and after the dev server answers `101` both connections are
//! spliced together, so subprotocols, extensions and frames pass through exactly as sent.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{self, HeaderName};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, Version};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::{TokioExecutor, TokioIo};
use serde::Serialize;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::auth;
use crate::config::Config;
use crate::error::ApiError;
use crate::projects::{DevserverStatus, ProjectRegistry};
use crate::state::AppState;

pub type UpstreamClient = Client<HttpConnector, Body>;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Query parameter carrying a preview origin's token on first load.
pub const TOKEN_PARAM: &str = "t";

/// Prefix of the cookie a preview origin mints for itself. Browsers scope cookies by host,
/// not port, so every origin on the same host sees every other's cookie: the port in the
/// name keeps them apart.
pub const COOKIE_PREFIX: &str = "orso_preview_";

/// HTTP client shared by all preview requests (connection-pooled, loopback only).
pub fn new_client() -> UpstreamClient {
    let mut http = HttpConnector::new();
    http.set_nodelay(true);
    http.set_connect_timeout(Some(CONNECT_TIMEOUT));
    Client::builder(TokioExecutor::new())
        .pool_idle_timeout(Duration::from_secs(2))
        .build(http)
}

/// What the app needs to frame a preview origin: its port (on the host the app already
/// uses for the daemon, so the origin's cookie stays same-site) and, unless the daemon runs
/// without auth, the token for the first load.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PreviewAccess {
    pub port: u16,
    pub token: Option<String>,
}

/// The app origins the preview bridge may talk to: the daemon itself under each loopback
/// name, the dev UI (`--ui-url`) and the hosted app (`--origin`). Handed to dev servers as
/// `ORSO_APP_ORIGINS` and used as `postMessage` targets by snapshot pages.
pub fn app_origins(config: &Config) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |o: String| {
        if !o.is_empty() && !out.contains(&o) {
            out.push(o);
        }
    };
    if config.port > 0 {
        for host in [config.hostname.as_str(), "127.0.0.1", "localhost", "[::1]"] {
            if !host.is_empty() {
                add(format!("http://{host}:{}", config.port));
            }
        }
    }
    for url in [config.ui_url.as_deref(), config.origin.as_deref()]
        .into_iter()
        .flatten()
    {
        if let Some(origin) = origin_of(url) {
            add(origin);
        }
    }
    out
}

/// `scheme://authority` of an absolute URL.
fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.trim().split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    (!authority.is_empty()).then(|| format!("{}://{authority}", scheme.to_ascii_lowercase()))
}

// --- One origin -------------------------------------------------------------------------------

/// A loopback listener serving an app at the root of its own origin, behind the preview
/// gate. Dropping it closes the listener and frees the port.
pub struct OriginServer {
    port: u16,
    token: String,
    no_auth: bool,
    task: JoinHandle<()>,
}

impl OriginServer {
    /// Bind `127.0.0.1` on a free port and serve `app` there behind the gate.
    pub async fn bind(config: Arc<Config>, app: Router) -> std::io::Result<Self> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
        let port = listener.local_addr()?.port();
        let token = crate::config::random_token();
        let gate = Arc::new(Gate {
            no_auth: config.no_auth,
            config,
            token: token.clone(),
            cookie: format!("{COOKIE_PREFIX}{port}"),
        });
        let no_auth = gate.no_auth;
        let app = app.layer(middleware::from_fn_with_state(gate, guard));
        let task = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("orso: preview origin on port {port} stopped: {e}");
            }
        });
        Ok(Self {
            port,
            token,
            no_auth,
            task,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn access(&self) -> PreviewAccess {
        PreviewAccess {
            port: self.port,
            token: (!self.no_auth).then(|| self.token.clone()),
        }
    }

    /// Stop serving and wait until the listener is closed, so the port is free on return.
    /// (Dropping only schedules that.) Upgraded connections already tunnelled run on until
    /// either side hangs up.
    pub async fn close(mut self) {
        self.task.abort();
        let _ = (&mut self.task).await;
    }
}

impl Drop for OriginServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct Gate {
    config: Arc<Config>,
    no_auth: bool,
    token: String,
    cookie: String,
}

/// Every request to a preview origin: Host allowlist, then auth, then credential stripping.
async fn guard(State(gate): State<Arc<Gate>>, mut req: Request, next: Next) -> Response {
    let host_ok = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|h| auth::host_allowed(h, gate.config.origin.as_deref()));
    if !host_ok {
        return ApiError::unauth("host not allowed")
            .with_status(403)
            .into_response();
    }
    let via_query =
        query_value(req.uri(), TOKEN_PARAM).is_some_and(|t| auth::ct_eq(t, &gate.token));
    let ok = gate.no_auth
        || via_query
        || cookie_values(req.headers(), &gate.cookie).any(|t| auth::ct_eq(t, &gate.token))
        || daemon_credentials(req.headers(), gate.config.token.as_deref());
    if !ok {
        return ApiError::unauth("missing or invalid preview token").into_response();
    }
    let cookie = via_query
        .then(|| {
            HeaderValue::from_str(&format!(
                "{}={}; Path=/; HttpOnly; SameSite=Lax",
                gate.cookie, gate.token
            ))
            .ok()
        })
        .flatten();

    let clean = without_token(req.uri());
    // A page load that carried the token is sent on without it, so the site never sees the
    // token in its own address (and cannot hand it on through links or `Referer`).
    if via_query
        && matches!(*req.method(), Method::GET | Method::HEAD)
        && !is_upgrade(req.headers())
    {
        let location = clean
            .path_and_query()
            .map_or("/", |pq| pq.as_str())
            .to_string();
        let mut res = Response::new(Body::empty());
        *res.status_mut() = StatusCode::FOUND;
        if let Ok(v) = HeaderValue::from_str(&location) {
            res.headers_mut().insert(header::LOCATION, v);
        }
        res.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        if let Some(c) = cookie {
            res.headers_mut().append(header::SET_COOKIE, c);
        }
        return res;
    }
    *req.uri_mut() = clean;
    strip_credentials(req.headers_mut());
    let mut res = next.run(req).await;
    if let Some(c) = cookie {
        res.headers_mut().append(header::SET_COOKIE, c);
    }
    res
}

/// A valid daemon token as `Authorization: Bearer` or the `wb_session` cookie.
fn daemon_credentials(headers: &HeaderMap, expected: Option<&str>) -> bool {
    let Some(expected) = expected else {
        return false;
    };
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .is_some_and(|t| auth::ct_eq(t, expected));
    bearer || cookie_values(headers, auth::SESSION_COOKIE).any(|t| auth::ct_eq(t, expected))
}

fn query_value<'a>(uri: &'a Uri, name: &str) -> Option<&'a str> {
    uri.query()?.split('&').find_map(|kv| {
        kv.split_once('=')
            .filter(|(k, _)| *k == name)
            .map(|(_, v)| v)
    })
}

fn cookie_values<'a>(headers: &'a HeaderMap, name: &'a str) -> impl Iterator<Item = &'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(move |c| {
            c.trim()
                .split_once('=')
                .filter(|(k, _)| *k == name)
                .map(|(_, v)| v)
        })
}

/// `uri` without the `t=` parameter (other parameters keep their order).
fn without_token(uri: &Uri) -> Uri {
    let Some(q) = uri.query() else {
        return uri.clone();
    };
    let prefix = format!("{TOKEN_PARAM}=");
    let kept: Vec<&str> = q
        .split('&')
        .filter(|kv| !kv.starts_with(&prefix) && *kv != TOKEN_PARAM)
        .collect();
    let mut pq = uri.path().to_string();
    if !kept.is_empty() {
        pq.push('?');
        pq.push_str(&kept.join("&"));
    }
    let mut parts = uri.clone().into_parts();
    parts.path_and_query = pq.parse().ok();
    Uri::from_parts(parts).unwrap_or_else(|_| uri.clone())
}

// --- Dev-server origins -----------------------------------------------------------------------

/// The preview origin of each project whose dev server orso supervises. An origin lives from
/// the first `ensure` until the project's supervision stops for good (project deleted, daemon
/// shutting down), so restarts of the dev server keep the same address.
pub struct PreviewOrigins {
    config: Arc<Config>,
    projects: Arc<ProjectRegistry>,
    client: UpstreamClient,
    live: Mutex<HashMap<String, OriginServer>>,
}

impl PreviewOrigins {
    pub fn new(
        config: Arc<Config>,
        projects: Arc<ProjectRegistry>,
        client: UpstreamClient,
    ) -> Self {
        Self {
            config,
            projects,
            client,
            live: Mutex::default(),
        }
    }

    /// The project's preview origin, started on first use.
    pub async fn ensure(&self, id: &str) -> Result<PreviewAccess, ApiError> {
        if self.projects.get(id).await.is_none() {
            return Err(ApiError::enoent(format!("no such project: {id}")));
        }
        let mut live = self.live.lock().await;
        if let Some(origin) = live.get(id) {
            return Ok(origin.access());
        }
        let proxy = Arc::new(Proxy {
            id: id.to_string(),
            projects: self.projects.clone(),
            client: self.client.clone(),
            origin: self.config.origin.clone(),
        });
        let app = Router::new().fallback(forward).with_state(proxy);
        let origin = OriginServer::bind(self.config.clone(), app)
            .await
            .map_err(|e| ApiError::io(format!("cannot open a preview port: {e}")))?;
        let access = origin.access();
        live.insert(id.to_string(), origin);
        Ok(access)
    }

    /// The project's preview port, if its origin is open.
    pub async fn port(&self, id: &str) -> Option<u16> {
        self.live.lock().await.get(id).map(OriginServer::port)
    }

    /// Close the project's origin and free its port.
    pub async fn release(&self, id: &str) {
        let origin = self.live.lock().await.remove(id);
        if let Some(origin) = origin {
            origin.close().await;
        }
    }

    /// Close every origin (daemon shutdown).
    pub async fn shutdown(&self) {
        let all: Vec<OriginServer> = self.live.lock().await.drain().map(|(_, o)| o).collect();
        for origin in all {
            origin.close().await;
        }
    }
}

struct Proxy {
    id: String,
    projects: Arc<ProjectRegistry>,
    client: UpstreamClient,
    /// `--origin`, allowed as a WebSocket `Origin` besides loopback pages.
    origin: Option<String>,
}

/// Forward one request (or upgrade) to the project's dev server, path and query unchanged.
async fn forward(State(proxy): State<Arc<Proxy>>, req: Request) -> Response {
    match forward_inner(&proxy, req).await {
        Ok(res) => res,
        Err(e) => e.into_response(),
    }
}

async fn forward_inner(proxy: &Proxy, mut req: Request) -> Result<Response, ApiError> {
    let id = &proxy.id;
    let project = proxy
        .projects
        .get(id)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {id}")))?;

    let upgrade = is_upgrade(req.headers());
    if upgrade {
        // Cross-site WebSocket hijacking protection, as for `/ws/events`.
        let origin = req
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok());
        if !auth::ws_origin_allowed(origin, proxy.origin.as_deref()) {
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
    // The guard already did this; the proxy must hold even if mounted without it.
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

    let mut res = proxy.client.request(up).await.map_err(|e| {
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
    Ok(out)
}

/// `http://127.0.0.1:{port}{path}?{query minus t=}`.
fn upstream_uri(port: u16, uri: &Uri) -> Option<Uri> {
    let clean = without_token(uri);
    let pq = clean.path_and_query().map_or("/", |pq| pq.as_str());
    Uri::builder()
        .scheme("http")
        .authority(format!("127.0.0.1:{port}"))
        .path_and_query(pq)
        .build()
        .ok()
}

// --- Old links ----------------------------------------------------------------------------------

/// `/preview/{id}/…` on the daemon port, where previews used to live: an authenticated
/// redirect to the same page on the project's own origin (with its token for the first
/// load), so old links and bookmarks keep working.
pub async fn redirect_legacy(State(state): State<AppState>, req: Request) -> Response {
    let Some((id, rest)) = legacy_path(req.uri().path()) else {
        return ApiError::enoent("no such project").into_response();
    };
    let access = match state.preview_origins.ensure(id).await {
        Ok(a) => a,
        Err(e) => return e.into_response(),
    };
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(host_name)
        .unwrap_or("127.0.0.1");
    let mut query: Vec<String> = without_token(req.uri())
        .query()
        .map(|q| q.split('&').map(str::to_string).collect())
        .unwrap_or_default();
    if let Some(t) = &access.token {
        query.push(format!("{TOKEN_PARAM}={t}"));
    }
    let mut location = format!("http://{host}:{}/{rest}", access.port);
    if !query.is_empty() {
        location.push('?');
        location.push_str(&query.join("&"));
    }
    let mut res = Response::new(Body::empty());
    *res.status_mut() = StatusCode::FOUND;
    if let Ok(v) = HeaderValue::from_str(&location) {
        res.headers_mut().insert(header::LOCATION, v);
    }
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

/// `/preview/{id}[/rest]` → `(id, rest)`.
fn legacy_path(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix("/preview/")?;
    let (id, rest) = rest.split_once('/').unwrap_or((rest, ""));
    (!id.is_empty()).then_some((id, rest))
}

/// The host of a `Host` header without its port (`[::1]` keeps its brackets).
fn host_name(host: &str) -> &str {
    if host.starts_with('[') {
        return host
            .split_once(']')
            .map_or(host, |(h, _)| &host[..=h.len()]);
    }
    host.rsplit_once(':').map_or(host, |(h, _)| h)
}

// --- Shared helpers -----------------------------------------------------------------------------

pub(crate) fn is_upgrade(headers: &HeaderMap) -> bool {
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

/// Remove the daemon's and every preview origin's auth material from a request headed for
/// site code.
fn strip_credentials(headers: &mut HeaderMap) {
    headers.remove(header::AUTHORIZATION);
    let session = format!("{}=", auth::SESSION_COOKIE);
    let kept: Vec<String> = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .map(str::trim)
        .filter(|c| !c.is_empty() && !c.starts_with(&session) && !c.starts_with(COOKIE_PREFIX))
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
    fn token_query_is_stripped() {
        let u: Uri = "/x?t=abc&y=1&tt=2".parse().unwrap();
        assert_eq!(
            upstream_uri(9, &u).unwrap().to_string(),
            "http://127.0.0.1:9/x?y=1&tt=2"
        );
        let u: Uri = "/?t=abc".parse().unwrap();
        assert_eq!(
            upstream_uri(9, &u).unwrap().to_string(),
            "http://127.0.0.1:9/"
        );
    }

    #[test]
    fn credentials_are_stripped() {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, "Bearer x".parse().unwrap());
        h.insert(
            header::COOKIE,
            "a=1; wb_session=tok; orso_preview_4000=p; b=2"
                .parse()
                .unwrap(),
        );
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
    fn legacy_paths() {
        assert_eq!(legacy_path("/preview/site"), Some(("site", "")));
        assert_eq!(legacy_path("/preview/site/"), Some(("site", "")));
        assert_eq!(
            legacy_path("/preview/site/a/b.js"),
            Some(("site", "a/b.js"))
        );
        assert_eq!(legacy_path("/preview/"), None);
        assert_eq!(legacy_path("/other/site"), None);
    }

    #[test]
    fn host_names() {
        assert_eq!(host_name("orso.localhost:7800"), "orso.localhost");
        assert_eq!(host_name("127.0.0.1"), "127.0.0.1");
        assert_eq!(host_name("[::1]:7800"), "[::1]");
    }

    #[test]
    fn app_origins_cover_loopback_names_ui_and_hosted_app() {
        let config = Config {
            port: 7800,
            ui_url: Some("http://localhost:5173/".into()),
            origin: Some("https://app.example.com".into()),
            ..Default::default()
        };
        assert_eq!(
            app_origins(&config),
            [
                "http://orso.localhost:7800",
                "http://127.0.0.1:7800",
                "http://localhost:7800",
                "http://[::1]:7800",
                "http://localhost:5173",
                "https://app.example.com",
            ]
        );
    }
}

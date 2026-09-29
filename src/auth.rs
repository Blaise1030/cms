//! Token auth and Host-header allowlist.
//!
//! REST: `Authorization: Bearer <token>` or `wb_session` cookie.
//! WS / first-visit: `?t=<token>` query param (also sets the `wb_session` cookie).
//! `GET /api/health` is exempt from token auth.

use axum::extract::{Request, State};
use axum::http::{header, HeaderValue};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::ApiError;
use crate::state::AppState;

pub const SESSION_COOKIE: &str = "wb_session";
const HEALTH_PATH: &str = "/api/health";

fn ct_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn host_only(host: &str) -> &str {
    // Strip port; handle bracketed IPv6.
    if let Some(rest) = host.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest);
    }
    host.rsplit_once(':').map_or(host, |(h, _)| h)
}

fn origin_host(origin: &str) -> &str {
    let no_scheme = origin.split("://").nth(1).unwrap_or(origin);
    host_only(no_scheme.split('/').next().unwrap_or(no_scheme))
}

fn host_allowed(host: &str, origin: Option<&str>) -> bool {
    let h = host_only(host).to_ascii_lowercase();
    if h == "localhost" || h.ends_with(".localhost") || h == "127.0.0.1" || h == "::1" {
        return true;
    }
    if h.parse::<std::net::IpAddr>()
        .is_ok_and(|ip| ip.is_loopback())
    {
        return true;
    }
    origin.is_some_and(|o| origin_host(o).eq_ignore_ascii_case(&h))
}

/// DNS-rebinding protection: reject unknown, missing, or malformed Host headers (fail closed).
pub async fn check_host(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let allowed = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|host| host_allowed(host, state.config.origin.as_deref()));
    if !allowed {
        return ApiError::unauth("host not allowed")
            .with_status(403)
            .into_response();
    }
    next.run(req).await
}

fn query_token(req: &Request) -> Option<String> {
    req.uri()
        .query()?
        .split('&')
        .find_map(|kv| kv.strip_prefix("t=").map(str::to_string))
}

fn bearer_token(req: &Request) -> Option<&str> {
    req.headers()
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn cookie_token(req: &Request) -> Option<&str> {
    let prefix = format!("{SESSION_COOKIE}=");
    req.headers()
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|c| c.trim().strip_prefix(prefix.as_str()))
}

/// Token middleware. Passes everything through when `--no-auth`.
pub async fn require_auth(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let cfg = &state.config;
    if cfg.no_auth || req.uri().path() == HEALTH_PATH {
        return next.run(req).await;
    }
    let Some(expected) = cfg.token.as_deref() else {
        return ApiError::unauth("auth token not configured").into_response();
    };

    let via_query = query_token(&req).is_some_and(|t| ct_eq(&t, expected));
    let ok = via_query
        || bearer_token(&req).is_some_and(|t| ct_eq(t, expected))
        || cookie_token(&req).is_some_and(|t| ct_eq(t, expected));
    if !ok {
        return ApiError::unauth("missing or invalid token").into_response();
    }

    let cookie = if via_query {
        let cookie = format!("{SESSION_COOKIE}={expected}; Path=/; HttpOnly; SameSite=Lax");
        match HeaderValue::from_str(&cookie) {
            Ok(v) => Some(v),
            Err(_) => return ApiError::io("failed to build session cookie").into_response(),
        }
    } else {
        None
    };

    let mut res = next.run(req).await;
    if let Some(v) = cookie {
        res.headers_mut().append(header::SET_COOKIE, v);
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_allowlist() {
        assert!(host_allowed("localhost:7800", None));
        assert!(host_allowed("foo.localhost", None));
        assert!(host_allowed("127.0.0.1:1", None));
        assert!(host_allowed("[::1]:7800", None));
        assert!(!host_allowed("evil.com", None));
        assert!(host_allowed(
            "app.example.com",
            Some("https://app.example.com")
        ));
        assert!(!host_allowed(
            "other.example.com",
            Some("https://app.example.com")
        ));
    }
}

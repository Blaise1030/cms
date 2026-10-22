//! The `POST /__orso` endpoint the orso Astro integration mounts on a project's dev server.

use std::net::IpAddr;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::CONTENT_TYPE;
use reqwest::{StatusCode, Url};
use serde_json::{json, Map, Value};

use crate::error::ApiError;

/// Where to reach a project's running integration. Deliberately has no `Debug` derive:
/// `token` authenticates the daemon to the dev server.
#[derive(Clone)]
pub struct ContentEndpoint {
    pub url: String,
    pub token: String,
}

pub const PREVIEW_REASON: &str = "Start the preview to edit content";
pub const PREVIEW_SAVE_REASON: &str = "Preview is not running; start the preview to save content";

const TOKEN_HEADER: &str = "x-orso-content-token";
const TIMEOUT: Duration = Duration::from_secs(10);
/// The endpoint's own request cap; responses are held to the same bound.
const MAX_BODY: usize = 2 * 1024 * 1024;
const EXCERPT_CHARS: usize = 200;

/// Validate the merged candidate and return the source-preserving patched text.
/// Schema issues become `invalid_schema` with `[{path, message}]` details.
pub async fn patch(
    endpoint: &ContentEndpoint,
    collection: &str,
    source: &str,
    fields: &Map<String, Value>,
    unset: &[String],
    body: Option<&str>,
) -> Result<String, ApiError> {
    let mut request =
        json!({"op":"patch","collection":collection,"source":source,"fields":fields,"unset":unset});
    if let Some(body) = body {
        request["body"] = json!(body);
    }
    let response = post(endpoint, &request).await?;
    serde_json::from_slice::<Value>(&response)
        .ok()
        .and_then(|v| v.get("source").and_then(Value::as_str).map(str::to_string))
        .ok_or_else(|| ApiError::devserver("The preview returned no patched source"))
}

/// One attempt, no retry: a save must not be applied twice.
async fn post(endpoint: &ContentEndpoint, request: &Value) -> Result<Vec<u8>, ApiError> {
    let url = loopback_url(&endpoint.url)?;
    let payload = request.to_string();
    if payload.len() > MAX_BODY {
        return Err(ApiError::badreq("Content exceeds 2 MiB").with_status(413));
    }
    // No proxy and no redirects: the token header must only ever reach the loopback dev server.
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| ApiError::devserver(PREVIEW_SAVE_REASON))?;
    let response = client
        .post(url)
        .header(CONTENT_TYPE, "application/json")
        .header(TOKEN_HEADER, &endpoint.token)
        .body(payload)
        .send()
        .await
        .map_err(transport)?;
    let status = response.status();
    let body = read_capped(response).await?;
    if status == StatusCode::OK {
        Ok(body)
    } else {
        Err(status_error(status, &body))
    }
}

fn loopback_url(raw: &str) -> Result<Url, ApiError> {
    let refused = || ApiError::devserver("The preview endpoint is not on loopback");
    let url = Url::parse(raw).map_err(|_| refused())?;
    let host = url.host_str().ok_or_else(refused)?;
    let ip = host.trim_start_matches('[').trim_end_matches(']');
    let loopback = host == "localhost" || ip.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
    if url.scheme() == "http" && loopback {
        Ok(url)
    } else {
        Err(refused())
    }
}

async fn read_capped(response: reqwest::Response) -> Result<Vec<u8>, ApiError> {
    let too_large = || ApiError::devserver("The preview's response exceeds 2 MiB");
    if response
        .content_length()
        .is_some_and(|n| n > MAX_BODY as u64)
    {
        return Err(too_large());
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(transport)?;
        if body.len() + chunk.len() > MAX_BODY {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn status_error(status: StatusCode, body: &[u8]) -> ApiError {
    let json = serde_json::from_slice::<Value>(body).unwrap_or(Value::Null);
    let message = json
        .get("message")
        .and_then(Value::as_str)
        .map(excerpt)
        .unwrap_or_else(|| excerpt(&String::from_utf8_lossy(body)));
    let unsupported = json.get("error").and_then(Value::as_str) == Some("unsupported_collection");
    match status {
        StatusCode::UNPROCESSABLE_ENTITY => {
            let issues = match json.get("issues") {
                Some(issues @ Value::Array(_)) => issues.clone(),
                _ => json!([]),
            };
            ApiError::badreq("Content does not match the collection schema")
                .with_status(422)
                .with_reason("invalid_schema")
                .with_details(issues)
        }
        StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND if unsupported => {
            ApiError::badreq(message).with_reason("unsupported_collection")
        }
        StatusCode::BAD_REQUEST => ApiError::badreq(message),
        StatusCode::UNAUTHORIZED => {
            ApiError::devserver("The preview session changed; restart the preview to save content")
        }
        _ => ApiError::devserver(format!(
            "The preview returned {status}: {}",
            excerpt(&String::from_utf8_lossy(body))
        )),
    }
}

fn transport(e: reqwest::Error) -> ApiError {
    if e.is_timeout() {
        ApiError::devserver("The preview did not respond within 10 seconds; restart the preview")
    } else {
        ApiError::devserver(PREVIEW_SAVE_REASON)
    }
}

/// At most [`EXCERPT_CHARS`] characters, so an error never carries a whole response.
fn excerpt(text: &str) -> String {
    let mut out: String = text.chars().take(EXCERPT_CHARS).collect();
    if text.chars().nth(EXCERPT_CHARS).is_some() {
        out.push('…');
    }
    out
}

//! The daemon's client for the `POST /__orso` endpoint, against a local fake endpoint.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Router;
use orso::content::integration::{self, ContentEndpoint};
use orso::error::{ApiError, ErrorCode};
use serde_json::{json, Map, Value};

const TOKEN: &str = "test-content-token";

#[derive(Clone)]
struct Recorded {
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
}

type Log = Arc<Mutex<Option<Recorded>>>;

/// Serve `respond` on `127.0.0.1:0` for every request, recording the last one.
async fn fake<F>(respond: F) -> (ContentEndpoint, Log)
where
    F: Fn() -> Response + Clone + Send + Sync + 'static,
{
    let log: Log = Arc::default();
    let seen = log.clone();
    let app = Router::new().fallback(
        move |method: Method, uri: Uri, headers: HeaderMap, body: Bytes| {
            let seen = seen.clone();
            let respond = respond.clone();
            async move {
                *seen.lock().unwrap() = Some(Recorded {
                    method,
                    uri,
                    headers,
                    body,
                });
                respond()
            }
        },
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let endpoint = ContentEndpoint {
        url: format!("http://{addr}/__orso"),
        token: TOKEN.to_string(),
    };
    (endpoint, log)
}

fn reply(status: StatusCode, body: Value) -> impl Fn() -> Response + Clone + Send + Sync {
    move || (status, axum::Json(body.clone())).into_response()
}

fn fields() -> Map<String, Value> {
    json!({ "title": "Next" }).as_object().unwrap().clone()
}

async fn call(endpoint: &ContentEndpoint) -> Result<String, ApiError> {
    integration::patch(endpoint, "blog", "old", &fields(), &[], None).await
}

#[tokio::test]
async fn patch_posts_the_exact_request_and_returns_the_source() {
    let (endpoint, log) = fake(reply(StatusCode::OK, json!({ "source": "next" }))).await;

    let unset = vec!["draft".to_string()];
    let source = integration::patch(&endpoint, "blog", "old", &fields(), &unset, Some("Body"))
        .await
        .unwrap();
    assert_eq!(source, "next");

    let seen = log.lock().unwrap().clone().unwrap();
    assert_eq!(seen.method, Method::POST);
    assert_eq!(seen.uri.path(), "/__orso");
    assert_eq!(seen.headers["x-orso-content-token"], TOKEN);
    assert_eq!(seen.headers["content-type"], "application/json");
    let body: Value = serde_json::from_slice(&seen.body).unwrap();
    assert_eq!(
        body,
        json!({
            "op": "patch",
            "collection": "blog",
            "source": "old",
            "fields": { "title": "Next" },
            "unset": ["draft"],
            "body": "Body",
        })
    );
}

#[tokio::test]
async fn patch_omits_body_when_none() {
    let (endpoint, log) = fake(reply(StatusCode::OK, json!({ "source": "next" }))).await;

    call(&endpoint).await.unwrap();

    let seen = log.lock().unwrap().clone().unwrap();
    let body: Value = serde_json::from_slice(&seen.body).unwrap();
    assert_eq!(
        body,
        json!({
            "op": "patch",
            "collection": "blog",
            "source": "old",
            "fields": { "title": "Next" },
            "unset": [],
        })
    );
}

#[tokio::test]
async fn ok_without_a_string_source_is_a_devserver_error() {
    for body in [json!({}), json!({ "source": 1 }), json!(["next"])] {
        let (endpoint, _) = fake(reply(StatusCode::OK, body)).await;
        let err = call(&endpoint).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::DevserverErr);
    }
}

#[tokio::test]
async fn ok_with_invalid_json_is_a_devserver_error() {
    let (endpoint, _) = fake(|| (StatusCode::OK, "not json").into_response()).await;
    let err = call(&endpoint).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::DevserverErr);
}

#[tokio::test]
async fn invalid_schema_keeps_status_reason_and_issues() {
    let issues = json!([
        { "path": ["title"], "message": "Required" },
        { "path": ["tags", 0], "message": "Expected string" },
    ]);
    let (endpoint, _) = fake(reply(
        StatusCode::UNPROCESSABLE_ENTITY,
        json!({ "error": "invalid_schema", "issues": issues }),
    ))
    .await;

    let err = call(&endpoint).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::Badreq);
    assert_eq!(err.http_status(), 422);
    assert_eq!(err.reason, Some("invalid_schema"));
    assert_eq!(err.details, Some(issues));
}

#[tokio::test]
async fn bad_request_message_becomes_badreq() {
    let (endpoint, _) = fake(reply(
        StatusCode::BAD_REQUEST,
        json!({ "error": "bad_request", "message": "source must be a string" }),
    ))
    .await;

    let err = call(&endpoint).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::Badreq);
    assert_eq!(err.http_status(), 400);
    assert_eq!(err.message, "source must be a string");
    assert_eq!(err.reason, None);
}

#[tokio::test]
async fn unsupported_collection_becomes_badreq_with_reason() {
    for status in [StatusCode::NOT_FOUND, StatusCode::BAD_REQUEST] {
        let message = "Collection \"blog\" is not in the current manifest";
        let (endpoint, _) = fake(reply(
            status,
            json!({ "error": "unsupported_collection", "message": message }),
        ))
        .await;

        let err = call(&endpoint).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::Badreq, "{status}");
        assert_eq!(err.message, message);
        assert_eq!(err.reason, Some("unsupported_collection"));
    }
}

#[tokio::test]
async fn unauthorized_is_a_devserver_error_without_the_token() {
    let (endpoint, _) = fake(reply(
        StatusCode::UNAUTHORIZED,
        json!({ "error": "unauthorized" }),
    ))
    .await;

    let err = call(&endpoint).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::DevserverErr);
    assert!(err.message.contains("preview"), "{}", err.message);
    assert!(!err.message.contains(TOKEN));
}

#[tokio::test]
async fn other_statuses_report_the_status_and_a_bounded_excerpt() {
    let long = "x".repeat(10_000);
    let (endpoint, _) = fake(reply(
        StatusCode::INTERNAL_SERVER_ERROR,
        json!({ "error": "internal", "message": long }),
    ))
    .await;

    let err = call(&endpoint).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::DevserverErr);
    assert!(err.message.contains("500"), "{}", err.message);
    assert!(err.message.contains("internal"), "{}", err.message);
    assert!(err.message.len() < 1_000, "{} bytes", err.message.len());
}

#[tokio::test]
async fn unreachable_endpoint_is_a_devserver_error() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let endpoint = ContentEndpoint {
        url: format!("http://{addr}/__orso"),
        token: TOKEN.to_string(),
    };

    let err = call(&endpoint).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::DevserverErr);
    assert!(err.message.contains("preview"), "{}", err.message);
    assert!(!err.message.contains(TOKEN));
}

#[tokio::test]
async fn non_loopback_endpoint_is_refused_without_a_request() {
    let endpoint = ContentEndpoint {
        url: "http://192.0.2.1/__orso".to_string(),
        token: TOKEN.to_string(),
    };

    let start = Instant::now();
    let err = call(&endpoint).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::DevserverErr);
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[tokio::test]
async fn request_over_two_mib_is_refused_before_sending() {
    let (endpoint, log) = fake(reply(StatusCode::OK, json!({ "source": "next" }))).await;
    let source = "x".repeat(2 * 1024 * 1024);

    let err = integration::patch(&endpoint, "blog", &source, &fields(), &[], None)
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Badreq);
    assert_eq!(err.http_status(), 413);
    assert!(log.lock().unwrap().is_none());
}

#[tokio::test]
async fn response_over_two_mib_is_rejected() {
    let big = json!({ "source": "x".repeat(2 * 1024 * 1024) });
    let (endpoint, _) = fake(reply(StatusCode::OK, big)).await;

    let err = call(&endpoint).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::DevserverErr);
    assert!(err.message.contains("2 MiB"), "{}", err.message);
}

#[tokio::test]
async fn streamed_response_over_two_mib_without_length_is_rejected() {
    let (endpoint, _) = fake(|| {
        let chunks = (0..3).map(|_| Ok::<_, std::io::Error>(Bytes::from(vec![b'x'; 1024 * 1024])));
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from_stream(futures_util::stream::iter(chunks)))
            .unwrap()
    })
    .await;

    let err = call(&endpoint).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::DevserverErr);
    assert!(err.message.contains("2 MiB"), "{}", err.message);
}

#[tokio::test]
#[ignore = "10 second timeout"]
async fn timeout_after_ten_seconds_is_a_devserver_error() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new().fallback(std::future::pending::<Response>);
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let endpoint = ContentEndpoint {
        url: format!("http://{addr}/__orso"),
        token: TOKEN.to_string(),
    };

    let start = Instant::now();
    let err = call(&endpoint).await.unwrap_err();
    let elapsed = start.elapsed();
    assert_eq!(err.code, ErrorCode::DevserverErr);
    assert!(err.message.contains("preview"), "{}", err.message);
    assert!(elapsed >= Duration::from_secs(10), "{elapsed:?}");
    assert!(elapsed < Duration::from_secs(15), "{elapsed:?}");
}

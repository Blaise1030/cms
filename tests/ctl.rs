//! `/api/ctl/*`: hook-token-authenticated app state for agents.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use orso::changes;
use orso::config::Config;
use orso::http;
use orso::http::ctl::{change_status_value, preview_status_value};
use orso::state::AppState;
use serde_json::{json, Value};
use tower::ServiceExt;

fn config() -> Config {
    Config {
        port: 0,
        origin: None,
        no_auth: true,
        token: None,
        agents_config: None,
        ..Default::default()
    }
}

async fn scaffolded(config: Config) -> (tempfile::TempDir, AppState, String) {
    let tmp = tempfile::tempdir().unwrap();
    let projects = tmp.path().join("projects");
    std::fs::create_dir_all(&projects).unwrap();
    let st = AppState::with_scaffold_install(
        Config {
            projects_dir: projects,
            ..config
        },
        false,
    );
    let info = st.projects.create("Site").await.unwrap();
    (tmp, st, info.id)
}

fn post(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::HOST, "localhost")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn call(app: &axum::Router, r: Request<Body>) -> (StatusCode, Value) {
    let res = app.clone().oneshot(r).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn every_ctl_route_refuses_without_a_live_chat() {
    let (_t, st, id) = scaffolded(config()).await;
    let app = http::router(st);
    for route in [
        "preview-status",
        "restart-preview",
        "change-status",
        "refresh-preview",
        "refresh-content",
    ] {
        let (s, body) = call(
            &app,
            post(
                &format!("/api/ctl/{route}"),
                json!({ "project": id, "token": "forged" }),
            ),
        )
        .await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{route}: {body}");
        assert!(body.get("devserver").is_none(), "{route} leaked: {body}");
        assert!(body.get("active").is_none(), "{route} leaked: {body}");
    }
}

#[tokio::test]
async fn ctl_rejects_a_malformed_body() {
    let (_t, st, _id) = scaffolded(config()).await;
    let app = http::router(st);
    let (s, body) = call(&app, post("/api/ctl/change-status", json!({}))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn ctl_skips_daemon_token_auth_but_still_needs_the_hook_token() {
    let (_t, st, id) = scaffolded(Config {
        no_auth: false,
        token: Some("daemon-secret".into()),
        ..config()
    })
    .await;
    let app = http::router(st);
    let (s, body) = call(
        &app,
        post(
            "/api/ctl/preview-status",
            json!({ "project": id, "token": "forged" }),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{body}");

    let (s, body) = call(
        &app,
        post(
            "/api/ctl/other",
            json!({ "project": id, "token": "forged" }),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED, "{body}");
}

#[tokio::test]
async fn preview_status_reports_devserver_port_and_path() {
    let (_t, st, id) = scaffolded(config()).await;
    let port = st.projects.get(&id).await.unwrap().port;
    let v = preview_status_value(&st, &id).await.unwrap();
    assert_eq!(
        v,
        json!({
            "devserver": "starting",
            "port": port,
            "previewPath": format!("/preview/{id}/"),
        })
    );
}

#[tokio::test]
async fn preview_status_of_unknown_project_is_enoent() {
    let (_t, st, _id) = scaffolded(config()).await;
    let err = preview_status_value(&st, "nope").await.unwrap_err();
    assert_eq!(err.http_status(), 404);
}

#[tokio::test]
async fn change_status_on_main_has_no_active_change() {
    let (_t, st, id) = scaffolded(config()).await;
    let v = change_status_value(&st, &id).await.unwrap();
    assert_eq!(v["active"], Value::Null, "{v}");
    assert_eq!(v["dirty"], false, "{v}");
    assert_eq!(v["changedPaths"], json!([]), "{v}");
}

#[tokio::test]
async fn change_status_reports_active_change_and_unsaved_edits() {
    let (_t, st, id) = scaffolded(config()).await;
    let dir = st.projects.get(&id).await.unwrap().dir;
    changes::create(&dir, "Launch week").await.unwrap();
    std::fs::write(dir.join("launch.md"), "hi\n").unwrap();

    let v = change_status_value(&st, &id).await.unwrap();
    assert_eq!(v["active"]["title"], "Launch week", "{v}");
    assert_eq!(v["dirty"], true, "{v}");
    assert_eq!(v["changedPaths"], json!(["launch.md"]), "{v}");
}

/// Start a fake ACP chat, then return the hook token its agent was given.
async fn live_hook_token(st: &AppState, project: &str) -> String {
    let dir = st.projects.get(project).await.unwrap().dir;
    let (cmd, args) = orso::chat::acp::fake_agent();
    let mut argv = vec![cmd];
    argv.extend(args);
    orso::providers::set_test_acp_argv("fake", argv.clone());
    st.chats
        .start(project, "ctl-chat", &dir, &argv, &[])
        .await
        .unwrap();
    st.chats.hook_token(project, "ctl-chat").unwrap()
}

async fn next_event_of(
    rx: &mut tokio::sync::broadcast::Receiver<std::sync::Arc<str>>,
    kind: &str,
) -> Value {
    loop {
        let text = rx.recv().await.expect("event bus closed");
        let v: Value = serde_json::from_str(&text).unwrap();
        if v["type"] == kind {
            return v;
        }
    }
}

#[tokio::test]
async fn refresh_routes_emit_ui_events_when_authorized() {
    let (_t, st, id) = scaffolded(config()).await;
    let token = live_hook_token(&st, &id).await;
    let mut rx = st.events.subscribe();
    let app = http::router(st.clone());

    for (route, kind) in [
        ("refresh-preview", "preview.reload"),
        ("refresh-content", "content.refresh"),
    ] {
        let (s, body) = call(
            &app,
            post(
                &format!("/api/ctl/{route}"),
                json!({ "project": id, "token": token }),
            ),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{route}: {body}");
        assert_eq!(body, json!({ "ok": true }), "{route}");
        let ev = next_event_of(&mut rx, kind).await;
        assert_eq!(ev["projectId"], id, "{ev}");
    }

    st.chats.shutdown().await;
}

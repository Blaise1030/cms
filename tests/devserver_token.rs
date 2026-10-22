//! The daemon hands each dev-server `start` a fresh `ORSO_CONTENT_TOKEN` and resolves the
//! project's content endpoint from it.

use std::path::Path;
use std::time::{Duration, Instant};

use orso::config::Config;
use orso::devserver::DevServerOptions;
use orso::projects::DevserverStatus;
use orso::state::AppState;
use tokio::process::Command;

const TIMEOUT: Duration = Duration::from_secs(10);

fn config(dir: &Path) -> Config {
    Config {
        port: 0,
        origin: None,
        projects_dir: dir.to_path_buf(),
        no_auth: true,
        token: None,
        agents_config: None,
        ..Default::default()
    }
}

fn options() -> DevServerOptions {
    DevServerOptions::custom(|spec| {
        let mut c = Command::new("sh");
        c.arg("-c").arg(format!(
            "printf %s \"$ORSO_CONTENT_TOKEN\" > token.txt\n\
             echo \"  Local http://127.0.0.1:{}/\"\n\
             exec sleep 300",
            spec.port
        ));
        c
    })
}

async fn wait_ready(state: &AppState, id: &str) {
    let start = Instant::now();
    loop {
        let p = state.projects.get(id).await.unwrap();
        if p.devserver == DevserverStatus::Ready {
            return;
        }
        assert!(start.elapsed() < TIMEOUT, "dev server for {id} never ready");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn read_token(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("token.txt")).unwrap()
}

#[tokio::test]
async fn each_start_injects_a_fresh_token_and_resolves_the_endpoint() {
    let tmp = tempfile::tempdir().unwrap();
    let state = AppState::with_options(config(tmp.path()), false, options());
    let id = state.projects.create("Site").await.unwrap().id;
    let dir = state.projects.get(&id).await.unwrap().dir;

    assert!(state.content_endpoint(&id).await.unwrap().is_none());
    assert!(state.content_endpoint("missing").await.is_err());

    state.devservers.start(&id).await.unwrap();
    wait_ready(&state, &id).await;
    let token = read_token(&dir);
    assert_eq!(token.len(), 64, "{token:?}");
    assert!(token.bytes().all(|b| b.is_ascii_hexdigit()), "{token:?}");

    let endpoint = state.content_endpoint(&id).await.unwrap().unwrap();
    let port = state.projects.get(&id).await.unwrap().port;
    assert_eq!(endpoint.url, format!("http://127.0.0.1:{port}/__orso"));
    assert!(endpoint.url.ends_with("/__orso"));
    assert_eq!(endpoint.token, token);
    assert_eq!(
        state.devservers.content_token(&id).as_deref(),
        Some(&*token)
    );

    state.devservers.stop(&id).await;
    assert!(state.content_endpoint(&id).await.unwrap().is_none());
    assert!(state.devservers.content_token(&id).is_none());

    std::fs::remove_file(dir.join("token.txt")).unwrap();
    state.devservers.start(&id).await.unwrap();
    wait_ready(&state, &id).await;
    let again = read_token(&dir);
    assert_eq!(again.len(), 64);
    assert_ne!(again, token, "each start generates a new token");
    let endpoint = state.content_endpoint(&id).await.unwrap().unwrap();
    assert_eq!(endpoint.token, again);

    state.devservers.shutdown().await;
    assert!(state.devservers.content_token(&id).is_none());
}

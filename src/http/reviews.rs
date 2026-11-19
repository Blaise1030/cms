//! `GET /api/projects/{id}/reviews`, `GET /api/projects/{id}/reviews/{n}`,
//! `GET /api/projects/{id}/reviews/{n}/diff`, `POST /api/projects/{id}/reviews/{n}/decision`:
//! the review inbox over HTTP.

use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::github::Decision;

use crate::error::ApiError;
use crate::projects::Project;
use crate::state::AppState;
use crate::{review, settings};

const GONE: &str = "This change is no longer waiting";
const OWN_CHANGE: &str = "This is your change";
const NEEDS_EDIT_ACCESS: &str = "You need edit access to this site's repository to greenlight";

/// The project and its `owner/name`; a project without a GitHub remote has nothing to review.
async fn project_repo(state: &AppState, id: &str) -> Result<(Project, String), ApiError> {
    let project = state
        .projects
        .get(id)
        .await
        .ok_or_else(|| ApiError::enoent(format!("no such project: {id}")))?;
    let repo = project.repo.clone().ok_or_else(|| {
        ApiError::badreq("this project has no GitHub remote — open a repository first")
    })?;
    Ok((project, repo))
}

/// `GET /api/projects/{id}/reviews` → `{ reviews: ReviewItem[] }`.
pub async fn list(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let (project, repo) = project_repo(&state, &id).await?;
    let me = state.github.signed_in_login().await?;
    let labels = settings::pr_labels(&project.dir);
    let reviews = state.github.review_inbox(&repo, &labels, &me).await?;
    let open: Vec<u64> = reviews.iter().map(|r| r.number).collect();
    review::prune_refs(&project.dir, &open).await;
    state.review_previews.prune(&project.id, &open).await;
    Ok(Json(json!({ "reviews": reviews })))
}

/// Why `me` may not greenlight or send back a change by `author` (the string the UI shows).
async fn refusal(
    state: &AppState,
    repo: &str,
    author: &str,
    me: &str,
) -> Result<Option<&'static str>, ApiError> {
    if author.eq_ignore_ascii_case(me) {
        Ok(Some(OWN_CHANGE))
    } else if !state.github.can_push(repo).await? {
        Ok(Some(NEEDS_EDIT_ACCESS))
    } else {
        Ok(None)
    }
}

/// `GET /api/projects/{id}/reviews/{n}` → the change plus `{ canDecide, reason }`.
pub async fn detail(
    State(state): State<AppState>,
    Path((id, number)): Path<(String, u64)>,
) -> Result<Json<Value>, ApiError> {
    let (_, repo) = project_repo(&state, &id).await?;
    let me = state.github.signed_in_login().await?;
    let detail = state.github.review_detail(&repo, number, &me).await?;
    if !detail.open || detail.draft {
        return Err(ApiError::enoent(GONE));
    }
    let reason = refusal(&state, &repo, &detail.item.author, &me).await?;
    let mut body = serde_json::to_value(&detail)
        .map_err(|e| ApiError::io(format!("could not encode the change: {e}")))?;
    body["canDecide"] = json!(reason.is_none());
    body["reason"] = json!(reason);
    Ok(Json(body))
}

/// Body of `POST /api/projects/{id}/reviews/{n}/decision`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionBody {
    decision: Decision,
    #[serde(default)]
    comment: Option<String>,
    /// The head commit the reviewer was looking at.
    head_sha: String,
}

/// `POST /api/projects/{id}/reviews/{n}/decision` → `204`: greenlight (approve) or send back
/// (request changes) a change, pinned to the commit the reviewer viewed.
pub async fn decide(
    State(state): State<AppState>,
    Path((id, number)): Path<(String, u64)>,
    Json(body): Json<DecisionBody>,
) -> Result<StatusCode, ApiError> {
    let comment = body
        .comment
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    if body.decision == Decision::SendBack && comment.is_none() {
        return Err(ApiError::badreq(
            "Say what needs changing before sending it back",
        ));
    }
    let (_, repo) = project_repo(&state, &id).await?;
    let me = state.github.signed_in_login().await?;
    let detail = state.github.review_detail(&repo, number, &me).await?;
    if !detail.open || detail.draft {
        return Err(ApiError::enoent(GONE));
    }
    if let Some(reason) = refusal(&state, &repo, &detail.item.author, &me).await? {
        return Err(ApiError::sandbox(reason));
    }
    state
        .github
        .submit_review(&repo, number, &body.head_sha, body.decision, comment)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/projects/{id}/reviews/{n}/diff` → the change's unified diff (`text/plain`).
pub async fn diff(
    State(state): State<AppState>,
    Path((id, number)): Path<(String, u64)>,
) -> Result<Response, ApiError> {
    let (project, _) = project_repo(&state, &id).await?;
    let env = state.github.git_env().await?;
    let head = review::fetch_head(&project.dir, number, &env).await?;
    let text = review::diff(&project.dir, &head).await?;
    Ok(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], text).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, GithubConfig};
    use crate::projects::Source;
    use axum::body::to_bytes;
    use axum::routing::get;
    use axum::Router;
    use std::path::Path as FsPath;
    use std::process::Command;

    fn git(dir: &FsPath, args: &[&str]) {
        let out = Command::new("git")
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}");
    }

    /// A bare origin whose `refs/pull/7/head` edits `a.md`.
    fn origin(root: &FsPath) -> String {
        std::fs::create_dir_all(root).unwrap();
        let bare = root.join("origin.git");
        let work = root.join("seed");
        std::fs::create_dir_all(&bare).unwrap();
        std::fs::create_dir_all(&work).unwrap();
        git(&bare, &["init", "--bare", "-b", "main"]);
        git(&work, &["init", "-b", "main"]);
        git(&work, &["remote", "add", "origin", bare.to_str().unwrap()]);
        std::fs::write(work.join("a.md"), "one\n").unwrap();
        git(&work, &["add", "."]);
        git(&work, &["commit", "-m", "base"]);
        git(&work, &["push", "origin", "main"]);
        git(&work, &["checkout", "-b", "pr"]);
        std::fs::write(work.join("a.md"), "one\ntwo\n").unwrap();
        git(&work, &["commit", "-am", "pr"]);
        git(&work, &["push", "origin", "pr:refs/pull/7/head"]);
        bare.to_str().unwrap().to_string()
    }

    fn pull(number: u64, author: &str, state: &str, draft: bool) -> Value {
        json!({ "number": number, "html_url": format!("u{number}"), "title": "t",
            "state": state, "draft": draft, "labels": [{ "name": "orso" }],
            "user": { "login": author }, "body": "why",
            "head": { "sha": format!("h{number}") } })
    }

    /// Signed in as `ada`, with a mock GitHub and a project cloned from a local origin.
    async fn setup(root: &FsPath, push: bool, repo: Option<&str>) -> (AppState, String) {
        let routes = Router::new()
            .route(
                "/repos/acme/site/pulls",
                get(|| async {
                    axum::Json(json!([
                        pull(7, "bob", "open", false),
                        pull(8, "ada", "open", false),
                        pull(9, "carol", "open", false)
                    ]))
                }),
            )
            .route(
                "/repos/acme/site/pulls/{n}",
                get(|Path(n): Path<u64>| async move {
                    axum::Json(match n {
                        7 => pull(7, "bob", "open", false),
                        8 => pull(8, "ADA", "open", false),
                        9 => pull(9, "bob", "closed", false),
                        _ => pull(n, "bob", "open", true),
                    })
                }),
            )
            .route(
                "/repos/acme/site/pulls/{n}/reviews",
                get(|| async { axum::Json(json!([])) })
                    .post(|| async { axum::Json(json!({ "id": 1 })) }),
            )
            .route(
                "/repos/acme/site/commits/{sha}/check-runs",
                get(|| async { axum::Json(json!({ "check_runs": [] })) }),
            )
            .route(
                "/repos/acme/site",
                get(move || async move {
                    axum::Json(json!({ "full_name": "acme/site", "private": false,
                        "default_branch": "main", "permissions": { "push": push } }))
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, routes).await.ok() });

        let state_dir = root.join(crate::projects::STATE_DIR);
        std::fs::create_dir_all(&state_dir).unwrap();
        std::fs::write(
            state_dir.join("github.json"),
            json!({ "token": "tok",
                "user": { "login": "ada", "id": 42, "name": null, "avatarUrl": "" } })
            .to_string(),
        )
        .unwrap();
        let config = Config {
            projects_dir: root.to_path_buf(),
            no_auth: true,
            github: GithubConfig {
                api_url: base.clone(),
                web_url: base.clone(),
                git_url: base,
                gh_path: std::path::PathBuf::new(),
                ..GithubConfig::default()
            },
            ..Default::default()
        };
        let state = AppState::with_scaffold_install(config, false);
        let id = match repo {
            Some(r) => {
                let url = origin(&root.join("o"));
                state
                    .projects
                    .create_from(
                        "Site",
                        Source::Clone {
                            repo: r.to_string(),
                            url,
                            env: vec![],
                        },
                        &crate::git::Identity::orso(),
                    )
                    .await
                    .unwrap()
                    .id
            }
            None => state.projects.create("Site").await.unwrap().id,
        };
        (state, id)
    }

    fn decision(d: Decision, comment: Option<&str>, head: &str) -> Json<DecisionBody> {
        Json(DecisionBody {
            decision: d,
            comment: comment.map(String::from),
            head_sha: head.into(),
        })
    }

    async fn body_json(r: Result<Json<Value>, ApiError>) -> Value {
        r.unwrap().0
    }

    #[tokio::test]
    async fn list_returns_only_labelled_changes_by_others() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let v = body_json(list(State(state), Path(id)).await).await;
        let nums: Vec<u64> = v["reviews"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["number"].as_u64().unwrap())
            .collect();
        assert_eq!(nums, vec![7, 9]);
        assert_eq!(v["reviews"][0]["author"], "bob");
        assert_eq!(v["reviews"][0]["headSha"], "h7");
    }

    #[tokio::test]
    async fn detail_of_own_change_cannot_be_decided() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let v = body_json(detail(State(state), Path((id, 8))).await).await;
        assert_eq!(v["canDecide"], false);
        assert_eq!(v["reason"], "This is your change");
    }

    #[tokio::test]
    async fn detail_without_push_access_explains_why() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), false, Some("acme/site")).await;
        let v = body_json(detail(State(state), Path((id, 7))).await).await;
        assert_eq!(v["canDecide"], false);
        assert_eq!(
            v["reason"],
            "You need edit access to this site's repository to greenlight"
        );
        assert_eq!(v["description"], "why");
    }

    #[tokio::test]
    async fn detail_with_push_access_can_be_decided() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let v = body_json(detail(State(state), Path((id, 7))).await).await;
        assert_eq!(v["canDecide"], true);
        assert!(v["reason"].is_null());
        assert_eq!(v["number"], 7);
    }

    #[tokio::test]
    async fn detail_of_closed_or_draft_change_is_gone() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        for n in [9, 10] {
            let err = detail(State(state.clone()), Path((id.clone(), n)))
                .await
                .unwrap_err();
            assert_eq!(err.message, "This change is no longer waiting");
            assert_eq!(err.code.status(), 404);
        }
    }

    #[tokio::test]
    async fn project_without_github_remote_is_a_bad_request() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, None).await;
        let err = list(State(state), Path(id)).await.unwrap_err();
        assert_eq!(
            err.message,
            "this project has no GitHub remote — open a repository first"
        );
        assert_eq!(err.code.status(), 400);
    }

    #[tokio::test]
    async fn diff_is_plain_text_with_the_changed_path() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let res = diff(State(state), Path((id, 7))).await.unwrap();
        assert_eq!(
            res.headers()[header::CONTENT_TYPE],
            "text/plain; charset=utf-8"
        );
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("a.md"), "{text}");
        assert!(text.contains("+two"), "{text}");
    }

    #[tokio::test]
    async fn send_back_needs_a_comment() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        for c in [None, Some(""), Some("  \n ")] {
            let err = decide(
                State(state.clone()),
                Path((id.clone(), 7)),
                decision(Decision::SendBack, c, "h7"),
            )
            .await
            .unwrap_err();
            assert_eq!(err.code.status(), 400, "{c:?}");
        }
    }

    #[tokio::test]
    async fn deciding_on_your_own_change_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let err = decide(
            State(state),
            Path((id, 8)),
            decision(Decision::Greenlight, None, "h8"),
        )
        .await
        .unwrap_err();
        assert_eq!(err.http_status(), 403);
        assert_eq!(err.message, "This is your change");
    }

    #[tokio::test]
    async fn deciding_without_push_access_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), false, Some("acme/site")).await;
        let err = decide(
            State(state),
            Path((id, 7)),
            decision(Decision::Greenlight, None, "h7"),
        )
        .await
        .unwrap_err();
        assert_eq!(err.http_status(), 403);
        assert_eq!(
            err.message,
            "You need edit access to this site's repository to greenlight"
        );
    }

    #[tokio::test]
    async fn deciding_on_a_closed_change_is_gone() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let err = decide(
            State(state),
            Path((id, 9)),
            decision(Decision::Greenlight, None, "h9"),
        )
        .await
        .unwrap_err();
        assert_eq!(err.http_status(), 404);
        assert_eq!(err.message, "This change is no longer waiting");
    }

    #[tokio::test]
    async fn a_moved_head_is_a_conflict_with_a_reason() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let err = decide(
            State(state),
            Path((id, 7)),
            decision(Decision::Greenlight, None, "old"),
        )
        .await
        .unwrap_err();
        assert_eq!(err.http_status(), 409);
        assert_eq!(err.reason, Some("head_moved"));
    }

    #[tokio::test]
    async fn a_decision_is_posted_and_answers_no_content() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let status = decide(
            State(state),
            Path((id, 7)),
            decision(Decision::SendBack, Some(" fix it "), "h7"),
        )
        .await
        .unwrap();
        assert_eq!(status, StatusCode::NO_CONTENT);
    }
}

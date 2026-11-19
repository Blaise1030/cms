//! `GET /api/projects/{id}/reviews`, `GET /api/projects/{id}/reviews/{n}`,
//! `GET /api/projects/{id}/reviews/{n}/diff`, `GET /api/projects/{id}/reviews/{n}/preview`,
//! `POST /api/projects/{id}/reviews/{n}/decision`: the review inbox over HTTP.

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::github::{Conversation, Decision, DeployState, ReviewDetail, TimelineItem};

use crate::error::ApiError;
use crate::projects::Project;
use crate::state::AppState;
use crate::{review, settings};

const GONE: &str = "This change is no longer waiting";
/// Response header of `/diff`: the head commit that was diffed.
pub(crate) const HEAD_SHA_HEADER: &str = "x-head-sha";
const OWN_CHANGE: &str = "This is your change";
const NEEDS_EDIT_ACCESS: &str = "You need edit access to this site's repository to greenlight";
const CLOSED: &str = "This change is closed";

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

/// Query of `GET /api/projects/{id}/reviews`.
#[derive(Deserialize, Default)]
pub struct ListQuery {
    /// `closed` for closed and merged changes; anything else (or nothing) lists open ones.
    #[serde(default)]
    state: Option<String>,
}

/// `GET /api/projects/{id}/reviews` → `{ reviews: ReviewItem[], labels }`; with
/// `?state=closed`, `{ reviews, more, labels }` of the most recently updated labelled closed or
/// merged changes. `labels` are the `orso.json` `prLabels` the list filters on.
pub async fn list(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let (project, repo) = project_repo(&state, &id).await?;
    let me = state.github.signed_in_login().await?;
    let labels = settings::pr_labels(&project.dir);
    if query.state.as_deref() == Some("closed") {
        let closed = state.github.closed_reviews(&repo, &labels, &me).await?;
        return Ok(Json(
            json!({ "reviews": closed.reviews, "more": closed.more, "labels": labels }),
        ));
    }
    let inbox = state.github.review_inbox(&repo, &labels, &me).await?;
    // Every open PR, not just the inbox's: a change opened by direct URL keeps its ref until
    // it actually closes.
    review::prune_refs(&project.dir, &inbox.open).await;
    Ok(Json(json!({ "reviews": inbox.reviews, "labels": labels })))
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

/// `GET /api/projects/{id}/reviews/{n}` → the change plus `{ canDecide, reason }`. A closed or
/// merged labelled change reads too, with decisions off ("This change is closed"); an open
/// draft is not waiting for review (404).
pub async fn detail(
    State(state): State<AppState>,
    Path((id, number)): Path<(String, u64)>,
) -> Result<Json<Value>, ApiError> {
    let (project, repo) = project_repo(&state, &id).await?;
    let me = state.github.signed_in_login().await?;
    let detail = state.github.review_detail(&repo, number, &me).await?;
    if detail.open && detail.draft {
        return Err(ApiError::enoent(GONE));
    }
    let reason = if detail.open {
        refusal(&state, &repo, &detail.item.author, &me).await?
    } else {
        // Closed: only one this project reviews is shown at all.
        labelled(&project, &detail)?;
        Some(CLOSED)
    };
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

/// `GET /api/projects/{id}/reviews/{n}/diff` → the change's unified diff (`text/plain`), with the
/// diffed head in `X-Head-Sha` so the UI can tell when it is newer than the detail it shows.
pub async fn diff(
    State(state): State<AppState>,
    Path((id, number)): Path<(String, u64)>,
) -> Result<Response, ApiError> {
    let (project, _) = project_repo(&state, &id).await?;
    let env = state.github.git_env().await?;
    let head = review::fetch_head(&project.dir, number, &env).await?;
    let text = review::diff(&project.dir, &head).await?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "text/plain; charset=utf-8".to_string(),
            ),
            (header::HeaderName::from_static(HEAD_SHA_HEADER), head),
        ],
        text,
    )
        .into_response())
}

/// Whether change `number` carries one of the project's review labels (and so may be read
/// here), with its detail.
async fn labelled_detail(
    state: &AppState,
    project: &Project,
    repo: &str,
    number: u64,
    me: &str,
) -> Result<ReviewDetail, ApiError> {
    let detail = state.github.review_detail(repo, number, me).await?;
    labelled(project, &detail)?;
    Ok(detail)
}

/// 404 unless `detail` carries one of the project's review labels.
fn labelled(project: &Project, detail: &ReviewDetail) -> Result<(), ApiError> {
    let labels = settings::pr_labels(&project.dir);
    let ok = detail
        .labels
        .iter()
        .any(|l| labels.iter().any(|w| w.eq_ignore_ascii_case(l)));
    if ok {
        Ok(())
    } else {
        Err(ApiError::enoent(GONE))
    }
}

/// `GET /api/projects/{id}/reviews/{n}/conversation` → [`Conversation`]: readable for any
/// labelled change, open or not.
pub async fn conversation(
    State(state): State<AppState>,
    Path((id, number)): Path<(String, u64)>,
) -> Result<Json<Conversation>, ApiError> {
    let (project, repo) = project_repo(&state, &id).await?;
    let me = state.github.signed_in_login().await?;
    labelled_detail(&state, &project, &repo, number, &me).await?;
    Ok(Json(state.github.conversation(&repo, number).await?))
}

/// Body of `POST /api/projects/{id}/reviews/{n}/comments`.
#[derive(Deserialize)]
pub struct CommentBody {
    body: String,
}

/// `POST /api/projects/{id}/reviews/{n}/comments` → the new comment as a timeline entry.
/// Anyone signed in may comment (no push access needed), but only on an open change.
pub async fn comment(
    State(state): State<AppState>,
    Path((id, number)): Path<(String, u64)>,
    Json(body): Json<CommentBody>,
) -> Result<Json<TimelineItem>, ApiError> {
    let text = body.body.trim();
    if text.is_empty() {
        return Err(ApiError::badreq("Write a comment first"));
    }
    let (project, repo) = project_repo(&state, &id).await?;
    let me = state.github.signed_in_login().await?;
    let detail = labelled_detail(&state, &project, &repo, number, &me).await?;
    if !detail.open {
        return Err(ApiError::enoent(GONE));
    }
    Ok(Json(state.github.add_comment(&repo, number, text).await?))
}

/// Where a change's deploy preview stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewState {
    /// Its deploy is still running.
    Deploying,
    /// A deployment of the head succeeded; `url` is its preview.
    Ready,
    /// Its deploy check failed.
    Failed,
    /// No deployment and no deploy check: the site doesn't publish previews for changes.
    None,
}

/// `GET /api/projects/{id}/reviews/{n}/preview`: the change's deploy preview and the live
/// site, so the reviewer can compare them. Nothing is built on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPreview {
    pub state: PreviewState,
    /// The head commit this describes.
    pub head_sha: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_url: Option<String>,
}

/// A successful deployment wins; otherwise the head's deploy check says whether one is coming.
fn preview_state(url: Option<&str>, deploy: DeployState) -> PreviewState {
    match (url, deploy) {
        (Some(_), _) => PreviewState::Ready,
        (None, DeployState::Building) => PreviewState::Deploying,
        (None, DeployState::Failed) => PreviewState::Failed,
        // A deploy check that passed without recording a deployment has no URL to show.
        (None, DeployState::Live | DeployState::Unknown) => PreviewState::None,
    }
}

/// `GET /api/projects/{id}/reviews/{n}/preview` → [`ReviewPreview`] of the change's current
/// head. 404 for a change that is closed, a draft or unlabelled.
pub async fn preview(
    State(state): State<AppState>,
    Path((id, number)): Path<(String, u64)>,
) -> Result<Json<ReviewPreview>, ApiError> {
    let (project, repo) = project_repo(&state, &id).await?;
    let me = state.github.signed_in_login().await?;
    let detail = labelled_detail(&state, &project, &repo, number, &me).await?;
    // A closed change still shows its deploy, if one is left.
    if detail.open && detail.draft {
        return Err(ApiError::enoent(GONE));
    }
    let sha = detail.item.head_sha;
    let worker = crate::github::worker_name_from_wrangler(&project.dir);
    let change = async {
        if sha.is_empty() {
            return Ok((None, DeployState::Unknown));
        }
        let url = state.github.preview_url(&repo, &sha).await?;
        let deploy = match url {
            Some(_) => DeployState::Live,
            None => state.github.deploy_state(&repo, &sha).await,
        };
        Ok::<_, ApiError>((url, deploy))
    };
    // Best effort: without a live URL the reviewer still sees the change.
    let live = async {
        state
            .github
            .live_url(&repo, worker.as_deref())
            .await
            .unwrap_or_default()
    };
    let (change, live_url) = tokio::join!(change, live);
    let (url, deploy) = change?;
    Ok(Json(ReviewPreview {
        state: preview_state(url.as_deref(), deploy),
        head_sha: sha,
        url,
        live_url,
    }))
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
                get(
                    |Query(q): Query<std::collections::HashMap<String, String>>| async move {
                        if q.get("state").map(String::as_str) == Some("closed") {
                            let mut merged = pull(20, "bob", "closed", false);
                            merged["merged_at"] = json!("2026-01-05T00:00:00Z");
                            let mut other = pull(21, "bob", "closed", false);
                            other["labels"] = json!([{ "name": "release" }]);
                            return axum::Json(json!([merged, pull(22, "cy", "closed", false), other]));
                        }
                        axum::Json(json!([
                            pull(7, "bob", "open", false),
                            pull(8, "ada", "open", false),
                            pull(9, "carol", "open", false)
                        ]))
                    },
                ),
            )
            .route(
                "/repos/acme/site/issues",
                get(|| async {
                    axum::Json(json!([{ "number": 7, "comments": 3 }, { "number": 20, "comments": 1 }]))
                }),
            )
            .route(
                "/repos/acme/site/pulls/{n}",
                get(|Path(n): Path<u64>| async move {
                    axum::Json(match n {
                        7 => pull(7, "bob", "open", false),
                        8 => pull(8, "ADA", "open", false),
                        9 => pull(9, "bob", "closed", false),
                        11 => {
                            let mut p = pull(11, "bob", "open", false);
                            p["labels"] = json!([]);
                            p
                        }
                        12 | 13 => pull(n, "bob", "open", false),
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
                get(|Path(sha): Path<String>| async move {
                    let runs = match sha.as_str() {
                        "h12" => json!([{ "name": "deploy", "status": "in_progress" }]),
                        "h13" => json!([{ "name": "deploy", "status": "completed",
                            "conclusion": "failure" }]),
                        _ => json!([]),
                    };
                    axum::Json(json!({ "check_runs": runs }))
                }),
            )
            .route(
                "/repos/acme/site/issues/{n}/comments",
                get(|| async {
                    axum::Json(json!([{ "id": 1, "user": { "login": "carol" },
                        "created_at": "2026-01-02T00:00:00Z", "body": "Nice", "html_url": "c1" }]))
                })
                .post(|axum::Json(b): axum::Json<Value>| async move {
                    axum::Json(json!({ "id": 2, "user": { "login": "ada" },
                        "created_at": "2026-01-03T00:00:00Z", "body": b["body"], "html_url": "c2" }))
                }),
            )
            .route(
                "/repos/acme/site/pulls/{n}/commits",
                get(|| async { axum::Json(json!([])) }),
            )
            .route(
                "/repos/acme/site/branches/main",
                get(|| async { axum::Json(json!({ "commit": { "sha": "hmain" } })) }),
            )
            // Deployments: #7's head has a preview, main's tip is the live site.
            .route(
                "/repos/acme/site/deployments",
                get(
                    |Query(q): Query<std::collections::HashMap<String, String>>| async move {
                        axum::Json(match q.get("sha").map(String::as_str) {
                            Some("h7") => json!([{ "id": 7, "sha": "h7" }]),
                            Some("hmain") => json!([{ "id": 1, "sha": "hmain" }]),
                            _ => json!([]),
                        })
                    },
                ),
            )
            .route(
                "/repos/acme/site/deployments/{id}/statuses",
                get(|Path(id): Path<u64>| async move {
                    let url = if id == 7 {
                        "https://pr-7.site.test/"
                    } else {
                        "https://site.test/"
                    };
                    axum::Json(json!([{ "state": "success", "environment_url": url }]))
                }),
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
    async fn list_returns_every_labelled_change_marking_your_own() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let v = body_json(list(State(state), Path(id), Query(ListQuery::default())).await).await;
        // The prLabels it filtered on (orso.json's default here), for the inbox's subtitle.
        assert_eq!(v["labels"], json!(["orso"]));
        let nums: Vec<u64> = v["reviews"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["number"].as_u64().unwrap())
            .collect();
        assert_eq!(nums, vec![7, 8, 9]);
        assert_eq!(v["reviews"][0]["author"], "bob");
        assert_eq!(v["reviews"][0]["mine"], false);
        assert_eq!(v["reviews"][1]["mine"], true);
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
    async fn detail_of_an_open_draft_is_gone() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let err = detail(State(state), Path((id, 10))).await.unwrap_err();
        assert_eq!(err.message, "This change is no longer waiting");
        assert_eq!(err.code.status(), 404);
    }

    #[tokio::test]
    async fn detail_of_a_closed_labelled_change_reads_without_decisions() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let v = body_json(detail(State(state), Path((id, 9))).await).await;
        assert_eq!(v["open"], false);
        assert_eq!(v["state"], "closed");
        assert_eq!(v["canDecide"], false);
        assert_eq!(v["reason"], "This change is closed");
    }

    #[tokio::test]
    async fn list_rows_carry_labels_counts_and_state() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let v = body_json(list(State(state), Path(id), Query(ListQuery::default())).await).await;
        let row = &v["reviews"][0];
        assert_eq!(row["number"], 7);
        assert_eq!(row["labels"], json!([{ "name": "orso", "color": "" }]));
        assert_eq!(row["comments"], 3);
        assert_eq!(row["state"], "open");
        assert_eq!(row["draft"], false);
        assert_eq!(row["checksPassed"], 0);
        assert_eq!(row["checksTotal"], 0);
        assert_eq!(v["reviews"][1]["comments"], 0);
    }

    #[tokio::test]
    async fn closed_list_has_labelled_closed_and_merged_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let q = Query(ListQuery {
            state: Some("closed".into()),
        });
        let v = body_json(list(State(state), Path(id), q).await).await;
        assert_eq!(v["labels"], json!(["orso"]));
        let rows: Vec<(u64, String)> = v["reviews"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["number"].as_u64().unwrap(),
                    r["state"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        assert_eq!(rows, vec![(20, "merged".into()), (22, "closed".into())]);
        assert_eq!(v["reviews"][0]["comments"], 1);
        assert_eq!(v["more"], false);
    }

    #[tokio::test]
    async fn project_without_github_remote_is_a_bad_request() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, None).await;
        let err = list(State(state), Path(id), Query(ListQuery::default()))
            .await
            .unwrap_err();
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
        assert_eq!(res.headers()["x-head-sha"], head_of(tmp.path()).as_str());
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

    /// `state` with a builder that writes a one-page site instead of running npm.
    fn head_of(root: &FsPath) -> String {
        let out = Command::new("git")
            .args(["rev-parse", "refs/pull/7/head"])
            .current_dir(root.join("o/origin.git"))
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    async fn preview_of(state: &AppState, id: &str, n: u64) -> Result<ReviewPreview, ApiError> {
        preview(State(state.clone()), Path((id.to_string(), n)))
            .await
            .map(|j| j.0)
    }

    #[test]
    fn preview_state_prefers_a_deployment_then_the_deploy_check() {
        use DeployState::*;
        assert_eq!(
            preview_state(Some("https://x"), Failed),
            PreviewState::Ready
        );
        assert_eq!(preview_state(None, Building), PreviewState::Deploying);
        assert_eq!(preview_state(None, Failed), PreviewState::Failed);
        assert_eq!(preview_state(None, Live), PreviewState::None);
        assert_eq!(preview_state(None, Unknown), PreviewState::None);
    }

    #[tokio::test]
    async fn preview_is_the_heads_deployment_with_the_live_site() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let got = preview_of(&state, &id, 7).await.unwrap();
        assert_eq!(
            got,
            ReviewPreview {
                state: PreviewState::Ready,
                head_sha: "h7".into(),
                url: Some("https://pr-7.site.test/".into()),
                live_url: Some("https://site.test/".into()),
            }
        );
        let json = serde_json::to_value(&got).unwrap();
        assert_eq!(json["state"], "ready");
        assert_eq!(json["liveUrl"], "https://site.test/");
        assert_eq!(json["headSha"], "h7");
    }

    #[tokio::test]
    async fn preview_follows_the_deploy_check_until_a_deployment_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let building = preview_of(&state, &id, 12).await.unwrap();
        assert_eq!(building.state, PreviewState::Deploying);
        assert_eq!(building.url, None);
        assert_eq!(building.live_url.as_deref(), Some("https://site.test/"));
        let failed = preview_of(&state, &id, 13).await.unwrap();
        assert_eq!(failed.state, PreviewState::Failed);
        // #8 has neither a deployment nor a deploy check.
        let none = preview_of(&state, &id, 8).await.unwrap();
        assert_eq!(none.state, PreviewState::None);
        let json = serde_json::to_value(&none).unwrap();
        assert!(json.get("url").is_none());
    }

    #[tokio::test]
    async fn conversation_is_readable_for_any_labelled_change() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        // Open (#7) and closed (#9) alike.
        for n in [7, 9] {
            let c = conversation(State(state.clone()), Path((id.clone(), n)))
                .await
                .unwrap()
                .0;
            let json = serde_json::to_value(&c).unwrap();
            assert_eq!(json["items"][0]["kind"], "comment", "#{n}");
            assert_eq!(json["items"][0]["author"], "carol");
        }
        let err = conversation(State(state), Path((id, 11)))
            .await
            .unwrap_err();
        assert_eq!(err.code.status(), 404);
    }

    fn comment_body(text: &str) -> Json<CommentBody> {
        Json(CommentBody { body: text.into() })
    }

    #[tokio::test]
    async fn a_comment_is_posted_and_comes_back_as_a_timeline_entry() {
        let tmp = tempfile::tempdir().unwrap();
        // No push access needed to comment.
        let (state, id) = setup(tmp.path(), false, Some("acme/site")).await;
        let item = comment(State(state), Path((id, 7)), comment_body("  Looks good  "))
            .await
            .unwrap()
            .0;
        let json = serde_json::to_value(&item).unwrap();
        assert_eq!(json["kind"], "comment");
        assert_eq!(json["author"], "ada");
        assert_eq!(json["body"], "Looks good");
    }

    #[tokio::test]
    async fn a_blank_comment_or_one_on_a_closed_change_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        let err = comment(
            State(state.clone()),
            Path((id.clone(), 7)),
            comment_body("  \n "),
        )
        .await
        .unwrap_err();
        assert_eq!(err.code.status(), 400);
        let err = comment(State(state), Path((id, 9)), comment_body("late"))
            .await
            .unwrap_err();
        assert_eq!(err.code.status(), 404);
        assert_eq!(err.message, GONE);
    }

    #[tokio::test]
    async fn preview_of_a_change_not_waiting_is_gone() {
        let tmp = tempfile::tempdir().unwrap();
        let (state, id) = setup(tmp.path(), true, Some("acme/site")).await;
        // Draft, unlabelled.
        for n in [10, 11] {
            let err = preview_of(&state, &id, n).await.unwrap_err();
            assert_eq!(err.message, GONE, "#{n}");
            assert_eq!(err.code.status(), 404);
        }
        // A closed change still answers with whatever deploy it has.
        assert_eq!(
            preview_of(&state, &id, 9).await.unwrap().state,
            PreviewState::None
        );
    }
}

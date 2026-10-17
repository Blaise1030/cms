//! Agent-driven conflict resolution: result parsing, prompt, verification, and the job runner.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;
use tokio::sync::{oneshot, Mutex, Notify};

use super::conflict_sessions::{ConflictSession, ConflictSessions};
use super::conflicts::{self, ConflictFile};
use crate::conflict_preview::{link_node_modules, BuildGroup, ConflictPreviewManager};
use crate::error::ApiError;
use crate::events::EventBus;
use crate::git::{self, Identity};
use crate::providers::{headless_spec, AgentSession, Provider};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub text: String,
    pub options: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum ResolveResult {
    Resolved { summary: String },
    NeedsInput { questions: Vec<Question> },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Failure {
    Unavailable,
    BuildFailed,
    AgentFailed,
    TimedOut,
    DraftMoved,
    Cancelled,
}

impl Failure {
    pub fn message(self) -> &'static str {
        match self {
            Failure::Unavailable => "Your AI assistant isn't available.",
            Failure::BuildFailed => "The site didn't build after combining.",
            Failure::AgentFailed => "Your AI assistant couldn't finish combining the changes.",
            Failure::TimedOut => "This took too long.",
            Failure::DraftMoved => "Your draft changed while combining. Please sync again.",
            Failure::Cancelled => "You stopped combining.",
        }
    }
}

pub const MAX_ROUNDS: u32 = 3;

#[derive(Deserialize)]
struct RawResult {
    status: String,
    summary: Option<String>,
    questions: Option<Vec<Question>>,
}

pub fn parse_result(bytes: &[u8]) -> Option<ResolveResult> {
    let raw: RawResult = serde_json::from_slice(bytes).ok()?;
    match raw.status.as_str() {
        "resolved" => {
            let summary = raw.summary?.trim().to_string();
            (!summary.is_empty()).then_some(ResolveResult::Resolved { summary })
        }
        "needs_input" => {
            let questions = raw.questions?;
            let mut ids = std::collections::HashSet::new();
            let valid = (1..=5).contains(&questions.len())
                && questions.iter().all(|q| {
                    !q.id.trim().is_empty()
                        && ids.insert(q.id.as_str())
                        && !q.text.trim().is_empty()
                        && (2..=3).contains(&q.options.len())
                });
            valid.then_some(ResolveResult::NeedsInput { questions })
        }
        _ => None,
    }
}

pub fn prompt(result_path: &Path, answers: &[(Question, String)]) -> String {
    let mut out = format!(
        "You are combining a writer's draft with newer changes to the live site. \
Resolve every git conflict marker in this repository. Keep the intent of both sides and never \
drop content silently. Do not commit, push, abort the merge or switch branches. If both sides \
made genuinely different choices about the same content, do not guess: write needs_input with \
plain-language questions a non-technical writer can answer, each with 2-3 short options. \
Otherwise write resolved with a one-sentence plain-language summary of what you kept.\n\n\
When you are done, write exactly one of these JSON objects to {}:\n\
{{\"status\":\"resolved\",\"summary\":\"<one sentence>\"}}\n\
{{\"status\":\"needs_input\",\"questions\":[{{\"id\":\"q1\",\"text\":\"<question>\",\"options\":[\"<option>\",\"<option>\"]}}]}}\n",
        result_path.display()
    );
    if !answers.is_empty() {
        out.push_str("\nAnswers from the writer:\n");
        for (question, answer) in answers {
            out.push_str(&format!("- {}: {}\n", question.text, answer));
        }
    }
    out
}

pub async fn stage_and_verify(
    worktree: &Path,
    tip: &str,
    incoming: &str,
    conflicted: &[String],
) -> Result<(), Failure> {
    let head = conflicts::text(worktree, &["rev-parse", "HEAD"])
        .await
        .map_err(fail)?;
    let merge_head = conflicts::text(worktree, &["rev-parse", "-q", "--verify", "MERGE_HEAD"])
        .await
        .map_err(fail)?;
    if head != tip || merge_head != incoming {
        return Err(Failure::AgentFailed);
    }
    conflicts::checked(
        worktree,
        &["add", "-A", "--", ".", ":!node_modules"],
        &[],
        None,
    )
    .await
    .map_err(fail)?;
    if !conflicted.is_empty() {
        let mut args = vec![
            "--literal-pathspecs",
            "grep",
            "--cached",
            "-l",
            "-E",
            "^(<<<<<<< |=======$|>>>>>>> )",
            "--",
        ];
        args.extend(conflicted.iter().map(String::as_str));
        let (status, _, stderr) = conflicts::git(worktree, &args, &[], None)
            .await
            .map_err(fail)?;
        match status {
            1 => {}
            0 => return Err(Failure::AgentFailed),
            _ => {
                return Err(fail(ApiError::git(format!(
                    "git grep failed: {}",
                    String::from_utf8_lossy(&stderr).trim()
                ))))
            }
        }
    }
    let raw = conflicts::checked(
        worktree,
        &["diff", "--cached", "--raw", "-z", "--no-renames", tip],
        &[],
        None,
    )
    .await
    .map_err(fail)?;
    let raw = String::from_utf8_lossy(&raw);
    let mut fields = raw.split('\0');
    while let (Some(header), Some(path)) = (fields.next(), fields.next()) {
        if header.split_whitespace().nth(1) != Some("120000") {
            continue;
        }
        let entry = conflicts::text(
            worktree,
            &["--literal-pathspecs", "ls-tree", incoming, "--", path],
        )
        .await
        .map_err(fail)?;
        if !entry.starts_with("120000 ") {
            return Err(Failure::AgentFailed);
        }
    }
    Ok(())
}

fn fail(e: ApiError) -> Failure {
    eprintln!("workbench: auto-resolve failed ({e})");
    Failure::AgentFailed
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Preparing,
    Resolving,
    NeedsInput,
    Verifying,
    Done,
    Failed,
}

impl JobState {
    fn is_terminal(self) -> bool {
        matches!(self, JobState::Done | JobState::Failed)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatus {
    pub session_id: String,
    pub state: JobState,
    pub stage: String,
    pub questions: Vec<Question>,
    pub summary: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct Timeouts {
    pub agent: Duration,
    pub build: Duration,
    /// How long to wait for the writer's answers; below the conflict session TTL.
    pub answer: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            agent: Duration::from_secs(600),
            build: Duration::from_secs(300),
            answer: Duration::from_secs(50 * 60),
        }
    }
}

pub struct JobContext {
    pub project_id: String,
    pub project_dir: PathBuf,
    pub session_id: String,
    pub provider: Provider,
    pub author: Identity,
    pub state_dir: PathBuf,
    pub events: EventBus,
    pub sessions: Arc<ConflictSessions>,
    pub previews: Arc<ConflictPreviewManager>,
}

struct Job {
    project_id: String,
    status: JobStatus,
    answer: Option<oneshot::Sender<Vec<(Question, String)>>>,
    cancel: Arc<Notify>,
}

type Jobs = Arc<Mutex<HashMap<String, Job>>>;

/// One auto-resolve job per conflict session, at most one running per project.
pub struct AutoResolveJobs {
    timeouts: Timeouts,
    jobs: Jobs,
}

impl AutoResolveJobs {
    pub fn new(timeouts: Timeouts) -> Self {
        Self {
            timeouts,
            jobs: Arc::default(),
        }
    }

    pub async fn start(&self, ctx: JobContext) -> Result<(), ApiError> {
        let session = ctx.sessions.get(&ctx.session_id).await?;
        if session.project_id != ctx.project_id {
            return Err(ApiError::enoent("conflict session expired or not found"));
        }
        let moved =
            match conflicts::ensure_tip(&ctx.project_dir, &session.branch, &session.tip).await {
                Ok(()) => false,
                Err(e) if e.status.unwrap_or(e.code.status()) == 409 => true,
                Err(e) => return Err(e),
            };
        let mut jobs = self.jobs.lock().await;
        if jobs
            .values()
            .any(|j| j.project_id == ctx.project_id && !j.status.state.is_terminal())
        {
            return Err(ApiError::conflict(
                "Your AI assistant is already combining changes in this project",
            ));
        }
        jobs.retain(|_, j| j.project_id != ctx.project_id);
        let status = if moved {
            JobStatus {
                error: Some(Failure::DraftMoved.message().to_string()),
                ..new_status(&ctx.session_id, JobState::Failed, "")
            }
        } else {
            new_status(&ctx.session_id, JobState::Preparing, "Getting ready…")
        };
        emit(&ctx, &status);
        let cancel = Arc::new(Notify::new());
        jobs.insert(
            ctx.session_id.clone(),
            Job {
                project_id: ctx.project_id.clone(),
                status,
                answer: None,
                cancel: cancel.clone(),
            },
        );
        drop(jobs);
        if !moved {
            tokio::spawn(run(self.jobs.clone(), self.timeouts, ctx, session, cancel));
        }
        Ok(())
    }

    /// The project that started the session's job; outlives the session itself.
    pub async fn project_of(&self, session_id: &str) -> Option<String> {
        self.jobs
            .lock()
            .await
            .get(session_id)
            .map(|j| j.project_id.clone())
    }

    pub async fn status(&self, session_id: &str) -> Option<JobStatus> {
        self.jobs
            .lock()
            .await
            .get(session_id)
            .map(|j| j.status.clone())
    }

    pub async fn answer(
        &self,
        session_id: &str,
        answers: BTreeMap<String, String>,
    ) -> Result<(), ApiError> {
        let mut jobs = self.jobs.lock().await;
        let job = jobs
            .get_mut(session_id)
            .ok_or_else(|| ApiError::enoent("no combining job for this conflict"))?;
        if job.status.state != JobState::NeedsInput || job.answer.is_none() {
            return Err(ApiError::conflict("Not waiting for an answer"));
        }
        let mut pairs = Vec::new();
        for question in &job.status.questions {
            let answer = answers
                .get(&question.id)
                .map(|a| a.trim())
                .filter(|a| !a.is_empty())
                .ok_or_else(|| ApiError::badreq(format!("Answer \"{}\"", question.text)))?;
            pairs.push((question.clone(), answer.to_string()));
        }
        if let Some(tx) = job.answer.take() {
            let _ = tx.send(pairs);
        }
        Ok(())
    }

    pub async fn cancel(&self, session_id: &str) -> Result<(), ApiError> {
        let jobs = self.jobs.lock().await;
        let job = jobs
            .get(session_id)
            .ok_or_else(|| ApiError::enoent("no combining job for this conflict"))?;
        if job.status.state.is_terminal() {
            return Err(ApiError::conflict("Combining already finished"));
        }
        job.cancel.notify_one();
        Ok(())
    }

    pub async fn is_running(&self, session_id: &str) -> bool {
        self.jobs
            .lock()
            .await
            .get(session_id)
            .is_some_and(|j| !j.status.state.is_terminal())
    }
}

fn new_status(session_id: &str, state: JobState, stage: &str) -> JobStatus {
    JobStatus {
        session_id: session_id.to_string(),
        state,
        stage: stage.to_string(),
        questions: Vec::new(),
        summary: None,
        error: None,
    }
}

fn emit(ctx: &JobContext, status: &JobStatus) {
    let mut fields = serde_json::to_value(status).unwrap_or(Value::Null);
    if let Value::Object(map) = &mut fields {
        map.insert("projectId".into(), Value::String(ctx.project_id.clone()));
    }
    ctx.events.emit("changes.auto_resolve", fields);
}

/// "src/content/pricing-plans.md" → "Pricing plans".
fn label(path: &str) -> String {
    let stem = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .replace(['-', '_'], " ");
    let mut chars = stem.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => path.to_string(),
    }
}

fn resolving_stage(files: &[ConflictFile]) -> String {
    let labels: Vec<String> = files.iter().map(|f| label(&f.path)).collect();
    format!("Combining {}…", labels.join(", "))
}

async fn run(
    jobs: Jobs,
    timeouts: Timeouts,
    ctx: JobContext,
    session: ConflictSession,
    cancel: Arc<Notify>,
) {
    let job_dir = std::env::temp_dir().join(format!("wb-sync-{}", ctx.session_id));
    cleanup(&ctx.project_dir, &job_dir).await;
    let runner = Runner {
        jobs: &jobs,
        ctx: &ctx,
        session: &session,
        timeouts,
        site: job_dir.join("site"),
        job_dir: &job_dir,
    };
    let outcome = tokio::select! {
        outcome = runner.resolve() => outcome,
        _ = cancel.notified() => Err(Failure::Cancelled),
    };
    let outcome = match outcome {
        Ok((commit, summary)) => runner.land(&commit).await.map(|()| summary),
        Err(failure) => Err(failure),
    };
    cleanup(&ctx.project_dir, &job_dir).await;
    let status = match outcome {
        Ok(summary) => JobStatus {
            summary: Some(summary),
            ..new_status(&ctx.session_id, JobState::Done, "Done")
        },
        Err(failure) => JobStatus {
            error: Some(failure.message().to_string()),
            ..new_status(&ctx.session_id, JobState::Failed, "")
        },
    };
    runner.update(status, None).await;
}

async fn cleanup(project_dir: &Path, job_dir: &Path) {
    let site = job_dir.join("site");
    let _ = conflicts::git(
        project_dir,
        &["worktree", "remove", "--force", &site.to_string_lossy()],
        &[],
        None,
    )
    .await;
    let _ = tokio::fs::remove_dir_all(job_dir).await;
}

struct Runner<'a> {
    jobs: &'a Jobs,
    ctx: &'a JobContext,
    session: &'a ConflictSession,
    timeouts: Timeouts,
    job_dir: &'a Path,
    site: PathBuf,
}

enum RunError {
    Io(std::io::Error),
    TimedOut,
}

impl Runner<'_> {
    async fn update(
        &self,
        status: JobStatus,
        answer: Option<oneshot::Sender<Vec<(Question, String)>>>,
    ) {
        let mut jobs = self.jobs.lock().await;
        if let Some(job) = jobs.get_mut(&self.ctx.session_id) {
            emit(self.ctx, &status);
            job.status = status;
            job.answer = answer;
        }
    }

    async fn set(&self, state: JobState, stage: &str) {
        self.update(new_status(&self.ctx.session_id, state, stage), None)
            .await;
    }

    /// Everything up to an unattached merge commit; safe to abandon at any await.
    async fn resolve(&self) -> Result<(String, String), Failure> {
        let (ctx, session) = (self.ctx, self.session);
        tokio::fs::create_dir_all(self.job_dir)
            .await
            .map_err(|e| fail(e.into()))?;
        conflicts::checked(
            &ctx.project_dir,
            &[
                "worktree",
                "add",
                "--detach",
                "--quiet",
                &self.site.to_string_lossy(),
                &session.tip,
            ],
            &[],
            None,
        )
        .await
        .map_err(fail)?;
        let (code, _, stderr) = conflicts::git(
            &self.site,
            &["merge", "--no-commit", "--no-ff", &session.incoming],
            &[],
            None,
        )
        .await
        .map_err(fail)?;
        if code != 0 && code != 1 {
            return Err(fail(ApiError::git(format!(
                "git merge failed: {}",
                String::from_utf8_lossy(&stderr).trim()
            ))));
        }

        let stage = resolving_stage(&session.files);
        let mut answers = Vec::new();
        let mut round = 1;
        let summary = loop {
            self.set(JobState::Resolving, &stage).await;
            match self.agent(&answers).await? {
                ResolveResult::Resolved { summary } => break summary,
                ResolveResult::NeedsInput { questions } if round < MAX_ROUNDS => {
                    let (tx, rx) = oneshot::channel();
                    let status = JobStatus {
                        questions,
                        ..new_status(
                            &ctx.session_id,
                            JobState::NeedsInput,
                            "Waiting for your answer",
                        )
                    };
                    self.update(status, Some(tx)).await;
                    let reply = tokio::time::timeout(self.timeouts.answer, rx)
                        .await
                        .map_err(|_| Failure::TimedOut)?;
                    answers.extend(reply.map_err(|_| Failure::Cancelled)?);
                    round += 1;
                }
                ResolveResult::NeedsInput { .. } => return Err(Failure::AgentFailed),
            }
        };

        self.set(JobState::Verifying, "Checking the site still builds…")
            .await;
        let conflicted: Vec<String> = session.files.iter().map(|f| f.path.clone()).collect();
        stage_and_verify(&self.site, &session.tip, &session.incoming, &conflicted).await?;
        link_node_modules(&ctx.project_dir, &self.site).map_err(|e| {
            eprintln!("workbench: auto-resolve cannot link node_modules ({e})");
            Failure::BuildFailed
        })?;
        let mut build = Command::new("npm");
        build.args(["run", "build"]);
        match self.run_logged(build, self.timeouts.build).await {
            Ok(status) if status.success() => {}
            Ok(_) | Err(RunError::Io(_)) => return Err(Failure::BuildFailed),
            Err(RunError::TimedOut) => return Err(Failure::TimedOut),
        }

        let tree = conflicts::text(&self.site, &["write-tree"])
            .await
            .map_err(fail)?;
        let commit = conflicts::merge_commit(
            &ctx.project_dir,
            &tree,
            &session.tip,
            &session.incoming,
            &ctx.author,
            "Sync with live site",
        )
        .await
        .map_err(fail)?;
        Ok((commit, summary))
    }

    async fn agent(&self, answers: &[(Question, String)]) -> Result<ResolveResult, Failure> {
        let ctx = self.ctx;
        let result_path = self.job_dir.join("result.json");
        let _ = tokio::fs::remove_file(&result_path).await;
        let spec = headless_spec(
            &ctx.provider,
            &prompt(&result_path, answers),
            &ctx.state_dir,
            &ctx.project_id,
            &AgentSession::Fresh,
        );
        let (program, args) = spec.argv.split_first().ok_or(Failure::Unavailable)?;
        let mut agent = Command::new(program);
        agent
            .args(args)
            .envs(spec.env)
            .env("WORKBENCH_RESOLVE_RESULT", &result_path);
        match self.run_logged(agent, self.timeouts.agent).await {
            Ok(_) => {}
            Err(RunError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(Failure::Unavailable)
            }
            Err(RunError::Io(e)) => return Err(fail(e.into())),
            Err(RunError::TimedOut) => return Err(Failure::TimedOut),
        }
        let bytes = tokio::fs::read(&result_path).await.unwrap_or_default();
        parse_result(&bytes).ok_or(Failure::AgentFailed)
    }

    /// Runs `cmd` in the worktree as its own process group, logging to the session's sync log.
    async fn run_logged(&self, mut cmd: Command, limit: Duration) -> Result<ExitStatus, RunError> {
        let logs = self.ctx.state_dir.join("sync-logs");
        let log = std::fs::create_dir_all(&logs)
            .and_then(|()| {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(logs.join(format!("{}.log", self.ctx.session_id)))
            })
            .map_err(RunError::Io)?;
        let stderr = log.try_clone().map_err(RunError::Io)?;
        cmd.current_dir(&self.site)
            .stdin(Stdio::null())
            .stdout(log)
            .stderr(stderr)
            .kill_on_drop(true)
            .process_group(0);
        let mut child = cmd.spawn().map_err(RunError::Io)?;
        let _group = BuildGroup(child.id().map(|pid| pid as i32));
        match tokio::time::timeout(limit, child.wait()).await {
            Ok(status) => status.map_err(RunError::Io),
            Err(_) => Err(RunError::TimedOut),
        }
    }

    /// Moves the draft branch onto `commit` and closes the conflict session.
    async fn land(&self, commit: &str) -> Result<(), Failure> {
        let (ctx, session) = (self.ctx, self.session);
        {
            let lock = git::lock_for(&ctx.project_dir);
            let _guard = lock.lock().await;
            conflicts::advance_branch(&ctx.project_dir, &session.branch, &session.tip, commit)
                .await
                .map_err(|e| {
                    if e.status.unwrap_or(e.code.status()) == 409 {
                        Failure::DraftMoved
                    } else {
                        fail(e)
                    }
                })?;
        }
        let _ = ctx.sessions.abandon(&ctx.session_id).await;
        ctx.previews.remove_session(&ctx.session_id).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn q(id: &str, options: &[&str]) -> serde_json::Value {
        serde_json::json!({"id": id, "text": format!("Question {id}?"), "options": options})
    }

    fn needs_input(questions: Vec<serde_json::Value>) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"status": "needs_input", "questions": questions}))
            .unwrap()
    }

    #[test]
    fn parse_resolved() {
        assert_eq!(
            parse_result(br#"{"status":"resolved","summary":"Kept both headlines"}"#),
            Some(ResolveResult::Resolved {
                summary: "Kept both headlines".into()
            })
        );
    }

    #[test]
    fn parse_needs_input() {
        let bytes = needs_input(vec![q("q1", &["A", "B"]), q("q2", &["A", "B", "C"])]);
        assert_eq!(
            parse_result(&bytes),
            Some(ResolveResult::NeedsInput {
                questions: vec![
                    Question {
                        id: "q1".into(),
                        text: "Question q1?".into(),
                        options: vec!["A".into(), "B".into()],
                    },
                    Question {
                        id: "q2".into(),
                        text: "Question q2?".into(),
                        options: vec!["A".into(), "B".into(), "C".into()],
                    },
                ]
            })
        );
    }

    #[test]
    fn parse_rejects_missing_summary() {
        assert_eq!(parse_result(br#"{"status":"resolved"}"#), None);
        assert_eq!(parse_result(br#"{"status":"resolved","summary":""}"#), None);
        assert_eq!(
            parse_result(br#"{"status":"resolved","summary":"  "}"#),
            None
        );
    }

    #[test]
    fn parse_rejects_zero_or_six_questions() {
        assert_eq!(parse_result(&needs_input(vec![])), None);
        let six = (1..=6).map(|i| q(&format!("q{i}"), &["A", "B"])).collect();
        assert_eq!(parse_result(&needs_input(six)), None);
        let five = (1..=5).map(|i| q(&format!("q{i}"), &["A", "B"])).collect();
        assert!(parse_result(&needs_input(five)).is_some());
    }

    #[test]
    fn parse_rejects_one_or_four_options() {
        assert_eq!(parse_result(&needs_input(vec![q("q1", &["A"])])), None);
        assert_eq!(
            parse_result(&needs_input(vec![q("q1", &["A", "B", "C", "D"])])),
            None
        );
    }

    #[test]
    fn parse_rejects_garbage() {
        assert_eq!(parse_result(b"not json"), None);
        assert_eq!(parse_result(b""), None);
        assert_eq!(parse_result(br#"{"status":"done","summary":"x"}"#), None);
        assert_eq!(
            parse_result(&needs_input(vec![
                q("q1", &["A", "B"]),
                q("q1", &["A", "B"])
            ])),
            None,
            "duplicate ids"
        );
        assert_eq!(
            parse_result(&needs_input(vec![q("", &["A", "B"])])),
            None,
            "empty id"
        );
        assert_eq!(
            parse_result(
                br#"{"status":"needs_input","questions":[{"id":"q1","text":"","options":["A","B"]}]}"#
            ),
            None,
            "empty text"
        );
    }

    #[test]
    fn prompt_without_answers_has_no_answer_block() {
        let p = prompt(Path::new("/tmp/wt/.workbench/resolve-result.json"), &[]);
        assert!(p.starts_with("You are combining a writer's draft"));
        assert!(p.contains("/tmp/wt/.workbench/resolve-result.json"));
        assert!(p.contains(r#""status":"resolved""#));
        assert!(p.contains(r#""status":"needs_input""#));
        assert!(!p.contains("Answers from the writer:"));
    }

    #[test]
    fn prompt_lists_answers() {
        let question = Question {
            id: "q1".into(),
            text: "Which headline should stay?".into(),
            options: vec!["Simple pricing".into(), "Plans for every team".into()],
        };
        let p = prompt(
            Path::new("/tmp/r.json"),
            &[(question, "Simple pricing".into())],
        );
        let instruction = p.find("Resolve every git conflict marker").unwrap();
        let path = p.find("/tmp/r.json").unwrap();
        let answers = p.find("Answers from the writer:").unwrap();
        assert!(instruction < path && path < answers);
        assert!(p[answers..].contains("\n- Which headline should stay?: Simple pricing"));
    }

    #[test]
    fn labels_from_file_stems() {
        assert_eq!(label("src/content/pricing-plans.md"), "Pricing plans");
        assert_eq!(label("about_us.yaml"), "About us");
        assert_eq!(label(".md"), ".md");
        let file = |path: &str| ConflictFile {
            path: path.into(),
            binary: false,
            ours_deleted: false,
            theirs_deleted: false,
            ours_oid: String::new(),
            theirs_oid: String::new(),
            base_oid: None,
            ours_mode: String::new(),
            theirs_mode: String::new(),
        };
        assert_eq!(
            resolving_stage(&[file("src/content/pricing-plans.md"), file("home.md")]),
            "Combining Pricing plans, Home…"
        );
    }

    #[test]
    fn failure_messages_exact() {
        assert_eq!(
            Failure::Unavailable.message(),
            "Your AI assistant isn't available."
        );
        assert_eq!(
            Failure::BuildFailed.message(),
            "The site didn't build after combining."
        );
        assert_eq!(
            Failure::AgentFailed.message(),
            "Your AI assistant couldn't finish combining the changes."
        );
        assert_eq!(Failure::TimedOut.message(), "This took too long.");
        assert_eq!(
            Failure::DraftMoved.message(),
            "Your draft changed while combining. Please sync again."
        );
        assert_eq!(Failure::Cancelled.message(), "You stopped combining.");
    }

    fn sh(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args([
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@e",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// Repo mid-merge with a conflict in `page.md`; `live` adds extra files on the incoming side.
    /// Returns (dir, tip, incoming).
    fn conflicted_with(live: impl Fn(&Path)) -> (tempfile::TempDir, String, String) {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        sh(dir, &["init", "-q", "-b", "main"]);
        std::fs::write(dir.join("page.md"), "title: base\n").unwrap();
        std::fs::write(dir.join("notes.md"), "Notes\n=======\n").unwrap();
        sh(dir, &["add", "."]);
        sh(dir, &["commit", "-qm", "base"]);
        sh(dir, &["checkout", "-qb", "draft"]);
        std::fs::write(dir.join("page.md"), "title: draft\n").unwrap();
        sh(dir, &["commit", "-qam", "draft"]);
        let tip = sh(dir, &["rev-parse", "HEAD"]);
        sh(dir, &["checkout", "-q", "main"]);
        std::fs::write(dir.join("page.md"), "title: live\n").unwrap();
        live(dir);
        sh(dir, &["add", "-A"]);
        sh(dir, &["commit", "-qm", "live"]);
        let incoming = sh(dir, &["rev-parse", "HEAD"]);
        sh(dir, &["checkout", "-q", "draft"]);
        sh(dir, &["merge", "--no-commit", "--no-ff", &incoming]);
        let page = std::fs::read_to_string(dir.join("page.md")).unwrap();
        assert!(page.contains("<<<<<<< "), "fixture must conflict");
        (tmp, tip, incoming)
    }

    fn conflicted() -> (tempfile::TempDir, String, String) {
        conflicted_with(|_| {})
    }

    fn resolve(dir: &Path) {
        std::fs::write(dir.join("page.md"), "title: draft and live\n").unwrap();
    }

    async fn verify(dir: &Path, tip: &str, incoming: &str) -> Result<(), Failure> {
        stage_and_verify(dir, tip, incoming, &["page.md".to_string()]).await
    }

    #[tokio::test]
    async fn verify_rejects_markers() {
        let (tmp, tip, incoming) = conflicted();
        assert_eq!(
            verify(tmp.path(), &tip, &incoming).await,
            Err(Failure::AgentFailed)
        );
    }

    #[tokio::test]
    async fn verify_ignores_markers_outside_conflicted_files() {
        let (tmp, tip, incoming) = conflicted();
        resolve(tmp.path());
        assert_eq!(verify(tmp.path(), &tip, &incoming).await, Ok(()));
    }

    #[tokio::test]
    async fn verify_rejects_agent_commit() {
        let (tmp, tip, incoming) = conflicted();
        resolve(tmp.path());
        sh(tmp.path(), &["commit", "-qam", "agent"]);
        assert_eq!(
            verify(tmp.path(), &tip, &incoming).await,
            Err(Failure::AgentFailed)
        );
    }

    #[tokio::test]
    async fn verify_rejects_new_symlink() {
        let (tmp, tip, incoming) = conflicted();
        resolve(tmp.path());
        std::os::unix::fs::symlink("/etc/passwd", tmp.path().join("link")).unwrap();
        assert_eq!(
            verify(tmp.path(), &tip, &incoming).await,
            Err(Failure::AgentFailed)
        );
    }

    #[tokio::test]
    async fn verify_allows_symlink_from_incoming() {
        let (tmp, tip, incoming) = conflicted_with(|dir| {
            std::os::unix::fs::symlink("page.md", dir.join("link")).unwrap();
        });
        resolve(tmp.path());
        assert_eq!(verify(tmp.path(), &tip, &incoming).await, Ok(()));
    }

    #[tokio::test]
    async fn verify_accepts_clean_merge() {
        let (tmp, tip, incoming) = conflicted();
        resolve(tmp.path());
        std::fs::create_dir(tmp.path().join("node_modules")).unwrap();
        std::fs::write(tmp.path().join("node_modules/x.js"), "<<<<<<< x\n").unwrap();
        assert_eq!(verify(tmp.path(), &tip, &incoming).await, Ok(()));
        let staged = sh(tmp.path(), &["diff", "--cached", "--name-only", &tip]);
        assert_eq!(staged, "page.md");
    }
}

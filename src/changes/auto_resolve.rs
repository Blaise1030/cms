//! Agent-driven conflict resolution: result parsing, prompt, and verification.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::conflicts;
use crate::error::ApiError;

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
    eprintln!("workbench: auto-resolve verification failed ({e})");
    Failure::AgentFailed
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

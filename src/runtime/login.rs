//! Provider sign-in, modelled on OpenBot's `CliLoginFlow`.
//!
//! orso never reads or copies credential files. It spawns the provider's own
//! login command, shows the URL (and one-time code) the CLI prints, forwards a
//! pasted code to the CLI's stdin, and asks the CLI's status command whether the
//! account is connected. One pending login is kept per provider; cancelling
//! kills the whole process group.

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, Command};
use tokio::sync::Mutex as AsyncMutex;

use super::resolve::RuntimeResolver;

/// How long `start` waits for the CLI to print a URL or ask for a code.
const INITIAL_WAIT: Duration = Duration::from_secs(3);
const STATUS_TIMEOUT: Duration = Duration::from_secs(6);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const TAIL_LINES: usize = 8;

/// The state the setup UI renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SignInState {
    /// The browser is open (or should be); waiting for it.
    Browser,
    /// The CLI asked for a pasted code.
    Code,
    /// The provider reports a connected account.
    Connected,
    /// The user cancelled, or signed out.
    Cancelled,
    /// The flow could not start or finish.
    Failed,
}

/// `POST /api/setup/providers/{id}/sign-in` response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignInReply {
    pub state: SignInState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub needs_code: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl SignInReply {
    fn failed(message: impl Into<String>) -> Self {
        Self {
            state: SignInState::Failed,
            url: None,
            needs_code: false,
            message: Some(message.into()),
        }
    }

    fn cancelled(message: impl Into<String>) -> Self {
        Self {
            state: SignInState::Cancelled,
            url: None,
            needs_code: false,
            message: Some(message.into()),
        }
    }
}

type Opener = Arc<dyn Fn(&str) + Send + Sync>;

struct Pending {
    reply: Arc<Mutex<SignInReply>>,
    stdin: Arc<AsyncMutex<Option<ChildStdin>>>,
    pid: Option<u32>,
    finished: Arc<AtomicBool>,
    tail: Arc<Mutex<VecDeque<String>>>,
    opened: Mutex<Vec<String>>,
}

/// One sign-in flow per provider.
#[derive(Clone)]
pub struct LoginManager {
    resolver: RuntimeResolver,
    pending: Arc<Mutex<HashMap<String, Arc<Pending>>>>,
    opener: Opener,
    flow_timeout: Duration,
}

impl LoginManager {
    pub fn new(resolver: RuntimeResolver) -> Self {
        Self {
            resolver,
            pending: Arc::new(Mutex::new(HashMap::new())),
            opener: Arc::new(open_url_default),
            flow_timeout: LOGIN_TIMEOUT,
        }
    }

    /// Replace the browser opener (tests).
    pub fn with_opener(mut self, opener: impl Fn(&str) + Send + Sync + 'static) -> Self {
        self.opener = Arc::new(opener);
        self
    }

    /// How long a flow may stay silent before it is stopped (tests).
    pub fn with_flow_timeout(mut self, timeout: Duration) -> Self {
        self.flow_timeout = timeout;
        self
    }

    /// Whether this manager knows a sign-in flow for the provider.
    pub fn supports(provider_id: &str) -> bool {
        matches!(provider_id, "opencode" | "claude" | "codex" | "cursor")
    }

    /// Start a flow, or return the pending one. Safe to poll.
    pub async fn start(&self, provider_id: &str) -> SignInReply {
        if !Self::supports(provider_id) {
            return SignInReply::failed(format!("{provider_id} does not support sign-in"));
        }
        if let Some(pending) = self.get(provider_id) {
            let reply = current(&pending);
            if matches!(reply.state, SignInState::Browser | SignInState::Code) {
                return reply;
            }
            // A finished, failed or cancelled flow is replaced on the next
            // attempt so a stale result never blocks signing in again.
            lock(&self.pending).remove(provider_id);
        }
        let executable = match self.resolver.resolve(provider_id) {
            Ok(resolved) => resolved.executable,
            Err(_) => {
                return SignInReply::failed(format!(
                    "{} is not installed yet. Set it up first.",
                    display(provider_id)
                ))
            }
        };
        let reply = self
            .start_args(provider_id, &executable, &login_args(provider_id))
            .await;
        // Codex's browser callback can fail on headless or locked-down Macs;
        // retry once with its device-code flow.
        if provider_id == "codex" && reply.state == SignInState::Failed {
            if let Some(pending) = self.take(provider_id) {
                pending.finished.store(true, Ordering::SeqCst);
                kill_group(pending.pid);
            }
            return self
                .start_args(provider_id, &executable, &["login", "--device-auth"])
                .await;
        }
        reply
    }

    async fn start_args(&self, provider_id: &str, executable: &Path, args: &[&str]) -> SignInReply {
        match self.spawn_flow(provider_id, executable, args).await {
            Ok(pending) => match self.wait_for_prompt(&pending).await {
                Some(reply) => reply,
                None => current(&pending),
            },
            Err(message) => SignInReply::failed(message),
        }
    }

    /// Current reply for a pending (or last) flow, if any.
    pub fn current(&self, provider_id: &str) -> Option<SignInReply> {
        self.get(provider_id).map(|pending| current(&pending))
    }

    /// Write a pasted code to the login process.
    pub async fn submit_code(&self, provider_id: &str, code: &str) -> SignInReply {
        let code = code.trim();
        if code.is_empty() {
            return SignInReply::failed("Enter the code from the sign-in page.");
        }
        let Some(pending) = self.get(provider_id) else {
            return SignInReply::failed("Sign-in is not waiting for a code.");
        };
        if pending.finished.load(Ordering::SeqCst) {
            return SignInReply::failed("Sign-in is not waiting for a code.");
        }
        let mut stdin = pending.stdin.lock().await;
        let Some(stdin) = stdin.as_mut() else {
            return SignInReply::failed("Sign-in is not waiting for a code.");
        };
        if let Err(error) = stdin.write_all(format!("{code}\n").as_bytes()).await {
            return SignInReply::failed(format!("Could not send the code: {error}"));
        }
        if let Err(error) = stdin.flush().await {
            return SignInReply::failed(format!("Could not send the code: {error}"));
        }
        set_message(&pending, "Checking the code…");
        tokio::time::sleep(Duration::from_millis(800)).await;
        current(&pending)
    }

    /// Stop the flow and kill its process group.
    pub fn cancel(&self, provider_id: &str) -> SignInReply {
        let Some(pending) = self.take(provider_id) else {
            return SignInReply::cancelled("Sign-in cancelled.");
        };
        pending.finished.store(true, Ordering::SeqCst);
        kill_group(pending.pid);
        *lock(&pending.reply) = SignInReply::cancelled("Sign-in cancelled.");
        SignInReply::cancelled("Sign-in cancelled.")
    }

    /// Sign out through the provider's own CLI.
    pub async fn sign_out(&self, provider_id: &str) -> SignInReply {
        if !Self::supports(provider_id) {
            return SignInReply::failed(format!("{provider_id} does not support sign-out"));
        }
        if let Some(pending) = self.take(provider_id) {
            pending.finished.store(true, Ordering::SeqCst);
            kill_group(pending.pid);
        }
        let Ok(executable) = self.resolver.resolve(provider_id).map(|r| r.executable) else {
            return SignInReply::failed(format!("{} is not installed yet.", display(provider_id)));
        };
        let args = logout_args(provider_id);
        let result = run_status(&executable, &args, &self.child_path(provider_id)).await;
        match result {
            Some(output) if output.status.success() => {
                SignInReply::cancelled(format!("Signed out of {}.", display(provider_id)))
            }
            Some(output) => SignInReply::failed(format!(
                "Could not sign out of {}: {}",
                display(provider_id),
                tail_text(&output.stderr)
            )),
            None => SignInReply::failed(format!("Could not sign out of {}.", display(provider_id))),
        }
    }

    /// Ask the provider's status command whether an account is connected.
    /// `None` means the CLI could not tell us.
    pub async fn signed_in(&self, provider_id: &str) -> Option<bool> {
        if !Self::supports(provider_id) {
            return None;
        }
        let executable = self.resolver.resolve(provider_id).ok()?.executable;
        match provider_id {
            "opencode" => {
                let output = run_status(
                    &executable,
                    &["auth", "list", "--format", "json"],
                    &self.child_path(provider_id),
                )
                .await?;
                parse_opencode_auth(&output)
            }
            "codex" => {
                let output = run_status(
                    &executable,
                    &["login", "status"],
                    &self.child_path(provider_id),
                )
                .await?;
                parse_login_status(&output)
            }
            "cursor" => {
                let output =
                    run_status(&executable, &["status"], &self.child_path(provider_id)).await?;
                parse_login_status(&output)
            }
            "claude" => {
                let output = run_status(
                    &executable,
                    &["auth", "status"],
                    &self.child_path(provider_id),
                )
                .await?;
                match parse_claude_status(&output) {
                    Some(signed_in) => Some(signed_in),
                    None => claude_acp_signed_in(&self.resolver).await,
                }
            }
            _ => None,
        }
    }

    fn get(&self, provider_id: &str) -> Option<Arc<Pending>> {
        lock(&self.pending).get(provider_id).cloned()
    }

    fn take(&self, provider_id: &str) -> Option<Arc<Pending>> {
        lock(&self.pending).remove(provider_id)
    }

    fn child_path(&self, provider_id: &str) -> std::ffi::OsString {
        self.resolver.child_path(Some(provider_id))
    }

    async fn spawn_flow(
        &self,
        provider_id: &str,
        executable: &Path,
        args: &[&str],
    ) -> Result<Arc<Pending>, String> {
        let mut command = Command::new(executable);
        command
            .args(args)
            .env("PATH", self.child_path(provider_id))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .process_group(0);
        let mut child = command.spawn().map_err(|error| {
            format!(
                "Could not start the {} sign-in: {error}",
                display(provider_id)
            )
        })?;
        let pid = child.id();
        let stdin = Arc::new(AsyncMutex::new(child.stdin.take()));
        let reply = Arc::new(Mutex::new(SignInReply {
            state: SignInState::Browser,
            url: None,
            needs_code: false,
            message: Some("Waiting for the sign-in page…".into()),
        }));
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        let opened = Mutex::new(Vec::new());
        let pending = Arc::new(Pending {
            reply: reply.clone(),
            stdin,
            pid,
            finished: Arc::new(AtomicBool::new(false)),
            tail: tail.clone(),
            opened,
        });
        lock(&self.pending).insert(provider_id.to_string(), pending.clone());

        for stream in [
            child
                .stdout
                .take()
                .map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
            child
                .stderr
                .take()
                .map(|s| Box::new(s) as Box<dyn tokio::io::AsyncRead + Unpin + Send>),
        ]
        .into_iter()
        .flatten()
        {
            let reply = reply.clone();
            let tail = tail.clone();
            let opened = pending.clone();
            let opener = self.opener.clone();
            let provider = provider_id.to_string();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stream).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    absorb_line(&provider, &line, &reply, &tail, &opened, &opener);
                }
            });
        }

        let pending_for_wait = pending.clone();
        let provider = provider_id.to_string();
        let resolver = self.resolver.clone();
        let flow_timeout = self.flow_timeout;
        tokio::spawn(async move {
            let waited = tokio::time::timeout(flow_timeout, child.wait()).await;
            if pending_for_wait.finished.load(Ordering::SeqCst) {
                return;
            }
            let status = match waited {
                Ok(status) => status,
                Err(_) => {
                    kill_group(pid);
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    let mut state = lock(&pending_for_wait.reply);
                    state.state = SignInState::Failed;
                    state.url = None;
                    state.needs_code = false;
                    state.message = Some(format!(
                        "Signing in to {} took too long. Try again.",
                        display(&provider)
                    ));
                    pending_for_wait.finished.store(true, Ordering::SeqCst);
                    return;
                }
            };
            // Give the CLI's credential store a moment to settle.
            tokio::time::sleep(Duration::from_millis(400)).await;
            let mut signed_in = signed_in_with(&resolver, &provider).await;
            // Device flows sometimes exit just before the credential store is
            // updated; poll for a few seconds before calling it a failure.
            if signed_in == Some(false) && status.as_ref().is_ok_and(|status| status.success()) {
                let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
                while tokio::time::Instant::now() < deadline {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    signed_in = signed_in_with(&resolver, &provider).await;
                    if signed_in != Some(false) {
                        break;
                    }
                }
            }
            let mut state = lock(&pending_for_wait.reply);
            if signed_in == Some(false) {
                state.state = SignInState::Failed;
                state.message = Some(format!(
                    "{} did not finish signing in.{}",
                    display(&provider),
                    tail_suffix(&pending_for_wait.tail)
                ));
            } else if status.as_ref().is_ok_and(|s| s.success()) || signed_in == Some(true) {
                state.state = SignInState::Connected;
                state.url = None;
                state.needs_code = false;
                state.message = Some(format!("{} is connected.", display(&provider)));
            } else {
                state.state = SignInState::Failed;
                state.message = Some(format!(
                    "{} sign-in stopped before it finished.{}",
                    display(&provider),
                    tail_suffix(&pending_for_wait.tail)
                ));
            }
            pending_for_wait.finished.store(true, Ordering::SeqCst);
        });
        Ok(pending)
    }

    async fn wait_for_prompt(&self, pending: &Arc<Pending>) -> Option<SignInReply> {
        let deadline = tokio::time::Instant::now() + INITIAL_WAIT;
        loop {
            let reply = current(pending);
            if reply.url.is_some()
                || reply.needs_code
                || matches!(
                    reply.state,
                    SignInState::Connected | SignInState::Failed | SignInState::Cancelled
                )
            {
                return Some(reply);
            }
            if tokio::time::Instant::now() >= deadline {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

async fn signed_in_with(resolver: &RuntimeResolver, provider_id: &str) -> Option<bool> {
    let executable = resolver.resolve(provider_id).ok()?.executable;
    match provider_id {
        "opencode" => {
            let output = run_status(
                &executable,
                &["auth", "list", "--format", "json"],
                &resolver.child_path(Some(provider_id)),
            )
            .await?;
            parse_opencode_auth(&output)
        }
        "codex" => {
            let output = run_status(
                &executable,
                &["login", "status"],
                &resolver.child_path(Some(provider_id)),
            )
            .await?;
            parse_login_status(&output)
        }
        "cursor" => {
            let output = run_status(
                &executable,
                &["status"],
                &resolver.child_path(Some(provider_id)),
            )
            .await?;
            parse_login_status(&output)
        }
        "claude" => {
            let output = run_status(
                &executable,
                &["auth", "status"],
                &resolver.child_path(Some(provider_id)),
            )
            .await?;
            match parse_claude_status(&output) {
                Some(signed_in) => Some(signed_in),
                None => claude_acp_signed_in(resolver).await,
            }
        }
        _ => None,
    }
}

fn login_args(provider_id: &str) -> Vec<&'static str> {
    match provider_id {
        "opencode" => vec!["auth", "login", "opencode", "--method", "device"],
        "claude" => vec!["auth", "login"],
        "codex" => vec!["login"],
        "cursor" => vec!["login"],
        _ => Vec::new(),
    }
}

fn logout_args(provider_id: &str) -> Vec<&'static str> {
    match provider_id {
        "opencode" => vec!["auth", "logout"],
        "claude" => vec!["auth", "logout"],
        "codex" => vec!["logout"],
        "cursor" => vec!["logout"],
        _ => Vec::new(),
    }
}

fn display(provider_id: &str) -> &str {
    match provider_id {
        "opencode" => "OpenCode",
        "claude" => "Claude",
        "codex" => "Codex",
        "cursor" => "Cursor",
        other => other,
    }
}

fn absorb_line(
    provider_id: &str,
    line: &str,
    reply: &Arc<Mutex<SignInReply>>,
    tail: &Arc<Mutex<VecDeque<String>>>,
    pending: &Arc<Pending>,
    opener: &Opener,
) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }
    {
        let mut tail = lock(tail);
        if tail.len() == TAIL_LINES {
            tail.pop_front();
        }
        tail.push_back(trimmed.to_string());
    }
    let lower = trimmed.to_lowercase();
    let mut state = lock(reply);
    if lower.contains("paste")
        || lower.contains("authorization code")
        || lower.contains("enter the code")
    {
        state.state = SignInState::Code;
        state.needs_code = true;
        state.message = Some("Paste the code from the sign-in page.".into());
    }
    if let Some(url) = find_url(trimmed) {
        if !lock(&pending.opened).iter().any(|seen| seen == &url) {
            lock(&pending.opened).push(url.clone());
            opener(&url);
        }
        state.url = Some(url);
        if state.state != SignInState::Code {
            state.state = SignInState::Browser;
        }
        state.message = Some("Finish signing in in your browser.".into());
    }
    if let Some(code) = find_device_code(trimmed) {
        if state.message.as_deref().is_none_or(|message| {
            message == "Waiting for the sign-in page…"
                || message == "Finish signing in in your browser."
        }) {
            state.message = Some(format!("Enter code {code} on the sign-in page."));
        }
    }
    let _ = provider_id;
}

/// Open a URL with the platform opener. Tests set `ORSO_NO_BROWSER=1` or
/// `ORSO_BROWSER_OPENER` to an executable.
fn open_url_default(url: &str) {
    if std::env::var("ORSO_NO_BROWSER").as_deref() == Ok("1") {
        return;
    }
    let mut command = if let Ok(opener) = std::env::var("ORSO_BROWSER_OPENER") {
        let mut command = std::process::Command::new(opener);
        command.arg(url);
        command
    } else if cfg!(target_os = "macos") {
        let mut command = std::process::Command::new("open");
        command.arg(url);
        command
    } else {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(url);
        command
    };
    let _ = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn find_url(line: &str) -> Option<String> {
    line.split_whitespace()
        .map(|token| {
            token.trim_matches(|c: char| {
                matches!(
                    c,
                    '(' | ')' | '[' | ']' | '<' | '>' | ',' | ';' | '"' | '\''
                )
            })
        })
        .find(|token| token.starts_with("https://") || token.starts_with("http://"))
        .map(|token| token.trim_end_matches('.').to_string())
}

fn find_device_code(line: &str) -> Option<String> {
    let lower = line.to_lowercase();
    let marker = lower
        .find("enter code")
        .or_else(|| lower.find("code:"))
        .or_else(|| lower.find("code is"))?;
    let rest = &line[marker..];
    let rest = rest
        .split_once(':')
        .map(|(_, rest)| rest)
        .or_else(|| rest.split_once("code is ").map(|(_, rest)| rest))
        .unwrap_or(rest);
    rest.split_whitespace()
        .map(|token| token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-'))
        .find(|token| {
            token.len() >= 4
                && token.len() <= 16
                && !token.eq_ignore_ascii_case("code")
                && token.chars().any(|c| c.is_ascii_digit())
        })
        .map(str::to_string)
}

fn parse_claude_status(output: &std::process::Output) -> Option<bool> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = format!("{}\n{}", stdout, String::from_utf8_lossy(&output.stderr));
    let trimmed = stdout.trim();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        for key in ["loggedIn", "signedIn", "authenticated", "isLoggedIn"] {
            if let Some(value) = value.get(key).and_then(serde_json::Value::as_bool) {
                return Some(value);
            }
        }
        if value
            .get("account")
            .is_some_and(|account| !account.is_null())
        {
            return Some(true);
        }
        if value
            .get("email")
            .and_then(serde_json::Value::as_str)
            .is_some()
        {
            return Some(true);
        }
    }
    let lower = text.to_lowercase();
    if lower.contains("not logged in")
        || lower.contains("not signed in")
        || lower.contains("logged out")
        || lower.contains("expired")
        || lower.contains("invalid api key")
        || lower.contains("please run")
    {
        return Some(false);
    }
    if lower.contains("unknown command")
        || lower.contains("unrecognized command")
        || lower.contains("unrecognized option")
        || lower.contains("usage:")
    {
        return None;
    }
    Some(output.status.success())
}

/// Codex and Cursor report a plain status line. Prefer the text when it is
/// explicit, otherwise fall back to the exit code.
fn parse_login_status(output: &std::process::Output) -> Option<bool> {
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lower = text.to_lowercase();
    if lower.contains("not logged in")
        || lower.contains("not signed in")
        || lower.contains("logged out")
        || lower.contains("no credentials")
        || lower.contains("expired")
    {
        return Some(false);
    }
    if output.status.success() {
        return Some(true);
    }
    if lower.contains("login")
        || lower.contains("sign in")
        || lower.contains("auth")
        || lower.contains("credential")
    {
        return Some(false);
    }
    None
}

/// `claude auth status` is not present in every Claude build. When it does not
/// answer, start the adapter (or CLI) just far enough to complete an ACP
/// `initialize`, which needs a working account.
async fn claude_acp_signed_in(resolver: &RuntimeResolver) -> Option<bool> {
    let acp = resolver.acp("claude")?;
    if acp
        .argv
        .iter()
        .any(|arg| arg == "npx" || arg == "--yes" || arg.contains("npx "))
    {
        // Never trigger an unpinned download just to probe auth.
        return None;
    }
    let (command, args) = acp.argv.split_first()?;
    let dir = std::env::temp_dir().join(format!(
        "orso-claude-auth-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    tokio::fs::create_dir_all(&dir).await.ok()?;
    let path = resolver
        .child_path(Some("claude"))
        .to_string_lossy()
        .into_owned();
    let result = tokio::time::timeout(Duration::from_secs(8), async {
        let client =
            crate::chat::acp::AcpClient::spawn_with_env(command, args, &dir, &[("PATH", &path)])
                .await?;
        let initialized = client.initialize().await;
        let _ = client.shutdown().await;
        initialized.map(|_| ())
    })
    .await;
    let _ = tokio::fs::remove_dir_all(&dir).await;
    match result {
        Ok(Ok(())) => Some(true),
        Ok(Err(error)) => {
            let message = error.to_string().to_lowercase();
            if message.contains("login")
                || message.contains("sign in")
                || message.contains("auth")
                || message.contains("credential")
                || message.contains("unauthorized")
            {
                Some(false)
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

fn parse_opencode_auth(output: &std::process::Output) -> Option<bool> {
    let text = String::from_utf8_lossy(&output.stdout);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return output.status.success().then_some(false);
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return Some(json_has_entries(&value));
    }
    let lower = trimmed.to_lowercase();
    if lower.contains("no credentials") || lower.contains("not logged in") {
        return Some(false);
    }
    None
}

fn json_has_entries(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Array(items) => !items.is_empty(),
        serde_json::Value::Object(entries) => !entries.is_empty(),
        _ => false,
    }
}

async fn run_status(
    executable: &Path,
    args: &[&str],
    path: &std::ffi::OsStr,
) -> Option<std::process::Output> {
    tokio::time::timeout(
        STATUS_TIMEOUT,
        Command::new(executable)
            .args(args)
            .env("PATH", path)
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()
}

fn kill_group(pid: Option<u32>) {
    #[cfg(unix)]
    if let Some(pid) = pid {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
    let _ = pid;
}

fn current(pending: &Arc<Pending>) -> SignInReply {
    lock(&pending.reply).clone()
}

fn set_message(pending: &Arc<Pending>, message: &str) {
    lock(&pending.reply).message = Some(message.to_string());
}

fn tail_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim();
    if text.is_empty() {
        "the command exited with an error".into()
    } else {
        text.lines().last().unwrap_or(text).to_string()
    }
}

fn tail_suffix(tail: &Arc<Mutex<VecDeque<String>>>) -> String {
    let tail = lock(tail);
    match tail.back() {
        Some(line) => format!(" ({line})"),
        None => String::new(),
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::descriptors::LockFile;
    use crate::runtime::RuntimeTarget;

    fn empty_lock() -> LockFile {
        LockFile {
            schema_version: 1,
            runtimes: Default::default(),
            acp: Default::default(),
        }
    }

    fn manager(dir: &Path, name: &str, script: &str) -> LoginManager {
        let executable = dir.join(name);
        std::fs::write(&executable, script).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let resolver =
            RuntimeResolver::new(dir.join("store"), empty_lock(), RuntimeTarget::DarwinArm64)
                .with_login_shell_path(dir.as_os_str().to_os_string())
                .with_daemon_path("/usr/bin:/bin");
        LoginManager::new(resolver)
            .with_opener(|_url: &str| {})
            .with_flow_timeout(Duration::from_secs(3))
    }

    async fn wait_for(manager: &LoginManager, provider: &str, state: SignInState) -> SignInReply {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(6);
        loop {
            if let Some(reply) = manager.current(provider) {
                if reply.state == state {
                    return reply;
                }
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out waiting for {state:?}: {:?}",
                manager.current(provider)
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    #[tokio::test]
    async fn opencode_parses_the_url_code_and_connects_after_submission() {
        let dir = tempfile::tempdir().unwrap();
        let script = "#!/bin/sh\n\
             if [ \"$1\" = \"auth\" ] && [ \"$2\" = \"list\" ]; then echo '[{\"provider\":\"opencode\"}]'; exit 0; fi\n\
             if [ \"$1\" = \"auth\" ] && [ \"$2\" = \"login\" ]; then\n\
               echo \"Open https://example.test/device\"\n\
               echo \"Enter code: ABCD-1234\"\n\
               read code\n\
               [ \"$code\" = \"ABCD-1234\" ] || exit 1\n\
               exit 0\n\
             fi\n\
             exit 1\n";
        let logins = manager(dir.path(), "opencode", script);
        let reply = logins.start("opencode").await;
        assert_eq!(reply.state, SignInState::Browser);
        assert_eq!(reply.url.as_deref(), Some("https://example.test/device"));
        assert!(reply.message.as_deref().unwrap().contains("ABCD-1234"));

        let reply = logins.submit_code("opencode", "ABCD-1234").await;
        assert!(
            matches!(reply.state, SignInState::Browser | SignInState::Connected),
            "{reply:?}"
        );
        let reply = wait_for(&logins, "opencode", SignInState::Connected).await;
        assert_eq!(reply.message.as_deref(), Some("OpenCode is connected."));
    }

    #[tokio::test]
    async fn claude_parses_the_url_and_uses_auth_status() {
        let dir = tempfile::tempdir().unwrap();
        let script = "#!/bin/sh\n\
             if [ \"$1\" = \"auth\" ] && [ \"$2\" = \"status\" ]; then echo '{\"loggedIn\":true}'; exit 0; fi\n\
             if [ \"$1\" = \"auth\" ] && [ \"$2\" = \"login\" ]; then echo \"Visit https://claude.test/login\"; exit 0; fi\n\
             exit 1\n";
        let logins = manager(dir.path(), "claude", script);
        let reply = logins.start("claude").await;
        assert_eq!(reply.state, SignInState::Browser);
        assert_eq!(reply.url.as_deref(), Some("https://claude.test/login"));
        assert!(!reply.needs_code);
        wait_for(&logins, "claude", SignInState::Connected).await;
    }

    #[tokio::test]
    async fn cancelled_sign_in_forgets_the_flow() {
        let dir = tempfile::tempdir().unwrap();
        let script = "#!/bin/sh\n\
             if [ \"$1\" = \"auth\" ] && [ \"$2\" = \"login\" ]; then\n\
               echo \"Open https://example.test/device\"\n\
               sleep 5\n\
             fi\n\
             exit 1\n";
        let logins = manager(dir.path(), "opencode", script);
        let reply = logins.start("opencode").await;
        assert_eq!(reply.state, SignInState::Browser);
        let reply = logins.cancel("opencode");
        assert_eq!(reply.state, SignInState::Cancelled);
        assert!(logins.current("opencode").is_none());
    }

    #[tokio::test]
    async fn expired_login_reports_a_plain_failure() {
        let dir = tempfile::tempdir().unwrap();
        let script = "#!/bin/sh\n\
             if [ \"$1\" = \"auth\" ] && [ \"$2\" = \"list\" ]; then echo '[]'; exit 0; fi\n\
             if [ \"$1\" = \"auth\" ] && [ \"$2\" = \"login\" ]; then\n\
               echo \"This sign-in link has expired.\"\n\
               exit 1\n\
             fi\n\
             exit 1\n";
        let logins = manager(dir.path(), "opencode", script);
        let reply = logins.start("opencode").await;
        assert_eq!(reply.state, SignInState::Failed, "{reply:?}");
        assert!(reply.message.as_deref().unwrap().contains("did not finish"));
    }

    #[tokio::test]
    async fn silent_login_never_stays_connecting_forever() {
        let dir = tempfile::tempdir().unwrap();
        let script = "#!/bin/sh\n\
             if [ \"$1\" = \"auth\" ] && [ \"$2\" = \"login\" ]; then\n\
               echo \"Open https://example.test/device\"\n\
               sleep 30\n\
             fi\n\
             exit 1\n";
        let logins =
            manager(dir.path(), "opencode", script).with_flow_timeout(Duration::from_millis(300));
        let reply = logins.start("opencode").await;
        assert_eq!(reply.state, SignInState::Browser);
        let reply = wait_for(&logins, "opencode", SignInState::Failed).await;
        assert!(
            reply.message.as_deref().unwrap().contains("took too long"),
            "{reply:?}"
        );
    }

    #[test]
    fn parses_device_codes_and_urls() {
        assert_eq!(
            find_url("Open https://example.test/device?code=1.").as_deref(),
            Some("https://example.test/device?code=1")
        );
        assert_eq!(
            find_device_code("Enter code: ABCD-1234").as_deref(),
            Some("ABCD-1234")
        );
        assert_eq!(find_device_code("Waiting for you"), None);
        assert_eq!(find_url("no link here"), None);
    }

    #[test]
    fn claude_status_fixtures() {
        use std::os::unix::process::ExitStatusExt;
        let output = |stdout: &str, stderr: &str, code: i32| std::process::Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        };
        assert_eq!(
            parse_claude_status(&output(r#"{"loggedIn":true}"#, "", 0)),
            Some(true)
        );
        assert_eq!(
            parse_claude_status(&output(r#"{"loggedIn":false}"#, "", 0)),
            Some(false)
        );
        assert_eq!(
            parse_claude_status(&output("", "error: unknown command 'auth'", 1)),
            None
        );
        assert_eq!(
            parse_claude_status(&output("Not logged in", "", 1)),
            Some(false)
        );
        assert_eq!(
            parse_login_status(&output("Logged in as a@b.test", "", 0)),
            Some(true)
        );
        assert_eq!(
            parse_login_status(&output("Not logged in", "", 1)),
            Some(false)
        );
    }
}

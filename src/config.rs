//! Runtime configuration for `workbench serve`.

use std::ffi::OsString;
use std::path::PathBuf;

use rand::Rng;

/// Where GitHub lives. Overridable so tests can point at a fake server and a local bare repo.
#[derive(Debug, Clone)]
pub struct GithubConfig {
    /// OAuth App client id (device flow enabled). `None` disables sign-in.
    pub client_id: Option<String>,
    /// REST API base, e.g. `https://api.github.com`.
    pub api_url: String,
    /// Web base for `/login/device/code` and `/login/oauth/access_token`.
    pub web_url: String,
    /// Base for clone URLs: `{git_url}/{owner}/{repo}.git`.
    pub git_url: String,
    /// The GitHub CLI binary. An empty path disables the CLI login.
    pub gh_path: PathBuf,
    /// Global git config file to read the commit identity from (tests); `None` = the real one.
    pub git_global_config: Option<PathBuf>,
}

impl Default for GithubConfig {
    fn default() -> Self {
        Self {
            client_id: None,
            api_url: "https://api.github.com".into(),
            web_url: "https://github.com".into(),
            git_url: "https://github.com".into(),
            gh_path: "gh".into(),
            git_global_config: None,
        }
    }
}

/// Resolve `gh` against `PATH` so git's credential helper gets an absolute path.
/// A path containing `/` is kept as is; an unresolvable name is returned unchanged.
pub fn resolve_gh(gh: &std::path::Path) -> PathBuf {
    if gh.as_os_str().is_empty() || gh.components().count() > 1 || gh.is_absolute() {
        return gh.to_path_buf();
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|d| d.join(gh))
        .find(|c| c.is_file())
        .unwrap_or_else(|| gh.to_path_buf())
}

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    /// Hosted PWA origin allowed via CORS + Host allowlist.
    pub origin: Option<String>,
    pub projects_dir: PathBuf,
    pub no_auth: bool,
    /// Auth token. `None` only when `no_auth` is set.
    pub token: Option<String>,
    pub agents_config: Option<PathBuf>,
    pub github: GithubConfig,
    /// Base URL printed in the ready link instead of the daemon's own address (dev: the Vite
    /// server, so the link opens the live HMR app rather than the embedded `web/dist` build).
    pub ui_url: Option<String>,
    /// Executable terminal CLIs run as `<exe> hook turn-end`. `None` = this daemon's own
    /// binary ([`Config::hook_exe_path`]); tests point it at the built `workbench`.
    pub hook_exe: Option<PathBuf>,
    /// Directories the setup check searches for git, gh and the agent CLIs, `PATH`-style.
    /// `None` = the daemon's own `PATH`.
    pub search_path: Option<OsString>,
}

/// Tests build configs with `..Default::default()`, so the default must never run the
/// `gh` installed on the machine: the CLI login is off unless a config enables it.
impl Default for Config {
    fn default() -> Self {
        Self {
            port: 0,
            origin: None,
            projects_dir: PathBuf::new(),
            no_auth: false,
            token: None,
            agents_config: None,
            github: GithubConfig {
                gh_path: PathBuf::new(),
                ..GithubConfig::default()
            },
            ui_url: None,
            hook_exe: None,
            search_path: None,
        }
    }
}

impl Config {
    /// Absolute path of the hook executable: `hook_exe`, else the running binary.
    pub fn hook_exe_path(&self) -> PathBuf {
        self.hook_exe
            .clone()
            .or_else(|| std::env::current_exe().ok())
            .unwrap_or_else(|| PathBuf::from("workbench"))
    }

    /// Reject configs whose token is not URL/cookie safe.
    pub fn validate(&self) -> Result<(), String> {
        match &self.token {
            Some(t) => parse_token(t).map(|_| ()),
            None => Ok(()),
        }
    }

    /// Fill in the token: explicit `--token`, else 32 random bytes as hex.
    /// With `--no-auth` no token is generated.
    pub fn resolve_token(no_auth: bool, token: Option<String>) -> Option<String> {
        if no_auth {
            token
        } else {
            Some(token.unwrap_or_else(random_token))
        }
    }
}

/// Longest accepted explicit token.
pub const MAX_TOKEN_LEN: usize = 128;

/// Validate a token: non-empty hex (`[0-9a-fA-F]`), at most [`MAX_TOKEN_LEN`] chars.
/// This is the same alphabet as the random default, so it is safe in URLs and cookies.
pub fn parse_token(s: &str) -> Result<String, String> {
    if s.is_empty() || s.len() > MAX_TOKEN_LEN || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "token must be 1-{MAX_TOKEN_LEN} hex characters (0-9, a-f), so it is safe in URLs and cookies"
        ));
    }
    Ok(s.to_string())
}

/// 32 random bytes, hex-encoded.
pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_token_is_valid() {
        let t = random_token();
        assert_eq!(t.len(), 64);
        assert!(parse_token(&t).is_ok());
    }

    #[test]
    fn rejects_reserved_and_unsafe_tokens() {
        for bad in [
            "",
            "a;b",
            "a b",
            "a&t=b",
            "a=b",
            "a%20b",
            "a\r\nb",
            "tok\u{e9}n",
            "a,b",
            "\"x\"",
            "g",
            "secret",
        ] {
            assert!(parse_token(bad).is_err(), "should reject {bad:?}");
        }
        assert!(parse_token(&"a".repeat(MAX_TOKEN_LEN + 1)).is_err());
        assert!(parse_token("abc123DEF").is_ok());
    }

    #[test]
    fn validate_checks_configured_token() {
        let mut c = Config {
            port: 0,
            origin: None,
            projects_dir: ".".into(),
            no_auth: false,
            token: Some("a;b".into()),
            agents_config: None,
            ..Default::default()
        };
        assert!(c.validate().is_err());
        c.token = Some("ab12".into());
        assert!(c.validate().is_ok());
    }
}

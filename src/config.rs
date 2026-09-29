//! Runtime configuration for `workbench serve`.

use std::path::PathBuf;

use rand::Rng;

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
}

impl Config {
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
        };
        assert!(c.validate().is_err());
        c.token = Some("ab12".into());
        assert!(c.validate().is_ok());
    }
}

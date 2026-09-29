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

/// 32 random bytes, hex-encoded.
pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

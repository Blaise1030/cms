//! Shared error model: `{ "error": { "code", "message" } }`.

use std::fmt;

use serde::Serialize;

/// Error codes from the spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    Unauth,
    Sandbox,
    Enoent,
    Conflict,
    AgentErr,
    DevserverErr,
    GitErr,
    Io,
    Badreq,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unauth => "UNAUTH",
            Self::Sandbox => "SANDBOX",
            Self::Enoent => "ENOENT",
            Self::Conflict => "CONFLICT",
            Self::AgentErr => "AGENT_ERR",
            Self::DevserverErr => "DEVSERVER_ERR",
            Self::GitErr => "GIT_ERR",
            Self::Io => "IO",
            Self::Badreq => "BADREQ",
        }
    }

    /// Default HTTP status for this code.
    pub fn status(self) -> u16 {
        match self {
            Self::Unauth => 401,
            Self::Sandbox => 403,
            Self::Enoent => 404,
            Self::Conflict => 409,
            Self::Badreq => 400,
            Self::AgentErr | Self::DevserverErr => 502,
            Self::GitErr | Self::Io => 500,
        }
    }
}

/// Transport-independent API error.
#[derive(Debug, Clone)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
    /// Overrides `code.status()` when set.
    pub status: Option<u16>,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            status: None,
        }
    }

    pub fn with_status(mut self, status: u16) -> Self {
        self.status = Some(status);
        self
    }

    pub fn http_status(&self) -> u16 {
        self.status.unwrap_or_else(|| self.code.status())
    }

    pub fn unauth(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauth, message)
    }
    pub fn sandbox(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Sandbox, message)
    }
    pub fn enoent(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Enoent, message)
    }
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, message)
    }
    pub fn badreq(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Badreq, message)
    }
    pub fn io(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Io, message)
    }
    pub fn agent(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::AgentErr, message)
    }
    pub fn git(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::GitErr, message)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::NotFound {
            Self::enoent(e.to_string())
        } else {
            Self::io(e.to_string())
        }
    }
}

#[derive(Serialize)]
pub struct ErrorBody<'a> {
    pub error: ErrorDetail<'a>,
}

#[derive(Serialize)]
pub struct ErrorDetail<'a> {
    pub code: &'a str,
    pub message: &'a str,
}

impl ApiError {
    pub fn body(&self) -> ErrorBody<'_> {
        ErrorBody {
            error: ErrorDetail {
                code: self.code.as_str(),
                message: &self.message,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_shape() {
        let v = serde_json::to_value(ApiError::conflict("x").body()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"error": {"code": "CONFLICT", "message": "x"}})
        );
    }

    #[test]
    fn all_codes_match_spec() {
        use ErrorCode::*;
        let got: Vec<_> = [
            Unauth,
            Sandbox,
            Enoent,
            Conflict,
            AgentErr,
            DevserverErr,
            GitErr,
            Io,
            Badreq,
        ]
        .iter()
        .map(|c| c.as_str())
        .collect();
        assert_eq!(
            got,
            [
                "UNAUTH",
                "SANDBOX",
                "ENOENT",
                "CONFLICT",
                "AGENT_ERR",
                "DEVSERVER_ERR",
                "GIT_ERR",
                "IO",
                "BADREQ"
            ]
        );
    }
}

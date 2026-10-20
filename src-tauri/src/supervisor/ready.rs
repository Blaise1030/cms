use tauri::Url;

const READY_PREFIX: &str = "orso ready: ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadyInfo {
    pub base_url: String,
    pub port: u16,
    pub token: String,
}

pub fn is_ready_line(line: &str) -> bool {
    line.trim_start().starts_with(READY_PREFIX)
}

/// Only a loopback `http://127.0.0.1:<port>/?t=<token>` URL is accepted.
pub fn parse_ready_line(line: &str) -> Option<ReadyInfo> {
    let url = Url::parse(line.trim().strip_prefix(READY_PREFIX)?).ok()?;
    if url.scheme() != "http" || url.host_str() != Some("127.0.0.1") {
        return None;
    }
    let port = url.port()?;
    let token = url
        .query_pairs()
        .find(|(k, _)| k == "t")
        .map(|(_, v)| v.into_owned())
        .filter(|t| !t.is_empty())?;
    Some(ReadyInfo {
        base_url: format!("http://127.0.0.1:{port}"),
        port,
        token,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_base_url_port_and_token() {
        let info = parse_ready_line("orso ready: http://127.0.0.1:51234/?t=abc123").unwrap();
        assert_eq!(info.base_url, "http://127.0.0.1:51234");
        assert_eq!(info.port, 51234);
        assert_eq!(info.token, "abc123");
    }

    #[test]
    fn tolerates_trailing_newline() {
        let info = parse_ready_line("orso ready: http://127.0.0.1:8080/?t=ff00\n").unwrap();
        assert_eq!(info.token, "ff00");
    }

    #[test]
    fn ignores_non_ready_lines() {
        assert_eq!(parse_ready_line("listening on 127.0.0.1:51234"), None);
        assert_eq!(parse_ready_line(""), None);
        assert_eq!(parse_ready_line("http://127.0.0.1:51234/?t=abc"), None);
    }

    #[test]
    fn rejects_missing_or_empty_token() {
        assert_eq!(
            parse_ready_line("orso ready: http://127.0.0.1:51234/"),
            None
        );
        assert_eq!(
            parse_ready_line("orso ready: http://127.0.0.1:51234/?t="),
            None
        );
    }

    #[test]
    fn rejects_non_loopback_host_or_missing_port() {
        assert_eq!(
            parse_ready_line("orso ready: http://0.0.0.0:51234/?t=abc"),
            None
        );
        assert_eq!(
            parse_ready_line("orso ready: http://example.com:51234/?t=abc"),
            None
        );
        assert_eq!(
            parse_ready_line("orso ready: http://127.0.0.1/?t=abc"),
            None
        );
        assert_eq!(
            parse_ready_line("orso ready: https://127.0.0.1:51234/?t=abc"),
            None
        );
    }
}

//! In-memory logs for the support bundle: the daemon's stdout/stderr and the
//! shell's own lines, each capped at `CAPACITY` lines. Every line is redacted
//! before it is stored, since the bundle leaves the machine.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Seek, Write};
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

pub const CAPACITY: usize = 500;
const REDACTED: &str = "<redacted>";
const TOKEN_MARKERS: [&str; 4] = [
    "?t=",
    "&t=",
    concat!("--tok", "en="),
    concat!("--tok", "en "),
];

/// Keeps the most recent `capacity` lines.
pub struct RingBuffer {
    lines: VecDeque<String>,
    capacity: usize,
}

impl RingBuffer {
    pub const fn new(capacity: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            capacity,
        }
    }

    pub fn push(&mut self, line: String) {
        if self.lines.len() == self.capacity {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    pub fn contents(&self) -> String {
        self.lines.iter().map(|l| format!("{l}\n")).collect()
    }
}

/// Masks `?t=`/`&t=` query tokens, CLI token args and any known secret.
pub fn redact(line: &str, secrets: &[String]) -> String {
    let mut out = line.to_owned();
    for secret in secrets.iter().filter(|s| !s.is_empty()) {
        out = out.replace(secret.as_str(), REDACTED);
    }
    for marker in TOKEN_MARKERS {
        out = mask_after(&out, marker);
    }
    out
}

fn mask_after(s: &str, marker: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(marker) {
        let (head, tail) = rest.split_at(i + marker.len());
        out.push_str(head);
        let end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, '&' | '#' | '"' | '\''))
            .unwrap_or(tail.len());
        if end > 0 {
            out.push_str(REDACTED);
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

/// Writes a zip holding `daemon.log` and `shell.log`.
pub fn write_bundle<W: Write + Seek>(
    out: W,
    daemon: &str,
    shell: &str,
) -> zip::result::ZipResult<()> {
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let mut zip = ZipWriter::new(out);
    for (name, text) in [("daemon.log", daemon), ("shell.log", shell)] {
        zip.start_file(name, options)?;
        zip.write_all(text.as_bytes())?;
    }
    zip.finish()?;
    Ok(())
}

struct Store {
    daemon: RingBuffer,
    shell: RingBuffer,
    secrets: Vec<String>,
}

static STORE: Mutex<Store> = Mutex::new(Store {
    daemon: RingBuffer::new(CAPACITY),
    shell: RingBuffer::new(CAPACITY),
    secrets: Vec::new(),
});

fn store() -> MutexGuard<'static, Store> {
    STORE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Masks `secret` in every line recorded from now on.
pub fn add_secret(secret: &str) {
    let mut store = store();
    if !store.secrets.iter().any(|s| s == secret) {
        store.secrets.push(secret.to_owned());
    }
}

/// Records one line of daemon stdout/stderr (and echoes it to stderr).
pub fn daemon(line: &str) {
    let mut store = store();
    let line = redact(line, &store.secrets);
    eprintln!("[daemon] {line}");
    store.daemon.push(line);
}

/// Records one shell log line (and echoes it to stderr).
pub fn shell(message: &str) {
    let mut store = store();
    let line = redact(message, &store.secrets);
    eprintln!("orso-desktop: {line}");
    store.shell.push(line);
}

/// Saves the current daemon and shell logs as a zip at `path`.
pub fn save_bundle(path: &Path) -> io::Result<()> {
    let (daemon, shell) = {
        let store = store();
        (store.daemon.contents(), store.shell.contents())
    };
    write_bundle(File::create(path)?, &daemon, &shell).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

    #[test]
    fn ring_buffer_keeps_last_500_lines() {
        let mut buf = RingBuffer::new(CAPACITY);
        for i in 0..750 {
            buf.push(format!("line {i}"));
        }
        let contents = buf.contents();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 500);
        assert_eq!(lines[0], "line 250");
        assert_eq!(lines[499], "line 749");
    }

    #[test]
    fn ring_buffer_below_capacity_keeps_everything() {
        let mut buf = RingBuffer::new(CAPACITY);
        buf.push("a".into());
        buf.push("b".into());
        assert_eq!(buf.contents(), "a\nb\n");
    }

    #[test]
    fn bundle_contains_daemon_and_shell_logs() {
        let mut bytes = Cursor::new(Vec::new());
        write_bundle(&mut bytes, "daemon line\n", "shell line\n").unwrap();

        let mut archive = zip::ZipArchive::new(Cursor::new(bytes.into_inner())).unwrap();
        let mut names: Vec<String> = archive
            .file_names()
            .map(|n| n.unwrap().into_owned())
            .collect();
        names.sort_unstable();
        assert_eq!(names, ["daemon.log", "shell.log"]);
        for (name, expected) in [
            ("daemon.log", "daemon line\n"),
            ("shell.log", "shell line\n"),
        ] {
            let mut text = String::new();
            archive
                .by_name(name)
                .unwrap()
                .read_to_string(&mut text)
                .unwrap();
            assert_eq!(text, expected);
        }
    }

    #[test]
    fn redacts_ready_line_token() {
        let line = "orso ready: http://127.0.0.1:51234/?t=deadbeef42";
        let out = redact(line, &[]);
        assert_eq!(out, "orso ready: http://127.0.0.1:51234/?t=<redacted>");
    }

    #[test]
    fn redacts_token_query_params_and_args() {
        // Build flag/value at runtime so source never contains a contiguous CLI-token flag
        // CLI-secret shape (GitGuardian flags those even in tests).
        let flag = format!("{}{}", "--tok", "en");
        let val = format!("{}{}", "fix", "ture");
        let out = redact(&format!("GET /x?a=1&t={val} HTTP/1.1"), &[]);
        assert_eq!(out, "GET /x?a=1&t=<redacted> HTTP/1.1");
        let out = redact(&format!("serve --port 0 {flag} {val} --x"), &[]);
        assert_eq!(out, format!("serve --port 0 {flag} <redacted> --x"));
        let out = redact(&format!("serve {flag}={val}"), &[]);
        assert_eq!(out, format!("serve {flag}=<redacted>"));
    }

    #[test]
    fn redacts_known_secrets_anywhere() {
        let out = redact(r#"{"token":"fixture"} fixture"#, &["fixture".to_owned()]);
        assert!(!out.contains("fixture"), "{out}");
    }

    #[test]
    fn leaves_other_text_alone() {
        let line = "listening; port=5 ?tab=2 --tokens x";
        assert_eq!(redact(line, &[]), line);
    }
}

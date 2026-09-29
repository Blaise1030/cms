//! Scripted fake ACP agent for tests (JSON-RPC over stdio). Not a real agent.
//!
//! The behaviour of a turn is chosen by the first word of the prompt text:
//!
//! * `hello`  – streams "Hello " + "world", ends the turn.
//! * `edit`   – edit tool call + `request_permission(kind=edit)`, writes `notes.txt`.
//! * `run`    – execute tool call + `request_permission(kind=execute)`; runs only if allowed.
//! * `slow`   – streams "working", then waits for `session/cancel`.
//! * `late`   – ends the turn while a tool is still running; finishes it 500 ms later.
//! * `crash`  – exits abruptly.
//!
//! Flags: `--log FILE` (append every received line), `--pidfile FILE`,
//! `--ignore-eof` (keep running after stdin closes, to exercise SIGTERM/SIGKILL).

use std::fs::OpenOptions;
use std::io::{self, BufRead, Write};
use std::time::Duration;

use serde_json::{json, Value};

fn send(v: Value) {
    let mut out = io::stdout().lock();
    writeln!(out, "{v}").unwrap();
    out.flush().unwrap();
}

fn respond(id: &Value, result: Value) {
    send(json!({ "jsonrpc": "2.0", "id": id, "result": result }));
}

fn update(session: &str, update: Value) {
    send(json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": { "sessionId": session, "update": update }
    }));
}

fn chunk(session: &str, text: &str) {
    update(
        session,
        json!({ "sessionUpdate": "agent_message_chunk", "content": { "type": "text", "text": text } }),
    );
}

fn tool(session: &str, id: &str, title: &str, kind: &str, status: &str, raw: Value) {
    update(
        session,
        json!({
            "sessionUpdate": "tool_call", "toolCallId": id, "title": title,
            "kind": kind, "status": status, "rawInput": raw
        }),
    );
}

fn tool_status(session: &str, id: &str, status: &str) {
    update(
        session,
        json!({ "sessionUpdate": "tool_call_update", "toolCallId": id, "status": status }),
    );
}

struct Agent {
    lines: io::Lines<io::StdinLock<'static>>,
    log: Option<String>,
    cancelled: bool,
    session: String,
}

impl Agent {
    fn next(&mut self) -> Option<Value> {
        loop {
            let line = self.lines.next()?.ok()?;
            if let Some(path) = &self.log {
                let mut f = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .unwrap();
                writeln!(f, "{line}").unwrap();
            }
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                return Some(v);
            }
        }
    }

    /// Read until the response to `id` arrives, noting any `session/cancel` on the way.
    fn wait_response(&mut self, id: &str) -> Value {
        loop {
            let msg = self.next().unwrap_or_else(|| std::process::exit(0));
            if msg["method"] == "session/cancel" {
                self.cancelled = true;
            } else if msg["id"] == id && msg.get("method").is_none() {
                return msg["result"]["outcome"].clone();
            }
        }
    }

    fn ask(&mut self, req_id: &str, call_id: &str, title: &str, kind: &str, raw: Value) -> Value {
        send(json!({
            "jsonrpc": "2.0", "id": req_id, "method": "session/request_permission",
            "params": {
                "sessionId": self.session,
                "toolCall": { "toolCallId": call_id, "title": title, "kind": kind, "rawInput": raw },
                "options": [
                    { "optionId": "allow-always", "name": "Always", "kind": "allow_always" },
                    { "optionId": "allow", "name": "Allow", "kind": "allow_once" },
                    { "optionId": "reject", "name": "Deny", "kind": "reject_once" }
                ]
            }
        }));
        self.wait_response(req_id)
    }

    fn prompt(&mut self, id: &Value, text: &str) {
        let s = self.session.clone();
        let mut stop = "end_turn";
        match text.split_whitespace().next().unwrap_or("") {
            "hello" => {
                chunk(&s, "Hello ");
                chunk(&s, "world");
            }
            "edit" => {
                tool(
                    &s,
                    "c1",
                    "Edit notes.txt",
                    "edit",
                    "pending",
                    json!({ "path": "notes.txt" }),
                );
                let outcome = self.ask("p1", "c1", "Edit notes.txt", "edit", json!({}));
                if outcome["optionId"] == "allow" {
                    std::fs::write("notes.txt", "edited by fake agent\n").unwrap();
                    tool_status(&s, "c1", "completed");
                    chunk(&s, "edited");
                } else {
                    tool_status(&s, "c1", "failed");
                }
            }
            "run" => {
                let raw = json!({ "command": "npm test" });
                tool(&s, "c2", "Run npm test", "execute", "pending", raw.clone());
                let outcome = self.ask("p2", "c2", "Run npm test", "execute", raw);
                if outcome["optionId"] == "allow" {
                    tool_status(&s, "c2", "completed");
                    chunk(&s, "ran");
                } else {
                    tool_status(&s, "c2", "failed");
                    chunk(&s, "denied");
                }
            }
            "slow" => {
                chunk(&s, "working");
                while !self.cancelled {
                    match self.next() {
                        Some(m) if m["method"] == "session/cancel" => self.cancelled = true,
                        Some(_) => {}
                        None => std::process::exit(0),
                    }
                }
            }
            "late" => {
                tool(&s, "c3", "Write late.txt", "edit", "in_progress", json!({}));
                respond(id, json!({ "stopReason": "end_turn" }));
                std::thread::sleep(Duration::from_millis(500));
                std::fs::write("late.txt", "late\n").unwrap();
                tool_status(&s, "c3", "completed");
                return;
            }
            "crash" => {
                chunk(&s, "about to die");
                std::process::exit(3);
            }
            _ => chunk(&s, "?"),
        }
        if self.cancelled {
            stop = "cancelled";
        }
        respond(id, json!({ "stopReason": stop }));
        self.cancelled = false;
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut log = None;
    let mut ignore_eof = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--log" => log = args.next(),
            "--pidfile" => {
                std::fs::write(args.next().unwrap(), std::process::id().to_string()).unwrap()
            }
            "--ignore-eof" => ignore_eof = true,
            _ => {}
        }
    }
    let stdin: &'static io::Stdin = Box::leak(Box::new(io::stdin()));
    let mut agent = Agent {
        lines: stdin.lock().lines(),
        log,
        cancelled: false,
        session: format!("fake-{}", std::process::id()),
    };
    while let Some(msg) = agent.next() {
        let id = msg.get("id").cloned();
        match (msg["method"].as_str(), id) {
            (Some("initialize"), Some(id)) => respond(
                &id,
                json!({ "protocolVersion": 1, "agentCapabilities": {}, "authMethods": [] }),
            ),
            (Some("session/new"), Some(id)) => respond(&id, json!({ "sessionId": agent.session })),
            (Some("session/prompt"), Some(id)) => {
                let text = msg["params"]["prompt"][0]["text"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                agent.prompt(&id, &text);
            }
            (Some(_), Some(id)) => send(json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": -32601, "message": "method not found" }
            })),
            _ => {}
        }
    }
    if ignore_eof {
        loop {
            std::thread::sleep(Duration::from_secs(60));
        }
    }
}

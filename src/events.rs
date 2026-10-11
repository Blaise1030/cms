//! Event broadcaster feeding `WS /ws/events`.
//!
//! Every event is a flat JSON object tagged by `"type"` (e.g. `fs.change`). Events are
//! serialised once and fanned out to all subscribers as text frames.

use std::sync::Arc;

use serde_json::{Map, Value};
use tokio::sync::broadcast;

/// Events buffered per subscriber before a slow consumer starts lagging.
const CAPACITY: usize = 1024;

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Arc<str>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            tx: broadcast::channel(CAPACITY).0,
        }
    }

    /// Broadcast `{ "type": kind, ..fields }`. `fields` must be a JSON object. Having no
    /// subscribers is not an error.
    pub fn emit(&self, kind: &str, fields: Value) {
        let mut obj = match fields {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        // The tag is authoritative: set last so fields cannot spoof it.
        obj.insert("type".into(), Value::String(kind.to_string()));
        if let Ok(text) = serde_json::to_string(&Value::Object(obj)) {
            let _ = self.tx.send(text.into());
        }
    }

    /// Receive serialised events emitted from now on.
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<str>> {
        self.tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn events_are_flat_and_tagged() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        bus.emit("fs.change", json!({ "path": "a.md", "type": "spoofed" }));
        let v: Value = serde_json::from_str(&rx.try_recv().unwrap()).unwrap();
        assert_eq!(v["path"], "a.md");
        assert_eq!(v["type"], "fs.change");
    }

    #[test]
    fn emit_without_subscribers_is_fine() {
        EventBus::new().emit("x", json!({}));
    }
}

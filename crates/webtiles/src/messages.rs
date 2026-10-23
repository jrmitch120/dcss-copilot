//! The message log, merged from webtiles `msgs` messages.

use std::collections::VecDeque;

use copilot_core::{GameMessage, RECENT_MESSAGES};
use serde_json::Value;

use crate::json::int;
use crate::markup::plain;

/// The latest messages the game has sent.
#[derive(Debug, Default)]
pub struct MessageLog {
    recent: VecDeque<GameMessage>,
}

impl MessageLog {
    /// Merges a `msgs` message: it first takes back the last `rollback` lines, which crawl
    /// replaces when it rewrites a message, then adds its new ones, which it returns.
    pub fn merge(&mut self, msg: &Value) -> Vec<GameMessage> {
        let rollback = int::<usize>(msg, "rollback").unwrap_or(0);
        self.recent
            .truncate(self.recent.len().saturating_sub(rollback));
        let mut added = Vec::new();
        for message in msg
            .get("messages")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(text) = message.get("text").and_then(Value::as_str) else {
                continue;
            };
            let message = GameMessage {
                text: plain(text),
                turn: int(message, "turn"),
            };
            self.recent.push_back(message.clone());
            added.push(message);
            if self.recent.len() > RECENT_MESSAGES {
                self.recent.pop_front();
            }
        }
        added
    }

    /// The recent messages, oldest first.
    pub fn recent(&self) -> Vec<GameMessage> {
        self.recent.iter().cloned().collect()
    }
}

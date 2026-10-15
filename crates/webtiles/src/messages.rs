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
    /// replaces when it rewrites a message, then adds its new ones.
    pub fn merge(&mut self, msg: &Value) {
        let rollback = int::<usize>(msg, "rollback").unwrap_or(0);
        self.recent
            .truncate(self.recent.len().saturating_sub(rollback));
        for message in msg
            .get("messages")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(text) = message.get("text").and_then(Value::as_str) else {
                continue;
            };
            self.recent.push_back(GameMessage {
                text: plain(text),
                turn: int(message, "turn"),
            });
            if self.recent.len() > RECENT_MESSAGES {
                self.recent.pop_front();
            }
        }
    }

    /// The recent messages, oldest first.
    pub fn recent(&self) -> Vec<GameMessage> {
        self.recent.iter().cloned().collect()
    }
}

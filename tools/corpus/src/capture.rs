//! What the extension would capture from a webtiles page: each frame the server sends, split into
//! the messages the page hands `comm.handle_message`, each wrapped in an envelope.

use copilot_webtiles::Envelope;
use serde_json::Value;

/// Messages the page handles as they arrive, before `comm.handle_message`, so the extension never
/// sees them: `client.js` registers these as immediate handlers.
const HANDLED_BEFORE_COMM: [&str; 2] = ["ping", "close"];

/// One tab of a webtiles page, as the extension sees it.
#[derive(Debug)]
pub struct Capture {
    origin: String,
    tab: i64,
    /// The text of the latest `version` message.
    game_version: Option<String>,
}

/// What one frame from the server held.
#[derive(Debug, Default, PartialEq)]
pub struct Captured {
    /// Each message the page hands `comm.handle_message`, enveloped, in order.
    pub envelopes: Vec<Envelope>,
    /// The page's immediate handlers took a `ping`, which the page answers with a `pong`.
    pub pinged: bool,
}

impl Capture {
    /// A page at `origin` in browser tab `tab` that has seen nothing yet.
    pub fn new(origin: &str, tab: i64) -> Self {
        Self {
            origin: origin.to_owned(),
            tab,
            game_version: None,
        }
    }

    /// Takes one text frame from the server, received at `ts` in Unix milliseconds.
    ///
    /// A frame is a JSON message, or `{"msgs": [...]}` holding several; a `multi` message's
    /// children are handled one by one in its place. Frames that aren't JSON are JavaScript the
    /// page evaluates, which never reaches `comm.handle_message`.
    pub fn frame(&mut self, text: &str, ts: u64) -> Captured {
        let mut captured = Captured::default();
        if !text.starts_with('{') {
            return captured;
        }
        let Ok(frame) = serde_json::from_str::<Value>(text) else {
            return captured;
        };
        let msgs = match frame.get("msgs") {
            Some(Value::Array(msgs)) => msgs.clone(),
            _ => vec![frame],
        };
        for msg in msgs {
            match msg_type(&msg) {
                Some("ping") => captured.pinged = true,
                Some(other) if HANDLED_BEFORE_COMM.contains(&other) => {}
                _ => self.handle(msg, ts, &mut captured.envelopes),
            }
        }
        captured
    }

    /// What `comm.handle_message` does with `msg`, as the extension's wrapper sees it.
    fn handle(&mut self, msg: Value, ts: u64, envelopes: &mut Vec<Envelope>) {
        if msg_type(&msg) == Some("multi") {
            // The extension skips `multi` itself; its children pass through on their own.
            if let Some(Value::Array(children)) = msg.get("msgs") {
                for child in children.clone() {
                    self.handle(child, ts, envelopes);
                }
            }
            return;
        }
        if msg_type(&msg) == Some("version") {
            self.game_version = Some(match msg.get("text") {
                Some(Value::String(text)) => text.clone(),
                Some(other) => other.to_string(),
                None => String::new(),
            });
        }
        envelopes.push(Envelope {
            origin: self.origin.clone(),
            tab: self.tab,
            ts,
            game_version: self.game_version.clone(),
            msg,
        });
    }
}

/// A webtiles message's type, its `msg` field.
pub fn msg_type(msg: &Value) -> Option<&str> {
    msg.get("msg").and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn types(captured: &Captured) -> Vec<String> {
        captured
            .envelopes
            .iter()
            .map(|envelope| envelope.msg["msg"].as_str().unwrap_or("?").to_owned())
            .collect()
    }

    #[test]
    fn a_batch_is_split_into_its_messages_and_a_multi_into_its_children_in_order() {
        let mut capture = Capture::new("http://localhost:8080", 1);

        let captured = capture.frame(
            r#"{"msgs":[{"msg":"lobby_clear"},{"msg":"multi","msgs":[{"msg":"map"},{"msg":"player"}]},{"msg":"input_mode"}]}"#,
            5,
        );

        assert_eq!(
            types(&captured),
            ["lobby_clear", "map", "player", "input_mode"]
        );
        assert!(captured.envelopes.iter().all(|envelope| envelope.ts == 5
            && envelope.tab == 1
            && envelope.origin == "http://localhost:8080"));
    }

    #[test]
    fn pings_and_closes_never_reach_the_extension_and_a_ping_is_answered() {
        let mut capture = Capture::new("http://localhost:8080", 1);

        let captured = capture.frame(r#"{"msgs":[{"msg":"ping"},{"msg":"close"}]}"#, 5);

        assert_eq!(
            captured,
            Captured {
                envelopes: Vec::new(),
                pinged: true
            }
        );
    }

    #[test]
    fn every_message_from_a_version_message_on_carries_its_text() {
        let mut capture = Capture::new("http://localhost:8080", 1);

        let before = capture.frame(r#"{"msg":"game_started"}"#, 1);
        let at = capture.frame(
            r#"{"msg":"version","text":"Dungeon Crawl Stone Soup 0.34.1"}"#,
            2,
        );
        let after = capture.frame(r#"{"msg":"player"}"#, 3);

        assert_eq!(before.envelopes[0].game_version, None);
        let version = Some("Dungeon Crawl Stone Soup 0.34.1".to_owned());
        assert_eq!(at.envelopes[0].game_version, version);
        assert_eq!(after.envelopes[0].game_version, version);
    }

    #[test]
    fn javascript_frames_are_not_messages() {
        let mut capture = Capture::new("http://localhost:8080", 1);

        assert_eq!(capture.frame("window.foo = 1;", 1), Captured::default());
    }
}

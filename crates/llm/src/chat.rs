//! The chat shape the OpenAI-compatible and Ollama adapters share: a system message, then the
//! conversation with each tool result a message of its own.

use serde_json::{Value, json};

use crate::request::{LlmRequest, Message, Part, Role, ToolDef};

/// The request's messages. An adapter says how a tool result and a turn of the model's look.
pub(crate) fn wire_messages(
    request: &LlmRequest,
    tool_result: impl Fn(&str, &str) -> Value,
    model_turn: impl Fn(&Message) -> Value,
) -> Vec<Value> {
    let mut messages = Vec::new();
    if !request.system.is_empty() {
        let system: Vec<&str> = request
            .system
            .iter()
            .map(|block| block.text.as_str())
            .collect();
        messages.push(json!({ "role": "system", "content": system.join("\n\n") }));
    }
    for turn in &request.messages {
        match turn.role {
            Role::Assistant => messages.push(model_turn(turn)),
            Role::User => {
                // Tool results come first: they must directly follow the turn that called the tools.
                for part in &turn.content {
                    if let Part::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } = part
                    {
                        messages.push(tool_result(tool_use_id, content));
                    }
                }
                let text = turn_text(turn);
                if !text.is_empty() {
                    messages.push(json!({ "role": "user", "content": text }));
                }
            }
        }
    }
    messages
}

/// The text of a turn, with its pieces set apart.
pub(crate) fn turn_text(turn: &Message) -> String {
    let pieces: Vec<&str> = turn
        .content
        .iter()
        .filter_map(|part| match part {
            Part::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    pieces.join("\n\n")
}

/// An engine tool, in the shape both adapters' APIs take.
pub(crate) fn wire_tool(tool: &ToolDef) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.input_schema,
        },
    })
}

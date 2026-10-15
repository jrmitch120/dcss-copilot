//! Reading fields of webtiles messages.

use serde_json::Value;

/// A string field.
pub fn string(msg: &Value, field: &str) -> Option<String> {
    msg.get(field).and_then(Value::as_str).map(str::to_owned)
}

/// An integer field that fits `T`.
pub fn int<T: TryFrom<i64>>(msg: &Value, field: &str) -> Option<T> {
    msg.get(field)
        .and_then(Value::as_i64)
        .and_then(|n| T::try_from(n).ok())
}

/// A webtiles boolean, which crawl sends as `0` or `1` and the server as `true` or `false`.
pub fn flag(value: &Value) -> bool {
    value.as_i64().is_some_and(|v| v != 0) || value.as_bool() == Some(true)
}

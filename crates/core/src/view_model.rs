use serde::Serialize;
use specta::Type;

/// Everything the panel renders, pushed by the core to the webview.
///
/// The webview holds no game logic: it only draws this. TypeScript types are
/// generated from it, so the two sides can't drift apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ViewModel {
    /// The running app's version, shown in the panel footer and About.
    pub app_version: String,
}

impl ViewModel {
    /// The view before any browser has connected.
    #[must_use]
    pub fn initial(app_version: &str) -> Self {
        Self {
            app_version: app_version.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_camel_case_fields_for_the_webview() {
        let json = serde_json::to_value(ViewModel::initial("1.2.3")).unwrap();
        assert_eq!(json, serde_json::json!({ "appVersion": "1.2.3" }));
    }
}

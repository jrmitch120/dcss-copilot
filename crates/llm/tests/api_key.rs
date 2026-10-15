//! A key shows only as its masked form, however it is formatted.

use copilot_llm::{AnthropicProvider, ApiKey};

const KEY: &str = "sk-ant-api03-test-secret-0123456789-a1b2";

#[test]
fn a_key_is_shown_masked_by_its_start_and_its_last_four_characters() {
    let key = ApiKey::new(&format!("  {KEY}\n"));

    assert_eq!(key.masked(), "sk-ant-…a1b2");
    assert_eq!(key.expose(), KEY);
}

#[test]
fn formatting_a_key_or_a_provider_holding_one_never_shows_the_key() {
    let key = ApiKey::new(KEY);
    let provider = AnthropicProvider::new(key.clone());

    let formatted = format!("{key} {key:?} {provider:?}");

    assert!(!formatted.contains(KEY), "{formatted}");
    assert!(formatted.contains("sk-ant-…a1b2"));
}

#[test]
fn a_key_too_short_to_mask_safely_shows_none_of_itself() {
    assert_eq!(ApiKey::new("sk-short").masked(), "…");
}

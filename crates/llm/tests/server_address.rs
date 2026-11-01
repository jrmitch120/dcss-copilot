//! A local server's address: checked wherever it comes from, typed or saved.

// Helpers outside #[test] functions panic on a broken setup too.
#![allow(clippy::unwrap_used)]

use copilot_llm::{Provider, ServerAddress};
use serde_json::json;

#[test]
fn an_address_loses_the_whitespace_and_slash_a_paste_brings_along() {
    let address = ServerAddress::new(" http://localhost:11434/ \n").unwrap();

    assert_eq!(address.as_str(), "http://localhost:11434");
}

#[test]
fn an_address_the_app_cant_call_is_refused() {
    for address in ["localhost:11434", "", "ftp://localhost", "http://"] {
        assert!(ServerAddress::new(address).is_err(), "{address}");
    }
}

#[test]
fn a_saved_address_round_trips_and_a_bad_one_doesnt_load() {
    let address = ServerAddress::new("https://llm.example.org").unwrap();
    let saved = serde_json::to_value(&address).unwrap();

    assert_eq!(saved, json!("https://llm.example.org"));
    assert_eq!(
        serde_json::from_value::<ServerAddress>(saved).unwrap(),
        address
    );
    assert!(serde_json::from_value::<ServerAddress>(json!("localhost:11434")).is_err());
}

#[test]
fn every_local_servers_default_is_an_address_the_app_can_call() {
    for provider in Provider::ALL {
        assert_eq!(provider.default_address().is_some(), provider.is_local());
        if let Some(address) = provider.default_address() {
            assert_eq!(ServerAddress::new(address.as_str()), Ok(address));
        }
    }
}

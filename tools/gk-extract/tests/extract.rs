//! Seam 3: crawl source at a release tag goes in; a Game Knowledge bundle comes out.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use copilot_gk::Bundle;
use gk_extract::{CrawlRelease, extract};

/// The crawl release the fixture and the shipped bundle come from.
fn release() -> CrawlRelease {
    CrawlRelease {
        tag: "0.34.1".to_owned(),
        commit: "381d86c47a17a7f29770cf5e6f7bd9b60642aab6".to_owned(),
    }
}

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Crawl's files at 0.34.1, trimmed to a few entries each so the snapshot can be read whole.
fn fixture() -> PathBuf {
    manifest_dir().join("tests/crawl-0.34.1")
}

#[test]
fn a_bundle_holds_the_monsters_items_spells_species_features_and_monster_states_of_its_crawl_tag() {
    let bundle = extract(&fixture(), &release()).unwrap();

    insta::assert_json_snapshot!(bundle);
}

#[test]
#[ignore = "needs crawl's source at the shipped bundle's tag: set DCSS_CRAWL_SOURCE to a checkout"]
fn the_shipped_bundle_is_what_the_extractor_makes_of_its_crawl_tag() {
    let crawl = std::env::var_os("DCSS_CRAWL_SOURCE").expect("DCSS_CRAWL_SOURCE is not set");
    let shipped_path = manifest_dir().join("../../crates/gk/bundles/0.34.1.json");
    let shipped: Bundle =
        serde_json::from_str(&std::fs::read_to_string(shipped_path).unwrap()).unwrap();

    let extracted = extract(Path::new(&crawl), &release()).unwrap();

    assert_eq!(shipped.crawl, extracted.crawl);
    assert!(
        shipped == extracted,
        "the shipped bundle is stale: regenerate it with gk-extract (see docs/game-knowledge.md)"
    );
}

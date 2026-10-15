//! Seam 1 for scenarios: each manifest and its Recording go in; what the Advisor made of the
//! scenario at the manifest's turns comes out.

// Helpers outside #[test] functions panic on bad fixtures too.
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use copilot_core::CrawlVersion;
use copilot_replay::{SCENARIO_EXTENSION, Scenario, ScenarioError};

fn scenarios_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios")
}

/// A manifest beside the committed scenarios, so its `recording` resolves among them.
fn manifest(text: &str) -> Scenario {
    Scenario::parse(text, &scenarios_dir().join("test.toml")).unwrap()
}

#[test]
fn every_scenario_plays_to_its_snapshots() {
    let mut manifests: Vec<_> = std::fs::read_dir(scenarios_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|ext| ext == SCENARIO_EXTENSION)
        })
        .collect();
    manifests.sort();
    assert!(
        !manifests.is_empty(),
        "no scenarios in {}",
        scenarios_dir().display()
    );

    for path in manifests {
        let scenario = Scenario::load(&path).unwrap();
        let replay = scenario
            .play()
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        insta::assert_json_snapshot!(format!("scenario-{name}"), replay);
    }
}

#[test]
fn a_scenario_whose_game_reports_another_crawl_version_than_its_tag_is_refused() {
    let scenario = manifest(
        r#"
        description = "A kobold on D:3, tagged with the wrong version"
        recording = "../recordings/kobold-on-d3.dcssrec"
        crawl_version = "0.33.1"
        [setup]
        rc = ""
        keys = []
        "#,
    );

    let error = scenario.play().unwrap_err();

    assert!(
        matches!(
            &error,
            ScenarioError::CrawlVersionMismatch(mismatch)
                if mismatch.tagged.0 == "0.33.1" && mismatch.reported == [CrawlVersion("0.34.1".to_owned())]
        ),
        "{error}"
    );
}

#[test]
fn a_scenario_asking_for_a_turn_its_game_never_reports_is_refused() {
    let scenario = manifest(
        r#"
        description = "A kobold on D:3"
        recording = "../recordings/kobold-on-d3.dcssrec"
        crawl_version = "0.34.1"
        snapshot_turns = [100000]
        [setup]
        rc = ""
        keys = []
        "#,
    );

    let error = scenario.play().unwrap_err();

    assert!(
        matches!(error, ScenarioError::TurnNeverReached(100_000)),
        "{error}"
    );
}

#[test]
fn a_scenario_cannot_expect_advice_the_advisor_does_not_raise_yet() {
    let scenario = manifest(
        r#"
        description = "A kobold on D:3"
        recording = "../recordings/kobold-on-d3.dcssrec"
        crawl_version = "0.34.1"
        [setup]
        rc = ""
        keys = []
        [[alerts]]
        turn = 5
        cause = "threat-level-rose"
        "#,
    );

    let error = scenario.play().unwrap_err();

    assert!(
        matches!(error, ScenarioError::AdviceNotYetRaised),
        "{error}"
    );
}

#[test]
fn a_manifest_with_a_misspelt_field_is_refused() {
    let error = Scenario::parse(
        r#"
        description = "A kobold on D:3"
        recording = "kobold.dcssrec"
        crawl_version = "0.34.1"
        snapshot_turn = [5]
        [setup]
        rc = ""
        keys = []
        "#,
        Path::new("scenario.toml"),
    )
    .unwrap_err();

    assert!(error.to_string().contains("snapshot_turn"), "{error}");
}

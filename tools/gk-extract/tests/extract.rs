//! Seam 3: crawl source at a release tag goes in; a Game Knowledge bundle comes out.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

use copilot_gk::Bundle;
use gk_extract::{CrawlRelease, ExtractError, extract};

/// The crawl release the fixture and the shipped bundle come from.
fn release() -> CrawlRelease {
    CrawlRelease {
        tag: "0.34.1".to_owned(),
        commit: "1eebc1a2892e1c89776a0d7a10691f8dac8d9796".to_owned(),
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
    let bundle = extract(&fixture(), &release()).unwrap().bundle;

    insta::assert_json_snapshot!(bundle);
}

/// Every file under `dir`, by its path from `dir`, with its bytes.
fn files_under(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut dirs = vec![dir.to_owned()];
    while let Some(next) = dirs.pop() {
        for entry in std::fs::read_dir(next).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                let name = path.strip_prefix(dir).unwrap().to_str().unwrap();
                files.push((name.replace('\\', "/"), std::fs::read(&path).unwrap()));
            }
        }
    }
    files.sort();
    files
}

/// A PNG's size and its pixels as 8-bit RGBA, read the way crawl's tile build reads an image
/// (`tile::load` in `rltiles/tool/tile.cc`).
fn rgba(png_bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    decoder.set_transformations(
        png::Transformations::normalize_to_color8() | png::Transformations::ALPHA,
    );
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut pixels).unwrap();
    pixels.truncate(frame.buffer_size());
    let pixels = match frame.color_type {
        png::ColorType::Rgba => pixels,
        png::ColorType::GrayscaleAlpha => pixels
            .chunks(2)
            .flat_map(|ga| [ga[0], ga[0], ga[0], ga[1]])
            .collect(),
        other => panic!("{other:?}"),
    };
    (frame.width, frame.height, pixels)
}

/// Tiles the fixture's tile list outlines (`%rim 1`), as crawl's own tile build draws them: made
/// by `tile::load`, `tile::add_rim(tile_colour::black)` and `write_png` from crawl 0.34.1's
/// `rltiles/tool`, compiled on their own, run on the fixture's files.
fn outlined_by_crawl() -> PathBuf {
    manifest_dir().join("tests/outlined-by-crawl")
}

#[test]
fn the_tiles_a_bundle_names_are_written_as_crawls_tile_build_draws_them() {
    let extraction = extract(&fixture(), &release()).unwrap();
    let out = tempfile::tempdir().unwrap();

    let written = extraction.tiles.write(&fixture(), out.path()).unwrap();

    let files = files_under(out.path());
    let names: Vec<&str> = files.iter().map(|(name, _)| name.as_str()).collect();
    // The rat's tile is one crawl's tile project doesn't release under CC0; it ships under crawl's GPL.
    assert_eq!(
        names,
        [
            "mon/animals/adder.png",
            "mon/animals/bat.png",
            "mon/animals/jackal.png",
            "mon/animals/quokka.png",
            "mon/animals/rat.png",
            "mon/animals/snapping_turtle.png",
            "mon/demihumanoids/taurs/centaur.png",
            "mon/fungi_plants/fungus1.png",
            "mon/humanoids/goblin_rider.png",
            "mon/humanoids/kobold.png",
            "mon/humanoids/orcs/orc_wizard.png",
            "mon/statues/test_statue.png",
            "mon/unique/maurice.png",
        ]
    );
    assert_eq!(written, names.len());
    let rltiles = fixture().join("crawl-ref/source/rltiles");
    let outlined: Vec<String> = files_under(&outlined_by_crawl())
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(outlined, ["mon/animals/bat.png", "mon/animals/quokka.png"]);
    for (name, bytes) in &files {
        if outlined.contains(name) {
            let crawls = std::fs::read(outlined_by_crawl().join(name)).unwrap();
            assert!(
                rgba(bytes) == rgba(&crawls),
                "{name} isn't outlined as crawl outlines it"
            );
        } else {
            assert_eq!(bytes, &std::fs::read(rltiles.join(name)).unwrap(), "{name}");
        }
    }
}

#[test]
fn a_monster_drawn_with_crawls_error_tile_has_no_tile() {
    let extraction = extract(&fixture(), &release()).unwrap();

    // Crawl composes and recolours a black draconian at runtime; its data names only the
    // program bug's error tile, which isn't a picture of it.
    let draconian = extraction
        .bundle
        .monsters
        .values()
        .find(|monster| monster.id.0 == "MONS_BLACK_DRACONIAN")
        .unwrap();
    assert_eq!(draconian.tile, None);
}

/// A copy of the fixture with `edit` made to each file: given its path under the fixture and its
/// bytes, it returns the bytes to write.
fn fixture_with(edit: impl Fn(&str, Vec<u8>) -> Vec<u8>) -> tempfile::TempDir {
    let crawl = tempfile::tempdir().unwrap();
    for (name, bytes) in files_under(&fixture()) {
        let path = crawl.path().join(&name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, edit(&name, bytes)).unwrap();
    }
    crawl
}

#[test]
fn a_status_light_crawl_no_longer_shows_fails_the_extraction() {
    // A copy of the fixture whose contamination light is renamed.
    let crawl = fixture_with(|name, bytes| {
        if name == "crawl-ref/source/status.cc" {
            String::from_utf8(bytes)
                .unwrap()
                .replace("\"Contam\"", "\"Glow\"")
                .into_bytes()
        } else {
            bytes
        }
    });

    let refused = extract(crawl.path(), &release());

    assert!(
        matches!(&refused, Err(ExtractError::Missing { file, what }) if file == "status.cc" && what == "\"Contam\""),
        "{refused:?}"
    );
}

#[test]
fn a_crawl_release_without_known_formula_variants_is_refused_rather_than_read_as_0_34() {
    for tag in ["0.35.0", "0.33.1", "0.35-a0"] {
        let release = CrawlRelease {
            tag: tag.to_owned(),
            ..release()
        };

        let refused = extract(&fixture(), &release);

        assert!(
            matches!(&refused, Err(ExtractError::UnknownSeries { tag: refused }) if refused == tag),
            "{tag}: {refused:?}"
        );
    }
}

/// Runs git in `dir`, returning what it printed.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=test", "-c", "user.email=test@example.org"])
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

/// A git checkout of two commits, the first tagged `0.34.1`, at the second unless `at_tag`.
fn checkout(at_tag: bool) -> (tempfile::TempDir, String, String) {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("README"), "crawl").unwrap();
    git(dir.path(), &["add", "README"]);
    git(dir.path(), &["commit", "-q", "-m", "release"]);
    git(dir.path(), &["tag", "0.34.1"]);
    let tagged = git(dir.path(), &["rev-parse", "HEAD"]);
    git(
        dir.path(),
        &["commit", "-q", "--allow-empty", "-m", "later"],
    );
    let later = git(dir.path(), &["rev-parse", "HEAD"]);
    if at_tag {
        git(dir.path(), &["checkout", "-q", "0.34.1"]);
    }
    (dir, tagged, later)
}

#[test]
fn the_release_is_the_tag_and_commit_checked_out() {
    let (crawl, tagged, _) = checkout(true);

    let release = CrawlRelease::checked_out(crawl.path(), "0.34.1", &tagged).unwrap();

    assert_eq!(
        release,
        CrawlRelease {
            tag: "0.34.1".to_owned(),
            commit: tagged
        }
    );
}

#[test]
fn a_tag_or_commit_that_isnt_what_is_checked_out_is_refused() {
    let (at_tag, tagged, later) = checkout(true);
    let (after_tag, _, _) = checkout(false);

    for (crawl, tag, commit) in [
        (&at_tag, "0.34.0", tagged.as_str()),
        (&at_tag, "0.34.1", later.as_str()),
        (&after_tag, "0.34.1", tagged.as_str()),
    ] {
        let refused = CrawlRelease::checked_out(crawl.path(), tag, commit);

        assert!(
            matches!(refused, Err(ExtractError::NotCheckedOut { .. })),
            "{tag} at {commit}: {refused:?}"
        );
    }
}

#[test]
fn a_checkout_with_changes_is_refused() {
    let (crawl, tagged, _) = checkout(true);
    std::fs::write(crawl.path().join("README"), "changed").unwrap();

    let refused = CrawlRelease::checked_out(crawl.path(), "0.34.1", &tagged);

    assert!(
        matches!(refused, Err(ExtractError::Changed { .. })),
        "{refused:?}"
    );
}

#[test]
#[ignore = "needs crawl's source at the shipped bundle's tag: set DCSS_CRAWL_SOURCE to a checkout"]
fn the_shipped_bundle_is_what_the_extractor_makes_of_its_crawl_tag() {
    let crawl = std::env::var_os("DCSS_CRAWL_SOURCE").expect("DCSS_CRAWL_SOURCE is not set");
    let shipped_path = manifest_dir().join("../../crates/gk/bundles/0.34.1.json");
    let shipped: Bundle =
        serde_json::from_str(&std::fs::read_to_string(shipped_path).unwrap()).unwrap();

    let expected = release();
    let release = CrawlRelease::checked_out(Path::new(&crawl), &expected.tag, &expected.commit)
        .expect("DCSS_CRAWL_SOURCE is crawl checked out at the shipped bundle's tag");
    let extraction = extract(Path::new(&crawl), &release).unwrap();

    assert_eq!(shipped.crawl, extraction.bundle.crawl);
    assert!(
        shipped == extraction.bundle,
        "the shipped bundle is stale: regenerate it with gk-extract (see docs/game-knowledge.md)"
    );
    let tiles = tempfile::tempdir().unwrap();
    extraction
        .tiles
        .write(Path::new(&crawl), tiles.path())
        .unwrap();
    assert!(
        files_under(&manifest_dir().join("../../app/static/tiles")) == files_under(tiles.path()),
        "the shipped tiles are stale: write them again with gk-extract (see docs/game-knowledge.md)"
    );
}

//! The crawl identifiers the webtiles source compares Game Knowledge against, grouped by what
//! they name. A test checks each against every shipped Game Knowledge bundle.

use copilot_gk::crawl_identifiers;

crawl_identifiers! {
    /// `mon_attitude_type` identifiers: a monster's webtiles `att`.
    attitudes {
        HOSTILE = "ATT_HOSTILE";
        NEUTRAL = "ATT_NEUTRAL";
        GOOD_NEUTRAL = "ATT_GOOD_NEUTRAL";
        /// Strict neutral, once Jiyva's, which crawl has since merged into good neutral
        /// (`TAG_MINOR_CUT_STRICT_NEUTRAL`); the value stays in its enum.
        OLD_STRICT_NEUTRAL = "ATT_OLD_STRICT_NEUTRAL";
        FRIENDLY = "ATT_FRIENDLY";
        MARIONETTE = "ATT_MARIONETTE";
    }
}

crawl_identifiers! {
    /// `mon_threat_level_type` identifiers: a monster's webtiles `threat`.
    monster_threats {
        TRIVIAL = "MTHRT_TRIVIAL";
        EASY = "MTHRT_EASY";
        TOUGH = "MTHRT_TOUGH";
        NASTY = "MTHRT_NASTY";
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use copilot_gk::Library;

    use super::*;

    #[test]
    fn every_identifier_the_webtiles_source_names_is_in_every_shipped_bundle() {
        let library = Library::shipped().unwrap();
        let mut bundles = 0;
        for bundle in library.bundles() {
            bundles += 1;
            for (kind, named, known) in [
                ("attitude", attitudes::ALL, &bundle.attitudes),
                (
                    "monster threat",
                    monster_threats::ALL,
                    &bundle.monster_threats,
                ),
            ] {
                let known: BTreeSet<&str> = known.values().map(|id| id.0.as_str()).collect();
                for id in named {
                    assert!(
                        known.contains(id),
                        "{} has no {kind} identifier {id}",
                        bundle.crawl.tag
                    );
                }
            }
        }
        assert!(bundles > 0, "no shipped bundles");
    }
}

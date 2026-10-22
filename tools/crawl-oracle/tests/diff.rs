//! The crawl oracle's diff, run on what its harnesses printed from a crawl 0.34.1 build: 24
//! randomised monster attackers, every other one holding a melee weapon, and player defenders of
//! random species, gods and resistances, 20,000 attacks each, 24 spell cases, 60 missile, gear and wand cases, and the turn harness's
//! block and reach cases with 126 turn cases and its torment cases; and 24 cases of the player
//! attacking monsters in melee, with launchers and thrown missiles and with spells' beams, and of
//! the hit points monsters are made with (all seed 34).

// Helpers outside #[test] functions panic on a broken fixture too.
#![allow(clippy::unwrap_used)]

use crawl_oracle::{Measure, diff};

const DUMP: &str = include_str!("fixtures/melee-0.34.1.jsonl");

#[test]
fn the_engine_agrees_with_crawl_on_every_case() {
    let report = diff(DUMP).unwrap();
    assert_eq!(report.cases, 24);
    assert!(report.mismatches.is_empty(), "{report}");
}

/// The dump with every to-hit roll in `case` one higher, as an off-by-one port would roll it.
fn with_to_hit_one_higher(case: usize) -> String {
    DUMP.lines()
        .enumerate()
        .map(|(line, json)| {
            let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
            if line == case + 1 {
                let shifted: serde_json::Map<_, _> = value["to_hit"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(roll, count)| {
                        (
                            (roll.parse::<i32>().unwrap() + 1).to_string(),
                            count.clone(),
                        )
                    })
                    .collect();
                value["to_hit"] = shifted.into();
            }
            format!("{value}\n")
        })
        .collect()
}

#[test]
fn a_to_hit_off_by_one_is_caught() {
    // Case 0, a battlemage, rolls to-hit over 0..=24: one in 25 rolls lands one higher.
    assert_eq!(caught(&with_to_hit_one_higher(0)), [(0, Measure::ToHit)]);
}

/// A share of `dump`'s cases, every `count`th from `index`, with a header naming shard `index` of
/// `count`, as a harness run with a shard prints it.
fn shard_of(dump: &str, index: u64, count: u64) -> String {
    dump.lines()
        .enumerate()
        .filter_map(|(line, json)| {
            let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
            if line == 0 {
                value["shard"] = serde_json::json!({ "index": index, "count": count });
            } else if value["case"].as_u64().unwrap() % count != index {
                return None;
            }
            Some(format!("{value}\n"))
        })
        .collect()
}

#[test]
fn one_shard_of_a_run_is_diffed_on_its_own() {
    let report = diff(&shard_of(DUMP, 1, 3)).unwrap();
    assert_eq!(report.cases, 8);
    assert!(report.mismatches.is_empty(), "{report}");
    assert!(
        report
            .to_string()
            .starts_with("crawl 0.34.1, shard 1/3: 8 cases"),
        "{report}"
    );
}

#[test]
fn a_cherubs_halo_left_out_of_its_aim_is_caught() {
    // Case 11, a cherub, whose halo backlights the player: counted as plain light.
    let dump = with_changed(DUMP, 11, |case| case["lighting"] = "plain".into());
    assert!(caught(&dump).contains(&(11, Measure::ToHit)));
}

#[test]
fn an_umbra_left_out_of_the_aim_of_a_monster_without_nightvision_is_caught() {
    // Case 2, a yaktaur, which can't see through the player's umbra: counted as plain light.
    let dump = with_changed(DUMP, 2, |case| case["lighting"] = "plain".into());
    assert!(caught(&dump).contains(&(2, Measure::ToHit)));
}

#[test]
fn an_umbra_doesnt_spoil_the_aim_of_a_monster_with_nightvision() {
    // Case 21, a vampire, undead: the player's umbra doesn't change its rolls.
    let dump = with_changed(DUMP, 21, |case| case["lighting"] = "plain".into());
    assert_eq!(caught(&dump), []);
}

#[test]
fn nightvision_the_game_knowledge_doesnt_give_a_monster_is_caught() {
    // Case 21, a vampire, which crawl says sees through an umbra: said not to.
    let dump = with_changed(DUMP, 21, |case| case["nightvision"] = false.into());
    assert_eq!(caught(&dump), [(21, Measure::Nightvision)]);
}

#[test]
fn a_wielded_weapons_enchantment_left_out_is_caught() {
    // Case 9, a spriggan's +3 demon blade: played as +0.
    let dump = with_changed(DUMP, 9, |case| case["main"]["plus"] = 0.into());
    assert_eq!(
        caught(&dump),
        [
            (9, Measure::ToHit),
            (9, Measure::Hits),
            (9, Measure::Damage)
        ]
    );
}

#[test]
fn a_wielded_weapons_brand_left_out_is_caught() {
    // Case 13, a pale draconian's +3 bardiche of pain, at a human with rN+: played unbranded.
    let dump = with_changed(DUMP, 13, |case| {
        case["main"]["brand"] = "SPWPN_NORMAL".into();
    });
    assert_eq!(caught(&dump), [(13, Measure::Damage)]);
}

#[test]
fn a_players_holiness_left_out_is_caught() {
    // Case 3, a minotaur's +4 halberd of draining, which doesn't drain a mummy: the player taken
    // for a human.
    let dump = with_changed(DUMP, 3, |case| case["resists"]["species"] = "Human".into());
    assert_eq!(caught(&dump), [(3, Measure::Damage)]);
}

#[test]
fn a_heavy_weapon_swung_as_quickly_as_any_is_caught() {
    // Case 21, a vampire with a +2 heavy war axe, biting: the bite isn't made with it, but its
    // melee action takes half as long again. Played unbranded.
    let dump = with_changed(DUMP, 21, |case| {
        case["main"]["brand"] = "SPWPN_NORMAL".into();
    });
    assert_eq!(caught(&dump), [(21, Measure::Delay)]);
}

#[test]
fn a_weapon_held_by_a_monster_crawl_shows_wielding_nothing_is_caught() {
    // Case 10, a djinni hitting unarmed: given a +9 spear.
    let dump = with_changed(DUMP, 10, |case| {
        case["main"] = serde_json::json!({ "weapon": "0:23", "plus": 9, "brand": "SPWPN_NORMAL" });
    });
    assert!(caught(&dump).contains(&(10, Measure::ToHit)));
}

#[test]
fn a_dump_for_a_crawl_without_a_bundle_is_refused() {
    let dump = DUMP.replacen("\"0.34.1\"", "\"0.12.0\"", 1);
    let error = diff(&dump).unwrap_err().to_string();
    assert!(error.contains("0.12.0"), "{error}");
}

/// What the spell harness printed from the same crawl build: 24 cases, alternating a monster
/// casting one spell at a randomised player and a monster choosing among its spells, 20,000
/// times each (seed 34).
const SPELLS: &str = include_str!("fixtures/spells-0.34.1.jsonl");

#[test]
fn the_engine_agrees_with_crawl_on_every_spell_case() {
    let report = diff(SPELLS).unwrap();
    assert_eq!(report.cases, 24);
    assert!(report.mismatches.is_empty(), "{report}");
}

/// `dump` with case `case` changed by `change`.
fn with_changed(dump: &str, case: usize, change: impl Fn(&mut serde_json::Value)) -> String {
    dump.lines()
        .enumerate()
        .map(|(line, json)| {
            let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
            if line == case + 1 {
                change(&mut value);
            }
            format!("{value}\n")
        })
        .collect()
}

/// A tally with every value one higher.
fn shifted(tally: &serde_json::Value) -> serde_json::Value {
    tally
        .as_object()
        .unwrap()
        .iter()
        .map(|(value, count)| {
            (
                (value.parse::<i32>().unwrap() + 1).to_string(),
                count.clone(),
            )
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}

#[test]
fn a_beam_damage_off_by_one_is_caught() {
    // Case 6, a tengu conjurer's lightning bolt (0 to 14): every hit one point harder.
    let dump = with_changed(SPELLS, 6, |case| case["damage"] = shifted(&case["damage"]));
    assert_eq!(caught(&dump), [(6, Measure::Damage)]);
}

#[test]
fn a_players_resistance_left_out_is_caught() {
    // Case 16, a hell knight's bolt of fire at a mummy with rF+: the player taken to resist none.
    let dump = with_changed(SPELLS, 16, |case| case["resists"]["fire"] = 0.into());
    assert_eq!(caught(&dump), [(16, Measure::Damage)]);
}

#[test]
fn a_backlit_player_counted_as_in_plain_light_is_caught() {
    // Case 10, an electric golem's lightning bolt at a backlit player: counted as plain light.
    let dump = with_changed(SPELLS, 10, |case| case["lighting"] = "plain".into());
    assert_eq!(caught(&dump), [(10, Measure::Hits)]);
}

#[test]
fn a_spell_power_off_by_one_is_caught() {
    let dump = with_changed(SPELLS, 2, |case| {
        case["power"] = (case["power"].as_i64().unwrap() + 1).into();
    });
    assert_eq!(caught(&dump), [(2, Measure::Power)]);
}

#[test]
fn a_spell_chosen_as_often_as_another_is_caught() {
    // Case 9, a fire giant six squares away: half its bolts of fire counted as no spell.
    let dump = with_changed(SPELLS, 9, |case| {
        let chosen = case["chosen"].as_object_mut().unwrap();
        let fire = chosen["10"].as_u64().unwrap();
        let none = chosen["-1"].as_u64().unwrap();
        chosen.insert("10".to_owned(), (fire / 2).into());
        chosen.insert("-1".to_owned(), (none + fire - fire / 2).into());
    });
    assert_eq!(caught(&dump), [(9, Measure::SpellChosen)]);
}

/// What the missile harness printed from the same crawl build: 60 cases, taking turns at a kind
/// of monster made with its gear, a monster shooting or throwing at a randomised player, deciding
/// whether to fire, zapping a wand at the player and deciding whether to zap, 20,000 times each
/// (seed 34).
const MISSILES: &str = include_str!("fixtures/missiles-0.34.1.jsonl");

#[test]
fn the_engine_agrees_with_crawl_on_every_missile_gear_and_wand_case() {
    let report = diff(MISSILES).unwrap();
    assert_eq!(report.cases, 60);
    assert!(report.mismatches.is_empty(), "{report}");
}

#[test]
fn a_launchers_enchantment_left_out_is_caught() {
    // Case 36, a naga sharpshooter's +2 shortbow of speed: played as +0.
    let dump = with_changed(MISSILES, 36, |case| case["plus"] = 0.into());
    assert_eq!(caught(&dump), [(36, Measure::ToHit), (36, Measure::Damage)]);
}

#[test]
fn a_launchers_brand_left_out_is_caught() {
    // Case 46, a centaur warrior's -1 shortbow of foul flame: played unbranded.
    let dump = with_changed(MISSILES, 46, |case| case["brand"] = "SPWPN_NORMAL".into());
    assert_eq!(caught(&dump), [(46, Measure::Damage)]);
}

#[test]
fn a_players_good_god_left_out_is_caught() {
    // Case 46, a centaur warrior's foul flame at a worshipper of the Shining One, whom it hurts
    // half as much again: the player taken to worship no god.
    let dump = with_changed(MISSILES, 46, |case| {
        case["resists"]["god"] = "No God".into();
    });
    assert_eq!(caught(&dump), [(46, Measure::Damage)]);
}

#[test]
fn a_heavy_launcher_fired_as_quickly_as_any_is_caught() {
    // Case 47, a faun's -2 heavy sling, which takes half as long again to fire: played unbranded.
    let dump = with_changed(MISSILES, 47, |case| case["brand"] = "SPWPN_NORMAL".into());
    assert_eq!(caught(&dump), [(47, Measure::Energy)]);
}

/// What `diff` caught in `dump`, by case and measure.
fn caught(dump: &str) -> Vec<(u64, Measure)> {
    diff(dump)
        .unwrap()
        .mismatches
        .iter()
        .map(|m| (m.case, m.measure))
        .collect()
}

#[test]
fn a_missile_damage_off_by_one_is_caught() {
    // Case 16, a merfolk javelineer's javelin, which deals up to 37: every hit one point harder.
    let dump = with_changed(MISSILES, 16, |case| {
        case["damage"] = shifted(&case["damage"]);
    });
    assert_eq!(caught(&dump), [(16, Measure::Damage)]);
}

#[test]
fn a_kind_made_with_its_nets_less_often_than_crawl_makes_it_is_caught() {
    // Case 15, Harold, always made with throwing nets: a tenth counted as made with nothing.
    let dump = with_changed(MISSILES, 15, |case| {
        let missile = case["missile"].as_object_mut().unwrap();
        let nets = missile["1:8"].as_u64().unwrap();
        missile.insert("1:8".to_owned(), (nets - nets / 10).into());
        missile.insert("none".to_owned(), (nets / 10).into());
    });
    assert_eq!(caught(&dump), [(15, Measure::MissileMade)]);
}

#[test]
fn a_launcher_fired_as_often_as_a_master_archer_fires_it_is_caught() {
    // Case 2, a merfolk's boomerang, thrown one time in three: counted as two in three.
    let dump = with_changed(MISSILES, 2, |case| {
        let energy = case["energy"].as_object_mut().unwrap();
        let held = energy["0"].as_u64().unwrap();
        let fired = energy["10"].as_u64().unwrap();
        energy.insert("0".to_owned(), (held / 2).into());
        energy.insert("10".to_owned(), (fired + held - held / 2).into());
    });
    assert_eq!(caught(&dump), [(2, Measure::Energy)]);
}

/// What the turn harness printed from the same crawl build: how many attacks each shield blocks,
/// how far every kind of monster with a damaging attack reaches in melee and with which attacks,
/// unarmed and, for a kind that hits, holding a spear, 126 monsters playing out the player's next
/// action 20,000 times each, taking turns at the rules they exercise (seed 34), and the hit
/// points a lurking horror's torment takes from 51 players.
const TURNS: &str = include_str!("fixtures/turns-0.34.1.jsonl");

#[test]
fn the_engine_agrees_with_crawl_on_every_block_reach_turn_and_torment_case() {
    let report = diff(TURNS).unwrap();
    assert_eq!(report.cases, 922);
    assert!(report.mismatches.is_empty(), "{report}");
}

#[test]
fn torment_taking_one_hit_point_more_is_caught() {
    // Case 889, a human in statue form with rNeg 1, whom torment takes 8 from on 40 HP: half
    // of 40 * 45 / 100 - 1.
    let dump = with_changed(TURNS, 889, |case| case["lost"]["40"] = 9.into());
    assert_eq!(caught(&dump), [(889, Measure::Torment)]);
}

#[test]
fn a_lurking_horror_attacking_again_after_blowing_itself_up_is_caught() {
    // Case 840, a lurking horror beside the player at speed 12, which attacks, blowing itself
    // up, when its roll lets it step into them: never twice.
    let dump = with_changed(TURNS, 840, |case| case["melee"] = shifted(&case["melee"]));
    assert_eq!(caught(&dump), [(840, Measure::MeleeActions)]);
}

#[test]
fn a_shield_that_blocks_one_more_attack_is_caught() {
    // Case 1, a buckler, which blocks two attacks before the player's next action.
    let dump = with_changed(TURNS, 1, |case| case["limit"] = 3.into());
    assert_eq!(caught(&dump), [(1, Measure::BlockLimit)]);
}

#[test]
fn a_reaching_attack_held_to_two_squares_is_caught() {
    // Case 666, a starflower, whose lash follows its rift three squares: counted as two.
    let dump = with_changed(TURNS, 666, |case| {
        case["attacks_from"]["3"] = serde_json::json!([0]);
    });
    assert_eq!(caught(&dump), [(666, Measure::AttacksFrom)]);
}

#[test]
fn a_monster_reaching_one_square_too_far_is_caught() {
    // Case 279, an orange demon, whose sting reaches two squares.
    let dump = with_changed(TURNS, 279, |case| case["reach"] = 3.into());
    assert_eq!(caught(&dump), [(279, Measure::Reach)]);
}

#[test]
fn a_polearm_held_to_one_square_is_caught() {
    // Case 139, an orc holding a +0 spear, which strikes from two squares: counted as one.
    let dump = with_changed(TURNS, 139, |case| {
        case["reach"] = 1.into();
        case["attacks_from"].as_object_mut().unwrap().remove("2");
    });
    assert_eq!(
        caught(&dump),
        [(139, Measure::Reach), (139, Measure::AttacksFrom)]
    );
}

#[test]
fn an_off_hands_dagger_reaching_as_far_as_the_main_hands_spear_is_caught() {
    // Case 234, an ettin holding a +0 spear and a +0 dagger: its second attack, with the dagger,
    // counted from two squares.
    let dump = with_changed(TURNS, 234, |case| {
        case["attacks_from"]["2"] = serde_json::json!([0, 1]);
    });
    assert_eq!(caught(&dump), [(234, Measure::AttacksFrom)]);
}

#[test]
fn a_monster_attacking_once_more_than_crawl_lets_it_is_caught() {
    // Case 803, a hellion beside the player at speed 12, which attacks once or twice.
    let dump = with_changed(TURNS, 803, |case| case["melee"] = shifted(&case["melee"]));
    assert_eq!(caught(&dump), [(803, Measure::MeleeActions)]);
}

#[test]
fn a_monster_closing_a_square_more_than_crawl_lets_it_is_caught() {
    // Case 748, a juggernaut four squares away at speed 15, which closes a square, and half the
    // time two.
    let dump = with_changed(TURNS, 748, |case| case["closed"] = shifted(&case["closed"]));
    assert_eq!(caught(&dump), [(748, Measure::Closed)]);
}

#[test]
fn a_blinkbolt_landing_past_the_player_is_caught() {
    // Case 818, a spark wasp four squares away, which its blinkbolt lands beside the player:
    // landed two squares past them, as on open floor, it closes two.
    let dump = with_changed(TURNS, 818, |case| {
        let closed = case["closed"].as_object_mut().unwrap();
        let landed = closed.remove("3").unwrap().as_u64().unwrap();
        let walked = closed["2"].as_u64().unwrap();
        closed.insert("2".to_owned(), (walked + landed).into());
    });
    assert_eq!(caught(&dump), [(818, Measure::Closed)]);
}

#[test]
fn a_monster_the_threat_level_leaves_out_that_crawl_shows_fighting_is_caught() {
    // Case 803, a hellion, counted twenty squares away while crawl shows it fighting.
    let dump = with_changed(TURNS, 803, |case| case["distance"] = 20.into());
    assert!(caught(&dump).contains(&(803, Measure::CanAttack)));
}

#[test]
fn a_watched_monster_walking_up_as_if_unwatched_is_caught() {
    // Case 811, a wandering mushroom four squares away, which crawl keeps where it is while the
    // player watches it: walked a square, as any other monster would be.
    let dump = with_changed(TURNS, 811, |case| {
        case["closed"] = serde_json::json!({ "1": 20_000 });
    });
    assert_eq!(caught(&dump), [(811, Measure::Closed)]);
}

#[test]
fn a_monster_keeping_its_range_walked_in_as_any_other_is_caught() {
    // Case 757, an orb spider four squares away casting from another kind's book, which closes
    // to three and holds there, stepping around the player: closing a square more, as any other
    // monster would.
    let dump = with_changed(TURNS, 757, |case| case["closed"] = shifted(&case["closed"]));
    assert_eq!(caught(&dump), [(757, Measure::Closed)]);
}

#[test]
fn a_monster_keeping_its_range_biting_from_beside_the_player_is_caught() {
    // Case 841, an orb spider three squares away with its own book, cut down to nothing, which
    // closes in to beside the player and holds there, stepping around them rather than bite: the
    // Threat Level counts it as unable to attack at all.
    let dump = with_changed(TURNS, 841, |case| case["melee"] = shifted(&case["melee"]));
    assert_eq!(
        caught(&dump),
        [(841, Measure::CanAttack), (841, Measure::MeleeActions)]
    );
}

#[test]
fn a_borrowed_book_left_out_is_caught() {
    // Case 757's orb spider with its own book, cut down to nothing, closes in to bite rather
    // than hold three squares away, and casts nothing.
    let dump = with_changed(TURNS, 757, |case| case["book"] = case["monster"].clone());
    assert_eq!(
        caught(&dump),
        [(757, Measure::Casts), (757, Measure::Closed)]
    );
}

#[test]
fn a_batty_monster_staying_to_bite_with_every_action_is_caught() {
    // Case 772, an obsidian bat beside the player at speed 30, which bites, then flits for its
    // two other actions, biting again only when a flit carries it into the player.
    let dump = with_changed(TURNS, 772, |case| {
        case["melee"] = serde_json::json!({ "3": 20_000 });
    });
    assert_eq!(caught(&dump), [(772, Measure::MeleeActions)]);
}

#[test]
fn a_batty_monster_casting_as_it_flits_is_caught() {
    // Case 786, a shard shrike beside the player at speed 30, which casts nothing in the actions
    // it flits after biting or casting.
    let dump = with_changed(TURNS, 786, |case| case["casts"] = shifted(&case["casts"]));
    assert_eq!(caught(&dump), [(786, Measure::Casts)]);
}

/// `case`'s casts with `moved` of its plays that cast once casting twice instead.
fn casting_twice(case: &mut serde_json::Value, moved: u64) {
    let casts = case["casts"].as_object_mut().unwrap();
    let once = casts["1"].as_u64().unwrap();
    let twice = casts.get("2").map_or(0, |twice| twice.as_u64().unwrap());
    casts.insert("1".to_owned(), (once - moved).into());
    casts.insert("2".to_owned(), (twice + moved).into());
}

#[test]
fn a_monster_breathing_again_through_its_opaque_clouds_is_caught() {
    // Case 824, a pearl dragon three squares away at speed 12, whose blessed fire hides the
    // player from it till it has walked to two: it never breathes twice. Without its clouds it
    // would breathe twice in about one play of 125, when its breath is caught by its second
    // action; said to in one of 50.
    let dump = with_changed(TURNS, 824, |case| casting_twice(case, 400));
    assert_eq!(caught(&dump), [(824, Measure::Casts)]);
}

#[test]
fn a_cloud_spread_again_to_a_player_already_in_it_is_caught() {
    // Case 796, Gloorx Vloq six squares away at speed 20, past the five its Miasma Breath's beam
    // reaches, whose cloud spreads to the player only while their square holds none: once it
    // does, it casts again only after closing a square. Were its cloud to spread to them again,
    // it would cast twice in about one play of 70 more; said to in one of 20 more.
    let dump = with_changed(TURNS, 796, |case| casting_twice(case, 1_000));
    assert_eq!(caught(&dump), [(796, Measure::Casts)]);
}

const PLAYER: &str = include_str!("fixtures/player-0.34.1.jsonl");

#[test]
fn the_engine_agrees_with_crawl_on_every_player_case() {
    let report = diff(PLAYER).unwrap();
    assert_eq!(report.cases, 24);
    assert!(report.mismatches.is_empty(), "{report}");
}

#[test]
fn a_heavy_weapon_played_as_plain_is_caught() {
    // Case 0 swings a +4 heavy weapon: plain, it hits for five ninths as much and sooner.
    let dump = with_changed(PLAYER, 0, |case| {
        case["weapon"]["brand"] = "SPWPN_NORMAL".into();
    });
    let measures: Vec<Measure> = caught(&dump).into_iter().map(|(_, m)| m).collect();
    assert!(measures.contains(&Measure::Damage), "{measures:?}");
    assert!(measures.contains(&Measure::AttackDelay), "{measures:?}");
}

#[test]
fn a_thrown_missiles_skill_read_too_low_is_caught() {
    // Case 1 throws a javelin at Throwing 21.4: at a third of that, it hits less often.
    let dump = with_changed(PLAYER, 1, |case| {
        let skill = case["skill"].as_i64().unwrap();
        case["skill"] = (skill / 3).into();
    });
    let measures: Vec<Measure> = caught(&dump).into_iter().map(|(_, m)| m).collect();
    assert!(measures.contains(&Measure::ToHit), "{measures:?}");
}

#[test]
fn a_beam_at_other_power_is_caught() {
    // Case 6's corrosive bolt at power 40: at 80 its accuracy and dice are both greater.
    let dump = with_changed(PLAYER, 6, |case| case["power"] = 80.into());
    let measures: Vec<Measure> = caught(&dump).into_iter().map(|(_, m)| m).collect();
    assert!(measures.contains(&Measure::BeamToHit), "{measures:?}");
    assert!(measures.contains(&Measure::Damage), "{measures:?}");
}

#[test]
fn a_wand_zapped_with_other_evocations_is_caught() {
    let dump = with_changed(PLAYER, 10, |case| {
        let evocations = case["evocations"].as_i64().unwrap();
        case["evocations"] = (evocations + 179_200 * 4).into();
    });
    assert_eq!(caught(&dump), [(10, Measure::WandPower)]);
}

#[test]
fn another_kinds_hit_points_are_caught() {
    // Case 3 makes draconian knights; a jackal has far fewer hit points.
    let dump = with_changed(PLAYER, 3, |case| case["monster"] = 0.into());
    assert_eq!(caught(&dump), [(3, Measure::HitPoints)]);
}

//! Which of the player's items, spells and abilities are Ways Out, and the statuses that stop
//! them: a judgement the Advisor makes, ported by hand for each release series, with every item,
//! spell, ability and status checked against crawl's source so a renamed one stops the
//! extraction. The statuses that stop one ability in particular are read from the code that
//! checks it, and the colour the ability menu greys an ability out in from crawl's enums.

use std::collections::{BTreeMap, BTreeSet};

use copilot_gk::{
    EnumId, EscapeAbility, EscapeItem, EscapeSpell, ItemClass, Relief, Spell, Stop, Use,
    WayOutSigns,
};

use crate::cpp::{self, Source, Token};
use crate::{Crawl, ExtractError, Shown};

/// What a release series counts as a Way Out, and what stops each.
pub struct WayOutRules {
    /// Each kind of item.
    pub items: &'static [ItemRule],
    /// Each spell.
    pub spells: &'static [SpellRule],
    /// Each ability.
    pub abilities: &'static [AbilityRule],
    /// Each status that stops something, by its light or the words the status pane shows for
    /// it.
    pub stops: &'static [(Shown, &'static [Use])],
    /// Where crawl checks whether an ability can be used, which the statuses that stop each are
    /// read from.
    pub ability_checks: AbilityChecks,
    /// The `spell_highlight_colours` enumerator (`spl-util.h`) the ability menu draws an
    /// ability in when it can't be used now.
    pub inapplicable_colour: &'static str,
    /// The message crawl gives for the spell list when the player knows no spells.
    pub no_spells: Shown,
    /// The message crawl gives for the ability menu when the player has no abilities.
    pub no_abilities: Shown,
}

/// A kind of item that is a Way Out.
pub struct ItemRule {
    /// Its `object_class_type` identifier, such as `OBJ_SCROLLS`.
    pub class: &'static str,
    /// Its sub-type's identifier, such as `SCR_TELEPORTATION`.
    pub sub_type: &'static str,
    /// What it does.
    pub effect: Relief,
    /// It translocates the player.
    pub translocation: bool,
}

impl ItemRule {
    /// A kind of item that does `effect` without translocating the player.
    const fn new(class: &'static str, sub_type: &'static str, effect: Relief) -> Self {
        Self {
            class,
            sub_type,
            effect,
            translocation: false,
        }
    }

    /// This kind of item, translocating the player.
    const fn translocating(self) -> Self {
        Self {
            translocation: true,
            ..self
        }
    }
}

/// A spell that is a Way Out.
pub struct SpellRule {
    /// Its `spell_type` identifier, such as `SPELL_BLINK`.
    pub spell: &'static str,
    /// What it does.
    pub effect: Relief,
    /// It translocates the player.
    pub translocation: bool,
}

impl SpellRule {
    /// A spell that does `effect` without translocating the player.
    const fn new(spell: &'static str, effect: Relief) -> Self {
        Self {
            spell,
            effect,
            translocation: false,
        }
    }

    /// This spell, translocating the player.
    const fn translocating(self) -> Self {
        Self {
            translocation: true,
            ..self
        }
    }
}

/// An ability that is a Way Out, by the name `ability.cc` gives it.
pub struct AbilityRule {
    /// Its name, which `ability.cc`'s table of abilities must give `ability`.
    pub name: &'static str,
    /// Its `ability_type` identifier, such as `ABIL_TROG_BERSERK`.
    pub ability: &'static str,
    /// What it does.
    pub effect: Relief,
    /// It is a god's power (`talent::is_invocation`).
    pub invocation: bool,
    /// It translocates the player.
    pub translocation: bool,
}

impl AbilityRule {
    /// An ability that isn't a god's power and does `effect` without translocating the player.
    const fn new(name: &'static str, ability: &'static str, effect: Relief) -> Self {
        Self {
            name,
            ability,
            effect,
            invocation: false,
            translocation: false,
        }
    }

    /// A god's power that does `effect` without translocating the player.
    const fn invocation(name: &'static str, ability: &'static str, effect: Relief) -> Self {
        Self {
            invocation: true,
            ..Self::new(name, ability, effect)
        }
    }

    /// This ability, translocating the player.
    const fn translocating(self) -> Self {
        Self {
            translocation: true,
            ..self
        }
    }
}

/// Where crawl checks whether an ability can be used. An ability is stopped by each status
/// whose duration, as `you.duration[DUR_EXHAUSTED]`, or whose test, as `you.beheld()`, the
/// checks read without negating it: in its arm of `possible`'s `switch`, and in the helpers that
/// arm, its arm of `used`'s `switch` or another helper calls.
pub struct AbilityChecks {
    /// The function in `ability.cc` that checks an ability before it is used, and as the menu
    /// greys it out or not, an arm of its `switch` for each ability with checks of its own.
    pub possible: &'static str,
    /// The function in `ability.cc` that uses an ability, whose arms call helpers that check
    /// some abilities only as they are used. Only its helper calls are read, as its arms'
    /// other code is the abilities' effects.
    pub used: &'static str,
    /// The functions that check abilities, each with its file, followed wherever an arm or
    /// another helper calls them: every overload of each.
    pub helpers: &'static [(&'static str, &'static str)],
    /// The tests of a status that isn't a duration, by the function crawl tests it with, with
    /// the light the status pane shows for it.
    pub tests: &'static [(&'static str, Shown)],
}

/// What crawl 0.34 counts as a Way Out, and what stops each: reading as
/// `_general_cannot_read_reason` (`item-name.cc:3062`), drinking as `cannot_drink_item_reason`
/// (`item-name.cc:3205`), casting as `can_cast_spells` (`spl-cast.cc:712`), abilities as
/// `_check_ability_possible` (`ability.cc:1803`) and `_do_ability` (`ability.cc:3216`), and
/// translocation as `player::no_tele`.
pub const WAY_OUT_RULES_0_34: WayOutRules = WayOutRules {
    items: &[
        ItemRule::new("OBJ_POTIONS", "POT_HEAL_WOUNDS", Relief::Heals),
        ItemRule::new("OBJ_POTIONS", "POT_CURING", Relief::Heals),
        ItemRule::new("OBJ_POTIONS", "POT_HASTE", Relief::Hastens),
        ItemRule::new("OBJ_POTIONS", "POT_INVISIBILITY", Relief::Hinders),
        ItemRule::new("OBJ_POTIONS", "POT_BERSERK_RAGE", Relief::Strengthens),
        ItemRule::new("OBJ_SCROLLS", "SCR_BLINKING", Relief::Escapes).translocating(),
        ItemRule::new("OBJ_SCROLLS", "SCR_TELEPORTATION", Relief::EscapesLater).translocating(),
        ItemRule::new("OBJ_SCROLLS", "SCR_FOG", Relief::Hinders),
        ItemRule::new("OBJ_SCROLLS", "SCR_FEAR", Relief::Hinders),
    ],
    spells: &[
        SpellRule::new("SPELL_BLINK", Relief::Escapes).translocating(),
        SpellRule::new("SPELL_GOLUBRIAS_PASSAGE", Relief::Escapes).translocating(),
        SpellRule::new("SPELL_PASSWALL", Relief::EscapesLater),
        SpellRule::new("SPELL_SWIFTNESS", Relief::Hastens),
    ],
    abilities: &[
        AbilityRule::new("Evoke Blink", "ABIL_EVOKE_BLINK", Relief::Escapes).translocating(),
        AbilityRule::new("Blinkbolt", "ABIL_BLINKBOLT", Relief::Escapes).translocating(),
        AbilityRule::new("Hop", "ABIL_HOP", Relief::Escapes),
        AbilityRule::new("Heal Wounds", "ABIL_HEAL_WOUNDS", Relief::Heals),
        AbilityRule::invocation("Berserk", "ABIL_TROG_BERSERK", Relief::Strengthens),
        AbilityRule::invocation("Sanctuary", "ABIL_ZIN_SANCTUARY", Relief::Hinders),
        AbilityRule::invocation("Heal Self", "ABIL_ELYVILON_HEAL_SELF", Relief::Heals),
        AbilityRule::invocation("Draw Escape", "ABIL_NEMELEX_DRAW_ESCAPE", Relief::Escapes),
        AbilityRule::invocation("Bend Time", "ABIL_CHEIBRIADOS_TIME_BEND", Relief::Hinders),
        AbilityRule::invocation(
            "Step From Time",
            "ABIL_CHEIBRIADOS_TIME_STEP",
            Relief::Escapes,
        ),
        AbilityRule::invocation("Shadowslip", "ABIL_DITHMENOS_SHADOWSLIP", Relief::Escapes),
        AbilityRule::invocation("Draw Out Power", "ABIL_RU_DRAW_OUT_POWER", Relief::Heals),
        AbilityRule::invocation(
            "Serpent's Lash",
            "ABIL_WU_JIAN_SERPENTS_LASH",
            Relief::Hastens,
        ),
        AbilityRule::invocation(
            "Rising Flame",
            "ABIL_IGNIS_RISING_FLAME",
            Relief::EscapesLater,
        ),
    ],
    stops: &[
        (
            Shown::quoted("Conf", "duration-data.h"),
            &[Use::Reading, Use::Casting, Use::UsingAbilities],
        ),
        (
            Shown::quoted("Berserk", "duration-data.h"),
            &[
                Use::Reading,
                Use::Drinking,
                Use::Casting,
                Use::UsingAbilities,
            ],
        ),
        (
            // Silence shows its light only beside the equipment bar, and its words always.
            Shown::quoted("silenced", "status.cc"),
            &[Use::Reading, Use::Casting, Use::Invoking],
        ),
        (Shown::quoted("-Scroll", "status.cc"), &[Use::Reading]),
        (Shown::quoted("-Potion", "status.cc"), &[Use::Drinking]),
        (Shown::quoted("-Cast", "duration-data.h"), &[Use::Casting]),
        (
            Shown::quoted("-Tele", "duration-data.h"),
            &[Use::Translocating],
        ),
    ],
    ability_checks: AbilityChecks {
        possible: "_check_ability_possible",
        used: "_do_ability",
        helpers: &[
            ("_can_hop", ABILITY_FILE),
            ("_can_blinkbolt", ABILITY_FILE),
            ("_can_rising_flame", ABILITY_FILE),
            ("can_go_berserk", "player-act.cc"),
            ("berserk", "player-act.cc"),
        ],
        // `fill_status_info`'s light for STATUS_BEHELD.
        tests: &[("beheld", Shown::quoted("Mesm", "status.cc"))],
    },
    inapplicable_colour: "COL_INAPPLICABLE",
    // MSG_NO_SPELLS, as `canned_msg` gives it.
    no_spells: Shown::quoted("You don't know any spells.", "message.cc"),
    // `no_ability_msg`.
    no_abilities: Shown::quoted(
        "Sorry, you're not good enough to have a special ability.",
        "ability.cc",
    ),
};

/// The file of the abilities' table and the checks on them.
const ABILITY_FILE: &str = "ability.cc";

/// The table in `ability.cc` that names each ability.
const ABILITY_TABLE: &str = "Ability_List";

/// The file of the durations' lights and words.
const DURATION_FILE: &str = "duration-data.h";

/// The file of the highlight colours, the ability menu's among them.
const HIGHLIGHT_FILE: &str = "spl-util.h";

/// The enum of the highlight colours.
const HIGHLIGHT_COLOURS: &str = "spell_highlight_colours";

/// The file of crawl's colours.
const COLOURS_FILE: &str = "defines.h";

/// The enum of crawl's colours, in the order the game numbers them.
const COLOURS: &str = "COLOURS";

impl Crawl {
    /// The Ways Out of `rules`, each found among `item_classes` and `spells` or in crawl's source.
    pub(crate) fn ways_out(
        &self,
        rules: &WayOutRules,
        item_classes: &BTreeMap<u8, ItemClass>,
        spells: &BTreeMap<u16, Spell>,
    ) -> Result<WayOutSigns, ExtractError> {
        let items = rules
            .items
            .iter()
            .map(|rule| {
                let known = item_classes.values().any(|known| {
                    known.id.0 == rule.class
                        && known
                            .sub_types
                            .values()
                            .any(|known| known.id.0 == rule.sub_type)
                });
                if !known {
                    return Err(ExtractError::Missing {
                        file: "item-prop-enum.h".to_owned(),
                        what: format!("{}, an item of {}", rule.sub_type, rule.class),
                    });
                }
                Ok(EscapeItem {
                    class: EnumId(rule.class.to_owned()),
                    sub_type: EnumId(rule.sub_type.to_owned()),
                    effect: rule.effect,
                    translocation: rule.translocation,
                })
            })
            .collect::<Result<_, _>>()?;
        let spells = rules
            .spells
            .iter()
            .map(|rule| {
                if !spells.values().any(|known| known.id.0 == rule.spell) {
                    return Err(ExtractError::Missing {
                        file: "spell-type.h".to_owned(),
                        what: rule.spell.to_owned(),
                    });
                }
                Ok(EscapeSpell {
                    spell: EnumId(rule.spell.to_owned()),
                    effect: rule.effect,
                    translocation: rule.translocation,
                })
            })
            .collect::<Result<_, _>>()?;
        let ability_source = self.parse(ABILITY_FILE)?;
        let named = ability_source.table(ABILITY_TABLE)?;
        let durations = self.duration_statuses()?;
        let abilities = rules
            .abilities
            .iter()
            .map(|rule| {
                if !named
                    .iter()
                    .any(|(id, name)| id == rule.ability && name == rule.name)
                {
                    return Err(ExtractError::Missing {
                        file: ABILITY_FILE.to_owned(),
                        what: format!("{}, named {}", rule.ability, rule.name),
                    });
                }
                Ok(EscapeAbility {
                    name: rule.name.to_owned(),
                    effect: rule.effect,
                    invocation: rule.invocation,
                    translocation: rule.translocation,
                    stopped_by: self.stopped_by(
                        &rules.ability_checks,
                        &ability_source,
                        rule.ability,
                        &durations,
                    )?,
                })
            })
            .collect::<Result<_, ExtractError>>()?;
        let stops = rules
            .stops
            .iter()
            .map(|(status, stops)| {
                Ok(Stop {
                    status: self.shown(status)?,
                    stops: stops.to_vec(),
                })
            })
            .collect::<Result<_, ExtractError>>()?;
        Ok(WayOutSigns {
            items,
            spells,
            abilities,
            stops,
            inapplicable_colour: self.colour(rules.inapplicable_colour)?,
            no_spells: self.shown(&rules.no_spells)?,
            no_abilities: self.shown(&rules.no_abilities)?,
        })
    }
}

impl Crawl {
    /// The statuses that stop `ability`, as the status pane shows them, read from `checks` in
    /// `source`, `ability.cc`, with each duration's light or words from `durations`.
    fn stopped_by(
        &self,
        checks: &AbilityChecks,
        source: &Source,
        ability: &str,
        durations: &BTreeMap<String, String>,
    ) -> Result<Vec<String>, ExtractError> {
        let mut checked = Checked::default();
        if let Some(arm) = cpp::case_arm(source.function_body(checks.possible)?, ability) {
            checked.checks(arm, checks);
        }
        if let Some(arm) = cpp::case_arm(source.function_body(checks.used)?, ability) {
            checked.calls(arm, checks);
        }
        let mut followed = BTreeSet::new();
        while let Some((helper, file)) = checked.helpers.pop() {
            if !followed.insert(helper) {
                continue;
            }
            let source = self.parse(file)?;
            let bodies = source.function_bodies(helper)?;
            if bodies.is_empty() {
                return Err(ExtractError::Missing {
                    file: file.to_owned(),
                    what: format!("the function {helper}"),
                });
            }
            for body in bodies {
                checked.checks(body, checks);
            }
        }
        let mut stopped_by = Vec::new();
        for duration in checked.durations {
            let status = durations
                .get(&duration)
                .ok_or_else(|| ExtractError::Missing {
                    file: DURATION_FILE.to_owned(),
                    what: format!("the light or words of {duration}"),
                })?;
            push_new(&mut stopped_by, status);
        }
        for test in checked.tests {
            let shown = checks
                .tests
                .iter()
                .find_map(|(name, shown)| (*name == test).then_some(shown))
                .ok_or_else(|| ExtractError::Missing {
                    file: ABILITY_FILE.to_owned(),
                    what: format!("the test {test}"),
                })?;
            push_new(&mut stopped_by, &self.shown(shown)?);
        }
        Ok(stopped_by)
    }

    /// What the status pane shows for each duration, by its `duration_type` identifier: its
    /// light, or its words when it has none (`duration_data`). One with neither is left out.
    fn duration_statuses(&self) -> Result<BTreeMap<String, String>, ExtractError> {
        let source = self.parse(DURATION_FILE)?;
        let mut statuses = BTreeMap::new();
        for fields in source.entries("duration_data")? {
            let (Some([Token::Ident(id)]), Some([Token::Str(light)]), Some([Token::Str(words)])) = (
                fields.first().map(Vec::as_slice),
                fields.get(2).map(Vec::as_slice),
                fields.get(3).map(Vec::as_slice),
            ) else {
                return Err(ExtractError::Unreadable {
                    file: DURATION_FILE.to_owned(),
                    what: "an entry of duration_data".to_owned(),
                });
            };
            let shown = if light.is_empty() { words } else { light };
            if !shown.is_empty() {
                statuses.insert(id.clone(), shown.clone());
            }
        }
        Ok(statuses)
    }

    /// The number crawl gives the highlight colour `name`: the `COLOURS` enumerator it is, or
    /// that an earlier highlight colour it names is. `COLOURS` is read from its own declaration
    /// in `defines.h`, as the file's other conditionals test the build's CPU.
    fn colour(&self, name: &str) -> Result<u8, ExtractError> {
        let highlights = self.parse(HIGHLIGHT_FILE)?;
        let highlights = highlights.enumerators(HIGHLIGHT_COLOURS)?;
        let mut colour = name;
        while let Some((_, init)) = highlights.iter().find(|(ident, _)| *ident == colour) {
            let Some([Token::Ident(aliased)]) = init else {
                return Err(ExtractError::Unreadable {
                    file: HIGHLIGHT_FILE.to_owned(),
                    what: format!("the value of {colour}"),
                });
            };
            colour = aliased;
        }
        let text = crate::read(&self.source.join(COLOURS_FILE))?;
        let declaration = text
            .find(&format!("enum {COLOURS}"))
            .and_then(|start| {
                let end = start + text[start..].find("};")? + 2;
                text.get(start..end)
            })
            .ok_or_else(|| ExtractError::Missing {
                file: COLOURS_FILE.to_owned(),
                what: format!("enum {COLOURS}"),
            })?;
        let value = Source::parse(COLOURS_FILE, declaration, self.tag_major)?
            .enumeration(COLOURS)?
            .into_iter()
            .find(|(ident, _)| ident == colour)
            .map(|(_, value)| value)
            .ok_or_else(|| ExtractError::Missing {
                file: COLOURS_FILE.to_owned(),
                what: format!("{colour}, a colour of {COLOURS}"),
            })?;
        u8::try_from(value).map_err(|_| ExtractError::Unreadable {
            file: COLOURS_FILE.to_owned(),
            what: format!("the value of {colour}"),
        })
    }
}

/// What the checks on one ability were found to read: the durations and tests that stop it, in
/// the order they are read, and the helpers still to follow, each with its file.
#[derive(Default)]
struct Checked {
    durations: Vec<String>,
    tests: Vec<String>,
    helpers: Vec<(&'static str, &'static str)>,
}

impl Checked {
    /// Reads `tokens`, code that checks an ability: each duration and test it reads without
    /// negating it, and each helper it calls.
    fn checks(&mut self, tokens: &[Token], checks: &AbilityChecks) {
        for (i, token) in tokens.iter().enumerate() {
            let Token::Ident(ident) = token else { continue };
            if negated(tokens, i) {
                continue;
            }
            match (ident.as_str(), tokens.get(i + 1), tokens.get(i + 2)) {
                ("duration", Some(Token::Punct('[')), Some(Token::Ident(duration)))
                    if tokens.get(i + 3) == Some(&Token::Punct(']'))
                        && !assigned(tokens, i + 4) =>
                {
                    push_new(&mut self.durations, duration);
                }
                (test, Some(Token::Punct('(')), _)
                    if checks.tests.iter().any(|(name, _)| *name == test) =>
                {
                    push_new(&mut self.tests, test);
                }
                _ => {}
            }
        }
        self.calls(tokens, checks);
    }

    /// Notes each helper `tokens` calls, to follow.
    fn calls(&mut self, tokens: &[Token], checks: &AbilityChecks) {
        for window in tokens.windows(2) {
            if let [Token::Ident(ident), Token::Punct('(')] = window
                && let Some(&helper) = checks.helpers.iter().find(|(name, _)| name == ident)
            {
                self.helpers.push(helper);
            }
        }
    }
}

/// Adds `item` to `list` unless it is there already.
fn push_new(list: &mut Vec<String>, item: &str) {
    if !list.iter().any(|seen| seen == item) {
        list.push(item.to_owned());
    }
}

/// Whether the expression at `at` in `tokens` is negated: `!` before it, or before the
/// `you.` or `you->` it is read from.
fn negated(tokens: &[Token], at: usize) -> bool {
    let object = match tokens.get(..at) {
        Some([.., Token::Ident(you), Token::Punct('.')]) if you == "you" => at - 2,
        Some([.., Token::Ident(you), Token::Punct('-'), Token::Punct('>')]) if you == "you" => {
            at - 3
        }
        _ => at,
    };
    object > 0 && tokens.get(object - 1) == Some(&Token::Punct('!'))
}

/// Whether the token at `at` assigns to what is before it: `=`, but not `==`.
fn assigned(tokens: &[Token], at: usize) -> bool {
    tokens.get(at) == Some(&Token::Punct('=')) && tokens.get(at + 1) != Some(&Token::Punct('='))
}

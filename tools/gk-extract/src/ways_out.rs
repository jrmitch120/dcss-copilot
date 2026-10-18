//! Which of the player's items, spells and abilities are Ways Out, and the statuses that stop
//! them: a judgement the Advisor makes, ported by hand for each release series, with every item,
//! spell, ability and status checked against crawl's source so a renamed one stops the
//! extraction.

use std::collections::BTreeMap;

use copilot_gk::{
    EnumId, EscapeAbility, EscapeItem, EscapeSpell, ItemClass, Relief, Spell, Stop, Use,
    WayOutSigns,
};

use crate::{Crawl, ExtractError, Shown};

/// What a release series counts as a Way Out, and what stops each.
pub struct WayOutRules {
    /// Each kind of item: its class, its sub-type, what it does and whether it translocates.
    pub items: &'static [(&'static str, &'static str, Relief, bool)],
    /// Each spell: its `spell_type` identifier, what it does and whether it translocates.
    pub spells: &'static [(&'static str, Relief, bool)],
    /// Each ability.
    pub abilities: &'static [AbilityRule],
    /// Each status that stops something, by its light or the words the status pane shows for
    /// it.
    pub stops: &'static [(Shown, &'static [Use])],
}

/// An ability that is a Way Out, by the name `ability.cc` gives it.
pub struct AbilityRule {
    /// Its name, which `ability.cc` must hold as a string literal.
    pub name: &'static str,
    /// What it does.
    pub effect: Relief,
    /// It is a god's power (`talent::is_invocation`).
    pub invocation: bool,
    /// It translocates the player.
    pub translocation: bool,
}

impl AbilityRule {
    const fn new(
        name: &'static str,
        effect: Relief,
        invocation: bool,
        translocation: bool,
    ) -> Self {
        Self {
            name,
            effect,
            invocation,
            translocation,
        }
    }
}

/// What crawl 0.34 counts as a Way Out, and what stops each: reading as
/// `_general_cannot_read_reason` (`item-name.cc:3062`), drinking as `cannot_drink_item_reason`
/// (`item-name.cc:3205`), casting as `can_cast_spells` (`spl-cast.cc:712`), abilities as
/// `_check_ability_possible` (`ability.cc:1803`), and translocation as `player::no_tele`.
pub const WAY_OUT_RULES_0_34: WayOutRules = WayOutRules {
    items: &[
        ("OBJ_POTIONS", "POT_HEAL_WOUNDS", Relief::Heals, false),
        ("OBJ_POTIONS", "POT_CURING", Relief::Heals, false),
        ("OBJ_POTIONS", "POT_HASTE", Relief::Hastens, false),
        ("OBJ_POTIONS", "POT_INVISIBILITY", Relief::Hinders, false),
        (
            "OBJ_POTIONS",
            "POT_BERSERK_RAGE",
            Relief::Strengthens,
            false,
        ),
        ("OBJ_SCROLLS", "SCR_BLINKING", Relief::Escapes, true),
        (
            "OBJ_SCROLLS",
            "SCR_TELEPORTATION",
            Relief::EscapesLater,
            true,
        ),
        ("OBJ_SCROLLS", "SCR_FOG", Relief::Hinders, false),
        ("OBJ_SCROLLS", "SCR_FEAR", Relief::Hinders, false),
    ],
    spells: &[
        ("SPELL_BLINK", Relief::Escapes, true),
        ("SPELL_GOLUBRIAS_PASSAGE", Relief::Escapes, true),
        ("SPELL_PASSWALL", Relief::EscapesLater, false),
        ("SPELL_SWIFTNESS", Relief::Hastens, false),
    ],
    abilities: &[
        AbilityRule::new("Evoke Blink", Relief::Escapes, false, true),
        AbilityRule::new("Blinkbolt", Relief::Escapes, false, true),
        AbilityRule::new("Hop", Relief::Escapes, false, false),
        AbilityRule::new("Heal Wounds", Relief::Heals, false, false),
        AbilityRule::new("Berserk", Relief::Strengthens, true, false),
        AbilityRule::new("Sanctuary", Relief::Hinders, true, false),
        AbilityRule::new("Heal Self", Relief::Heals, true, false),
        AbilityRule::new("Draw Escape", Relief::Escapes, true, false),
        AbilityRule::new("Bend Time", Relief::Hinders, true, false),
        AbilityRule::new("Step From Time", Relief::Escapes, true, false),
        AbilityRule::new("Shadowslip", Relief::Escapes, true, false),
        AbilityRule::new("Draw Out Power", Relief::Heals, true, false),
        AbilityRule::new("Serpent's Lash", Relief::Hastens, true, false),
        AbilityRule::new("Rising Flame", Relief::EscapesLater, true, false),
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
};

/// The file every ability's name must be in.
const ABILITY_FILE: &str = "ability.cc";

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
            .map(|&(class, sub_type, effect, translocation)| {
                let known = item_classes.values().any(|known| {
                    known.id.0 == class
                        && known.sub_types.values().any(|known| known.id.0 == sub_type)
                });
                if !known {
                    return Err(ExtractError::Missing {
                        file: "item-prop-enum.h".to_owned(),
                        what: format!("{sub_type}, an item of {class}"),
                    });
                }
                Ok(EscapeItem {
                    class: EnumId(class.to_owned()),
                    sub_type: EnumId(sub_type.to_owned()),
                    effect,
                    translocation,
                })
            })
            .collect::<Result<_, _>>()?;
        let spells = rules
            .spells
            .iter()
            .map(|&(spell, effect, translocation)| {
                if !spells.values().any(|known| known.id.0 == spell) {
                    return Err(ExtractError::Missing {
                        file: "spell-type.h".to_owned(),
                        what: spell.to_owned(),
                    });
                }
                Ok(EscapeSpell {
                    spell: EnumId(spell.to_owned()),
                    effect,
                    translocation,
                })
            })
            .collect::<Result<_, _>>()?;
        let abilities = rules
            .abilities
            .iter()
            .map(|rule| {
                Ok(EscapeAbility {
                    name: self.shown(&Shown::quoted(rule.name, ABILITY_FILE))?,
                    effect: rule.effect,
                    invocation: rule.invocation,
                    translocation: rule.translocation,
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
        })
    }
}

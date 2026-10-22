//! The Ways Out the player carries, has as abilities or knows as spells, each with what stops it
//! right now.

use std::collections::BTreeSet;

use copilot_gk::{Bundle, EscapeAbility, Relief, Use};

use super::{Blocker, Escape, Usable, WayOut, current};
use crate::accumulated_state::AccumulatedState;
use crate::crawl_ids::object_classes;
use crate::fact_sheet::Gap;
use crate::visible_state::VisibleState;

/// What the player carries, has and knows, with the Game Knowledge that says which are Ways
/// Out.
pub(super) struct Carried<'a> {
    pub(super) visible: &'a VisibleState,
    pub(super) remembered: Option<&'a AccumulatedState>,
    pub(super) bundle: &'a Bundle,
}

impl Carried<'_> {
    /// The items in the inventory that are Ways Out, by letter. Crawl's own judgement of each
    /// says whether it is of use right now, and the player's statuses say why not where they
    /// can.
    pub(super) fn items(&self) -> Vec<WayOut> {
        let signs = &self.bundle.ways_out;
        self.visible
            .inventory
            .iter()
            .flatten()
            .filter_map(|item| {
                let kind = item.kind.as_ref()?;
                let sub_type = kind.sub_type.as_ref()?;
                let escape = signs
                    .items
                    .iter()
                    .find(|escape| escape.class == kind.class && escape.sub_type == *sub_type)?;
                let kind_name = self
                    .bundle
                    .item_classes
                    .values()
                    .find(|class| class.id == kind.class)?
                    .sub_types
                    .values()
                    .find(|known| known.id == *sub_type)?
                    .name
                    .clone();
                let usable = if item.useless {
                    let mut uses = Vec::new();
                    match escape.class.0.as_str() {
                        object_classes::SCROLLS => uses.push(Use::Reading),
                        object_classes::POTIONS => uses.push(Use::Drinking),
                        _ => {}
                    }
                    if escape.translocation {
                        uses.push(Use::Translocating);
                    }
                    let mut because = self.stopped(&uses, &[]);
                    if because.is_empty() {
                        let stats = &self.visible.stats;
                        let full_health = stats.hp.is_some() && stats.hp == stats.hp_max;
                        because.push(if escape.effect == Relief::Heals && full_health {
                            Blocker::FullHealth
                        } else {
                            Blocker::Useless
                        });
                    }
                    Usable::No { because }
                } else {
                    Usable::Yes
                };
                Some(WayOut {
                    option: Escape::Item {
                        letter: item.letter,
                        name: item.name.clone(),
                        kind_name,
                        quantity: item.quantity,
                        effect: escape.effect,
                    },
                    usable,
                })
            })
            .collect()
    }

    /// The abilities that are Ways Out, as the ability menu last showed them, each stopped by
    /// the player's statuses or their magic, or greyed out in the menu for a reason the Advisor
    /// couldn't see. Never having read the menu is a Gap in `gaps`:
    /// abilities come from gods, species, mutations and gear alike, and crawl answers the menu
    /// key even when there are none.
    pub(super) fn abilities(&self, gaps: &mut BTreeSet<Gap>) -> Vec<WayOut> {
        let Some((abilities, read)) = self
            .remembered
            .and_then(|remembered| current(&remembered.abilities))
        else {
            gaps.insert(Gap::AbilityMenu);
            return Vec::new();
        };
        abilities
            .iter()
            .filter_map(|ability| {
                let escape = self.escape_ability(&ability.name)?;
                Some(WayOut {
                    option: Escape::Ability {
                        name: ability.name.clone(),
                        effect: escape.effect,
                        failure_percent: ability.failure_percent,
                        read,
                    },
                    usable: usable(
                        self.ability_stopped(escape, ability.magic_cost)
                            .into_iter()
                            .chain(ability.greyed_out.then_some(Blocker::GreyedOut { read }))
                            .collect(),
                    ),
                })
            })
            .collect()
    }

    /// The ability named `name`, as the ability menu lists it, if it is a Way Out.
    pub(super) fn escape_ability(&self, name: &str) -> Option<&EscapeAbility> {
        self.bundle
            .ways_out
            .abilities
            .iter()
            .find(|escape| escape.name == name)
    }

    /// What stops `escape`, which costs `magic` points, now: the statuses that stop using
    /// abilities, invoking or translocating as it does, those that stop it in particular, and
    /// too little magic.
    pub(super) fn ability_stopped(&self, escape: &EscapeAbility, magic: u32) -> Vec<Blocker> {
        let mut uses = vec![Use::UsingAbilities];
        if escape.invocation {
            uses.push(Use::Invoking);
        }
        if escape.translocation {
            uses.push(Use::Translocating);
        }
        let mut because = self.stopped(&uses, &escape.stopped_by);
        because.extend(self.short_of_magic(magic));
        because
    }

    /// The spells that are Ways Out, as the spell list last showed them, each stopped by the
    /// player's statuses or their magic. Never having read the list is a Gap in `gaps` for a
    /// player with magic to cast with.
    pub(super) fn spells(&self, gaps: &mut BTreeSet<Gap>) -> Vec<WayOut> {
        let Some((spells, read)) = self
            .remembered
            .and_then(|remembered| current(&remembered.spells))
        else {
            if self.visible.stats.mp_max.is_some_and(|mp| mp > 0) {
                gaps.insert(Gap::SpellList);
            }
            return Vec::new();
        };
        let signs = &self.bundle.ways_out;
        spells
            .iter()
            .filter_map(|spell| {
                let escape = signs.spells.iter().find(|escape| {
                    self.bundle
                        .spell_by_id(&escape.spell)
                        .is_some_and(|known| known.name == spell.name)
                })?;
                let mut uses = vec![Use::Casting];
                if escape.translocation {
                    uses.push(Use::Translocating);
                }
                Some(WayOut {
                    option: Escape::Spell {
                        name: spell.name.clone(),
                        effect: escape.effect,
                        failure_percent: spell.failure_percent,
                        read,
                    },
                    // A spell costs as many magic points as its level.
                    usable: usable(
                        self.stopped(&uses, &[])
                            .into_iter()
                            .chain(self.short_of_magic(spell.level))
                            .collect(),
                    ),
                })
            })
            .collect()
    }

    /// Too little magic for something that costs `magic` points, if the player has.
    fn short_of_magic(&self, magic: u32) -> Option<Blocker> {
        let has = self.visible.stats.mp?;
        (magic > 0 && i64::from(has) < i64::from(magic))
            .then_some(Blocker::Magic { needs: magic, has })
    }

    /// The player's statuses that stop any of `uses`, or that are among `own`, the statuses
    /// that stop one thing in particular, in the order the status pane lists them.
    fn stopped(&self, uses: &[Use], own: &[String]) -> Vec<Blocker> {
        let stops = &self.bundle.ways_out.stops;
        self.visible
            .statuses
            .iter()
            .flatten()
            .filter(|status| {
                let shows = |shown: &String| {
                    status.light.as_ref() == Some(shown) || status.text.as_ref() == Some(shown)
                };
                own.iter().any(shows)
                    || stops.iter().any(|stop| {
                        shows(&stop.status)
                            && stop.stops.iter().any(|stopped| uses.contains(stopped))
                    })
            })
            .filter_map(|status| {
                Some(Blocker::Status {
                    status: status.text.clone().or_else(|| status.light.clone())?,
                })
            })
            .collect()
    }
}

/// Usable unless something in `because` stops it.
fn usable(because: Vec<Blocker>) -> Usable {
    if because.is_empty() {
        Usable::Yes
    } else {
        Usable::No { because }
    }
}

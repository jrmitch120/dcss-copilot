//! The Fact Sheet: everything the Advisor has computed about one moment, and the Gaps that kept
//! it from computing more.

use std::collections::BTreeSet;

use copilot_gk::{Bundle, LightingSigns, MonsterSpells};
use serde::Serialize;
use specta::Type;

use crate::accumulated_state::{AccumulatedState, GearSource, Remembered};
use crate::carried_gear::{Unlisted, Unplayed};
use crate::crawl_ids::{attack_flavours, genera, monster_types};
use crate::engine::lighting::Lighting;
use crate::engine::melee::DisplayedDefences;
use crate::engine::resists::{Holiness, Needs, Resists};
use crate::engine::spells::{DamagingSpell, Reach};
use crate::engine::threat::{self, Attacker, Defender, Outcome};
use crate::screen::{Resistance, Resistances, Screen};
use crate::visible_state::{
    Attitude, Position, Religion, SquareLight, VisibleMonster, VisibleState,
};
use crate::ways_out::WaysOut;

/// LETHAL from this chance of death before the player's next action, per mille: 5%. A starting
/// threshold from the spec (#24, "Engine (ADR 0003)"), to be tuned on the corpus.
const LETHAL_DEATHS_PER_MILLE: u32 = 50;

/// DANGEROUS from this chance of death, per mille: 0.5%. A starting threshold from the spec (#24,
/// "Engine (ADR 0003)"), to be tuned on the corpus.
const DANGEROUS_DEATHS_PER_MILLE: u32 = 5;

/// DANGEROUS when the worst-case hit is at least this percentage of current HP. A starting
/// threshold from the spec (#24, "Engine (ADR 0003)"), to be tuned on the corpus.
const DANGEROUS_WORST_HIT_PERCENT_OF_HP: i32 = 40;

/// Everything the Advisor has computed about one moment of the game it follows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactSheet {
    /// How dangerous this moment is.
    pub threat: Threat,
    /// The player's escapes and emergency options, and the ground they stand on.
    pub ways_out: WaysOut,
}

/// How dangerous this moment is, or why the Advisor can't tell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Threat {
    /// The Advisor judged the moment.
    Rated(Box<ThreatAssessment>),
    /// The Advisor lacks facts it needs to judge the moment, so it judges nothing.
    Unrated {
        /// What it lacks.
        gaps: Vec<Gap>,
    },
}

/// The Threat Level and the numbers it was judged from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreatAssessment {
    /// How dangerous the moment is.
    pub level: ThreatLevel,
    /// The player's hit points it was judged against.
    pub hp: i32,
    /// The chance the player dies before their next standard-speed action. Nothing is played
    /// out when nothing can attack.
    pub death: Chance,
    /// The total damage the player takes before their next standard-speed action at the 99th
    /// percentile: as bad as it gets, barring one time in a hundred.
    pub worst_hit: i32,
    /// How many monsters in view would attack the player, whether or not they can yet.
    pub hostiles_in_view: u32,
    /// The monsters that can attack before the player's next standard-speed action, in map
    /// order.
    pub acting: Vec<ActingMonster>,
    /// The gear the simulation played out for each hostile in view, acting or not, in map
    /// order: its Carried Gear, or its kind's Possible Gear when that isn't read.
    pub gear: Vec<GearPlayed>,
    /// How the light falls on the player, which changes how well monsters aim at them. `None`
    /// when where they stand isn't known.
    pub lighting: Option<Lighting>,
    /// The player's resistances the simulation applied, as the character overview or dump last
    /// showed them. `None` when it hasn't been read, and the player is taken to resist nothing
    /// and to be a living mortal who worships no good god.
    pub resistances: Option<ResistancesApplied>,
    /// What the judgement leaves out, so the numbers are read for what they are.
    pub not_modelled: Vec<NotModelled>,
}

/// The player's resistances as the simulation applied them: the last reading of the character
/// overview or dump.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResistancesApplied {
    /// Whether the game has shown anything since that may have changed them. A Stale reading is
    /// applied all the same.
    pub state: KnownOrStale,
    /// The turn they were read on.
    pub turn: Option<u32>,
    /// What they were read off.
    pub source: Screen,
    /// Each resistance's pips, or immunity.
    pub resistances: Resistances,
}

/// How dangerous a moment is: the Threat Level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ThreatLevel {
    /// Neither likely to die nor to be hit hard before the next action.
    Calm,
    /// A real chance of death, or a hard hit, before the next action.
    Dangerous,
    /// Death before the next action is likely enough, or a hit could take every hit point.
    Lethal,
}

/// How often something happened when the engine played a moment out many times: exact, as
/// counted, rather than rounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chance {
    /// Trials in which it happened.
    pub times: u32,
    /// Trials played out.
    pub trials: u32,
}

impl Chance {
    /// No chance at all: nothing could happen, so nothing was played out.
    const NONE: Self = Self {
        times: 0,
        trials: 0,
    };

    /// Whether the chance is at least `per_mille` in a thousand, for a `per_mille` above 0.
    fn at_least_per_mille(self, per_mille: u32) -> bool {
        self.times > 0
            && u64::from(self.times) * 1000 >= u64::from(per_mille) * u64::from(self.trials)
    }

    /// The chance as a percentage.
    #[must_use]
    pub fn percent(self) -> f64 {
        if self.trials == 0 {
            return 0.0;
        }
        f64::from(self.times) * 100.0 / f64::from(self.trials)
    }
}

/// A monster that can attack the player before their next standard-speed action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActingMonster {
    /// Its name, as the game names it.
    pub name: String,
    /// The square it is on.
    pub position: Position,
    /// Squares between it and the player.
    pub distance: i32,
    /// The spells it may cast at the player that the simulation plays out, by name.
    pub casts: Vec<String>,
    /// The other spells of its book, by name: the simulation assumes it doesn't cast them.
    pub spells_not_modelled: Vec<String>,
    /// The ways crawl moves it, or acts for it, that the simulation doesn't play out, in the
    /// order [`NotModelled`] lists them.
    pub moves_not_modelled: Vec<NotModelled>,
    /// The launchers, throwing missiles and wands the simulation plays out for it, by name: those
    /// of its Carried Gear as the game named them, such as `+3 shortbow of flaming`, or those
    /// crawl may make its kind with, such as `shortbow` or `wand of flame`.
    pub shoots: Vec<String>,
}

/// The gear the simulation played out for one hostile in view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GearPlayed {
    /// Its name, as the game names it.
    pub name: String,
    /// The square it is on.
    pub position: Position,
    /// Squares between it and the player.
    pub distance: i32,
    /// Whose gear was played: what it was seen with, or what its kind is made with.
    pub basis: GearBasis,
    /// What of its Carried Gear isn't played out as the game named it.
    pub unplayed: Vec<Unplayed>,
}

impl GearPlayed {
    /// Whether it may shoot, throw or zap by its kind's Possible Gear, which is played as its
    /// Carried Gear isn't read.
    #[must_use]
    pub fn guessed_shooter(&self) -> bool {
        self.basis == GearBasis::Possible { may_shoot: true }
    }
}

/// Whose gear the simulation played out for a monster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "basis",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GearBasis {
    /// Its Carried Gear, as the game last listed it.
    Carried {
        /// Whether the game has shown anything since that may have changed it.
        state: KnownOrStale,
        /// The turn it was read on.
        turn: Option<u32>,
        /// What it was read off.
        source: GearSource,
        /// What that listing leaves out, which is played from its kind's Possible Gear, given
        /// that it is nothing the listing would have named.
        unlisted: Unlisted,
    },
    /// Its kind's Possible Gear, by the odds crawl makes its kind with, as its Carried Gear
    /// hasn't been read.
    Possible {
        /// Whether its kind may be made with a launcher, throwing missiles or a wand.
        may_shoot: bool,
    },
}

/// Whether a remembered fact that was read is Known or Stale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KnownOrStale {
    /// Nothing the game has shown since may have changed it.
    Known,
    /// Something the game has shown since may have changed it.
    Stale,
}

/// Something the engine doesn't model, in the Threat Level's simulation or in judging the Ways
/// Out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NotModelled {
    /// A monster in view that can fight in melee may wield a weapon the Advisor hasn't read: its
    /// Carried Gear isn't read, its listing may leave out a plain weapon in its main hand, it
    /// holds something the Advisor can't name as a weapon, or it is an animated object, such as a
    /// dancing weapon, which fights as the item it is. It fights as if it wields nothing, with no
    /// weapon's damage, accuracy, brand or reach, and still makes the attacks only a weapon
    /// makes. Its look text (`x`) tells it. Monsters' armour and shields aren't counted either,
    /// which matters only to the player's own attacks.
    MonsterEquipment,
    /// Attacks, spells and missiles count only their damage as they land: no poison, burning,
    /// draining, nets, clouds left behind and the like.
    AttackFlavours,
    /// Statuses on either side, such as haste, might or confusion, beyond those that backlight
    /// the player.
    Statuses,
    /// Monsters walk straight at the player, through walls and each other.
    Paths,
    /// Monsters are assumed awake and hunting the player.
    Awareness,
    /// A monster in reach has spells the simulation doesn't play out, such as summons, hexes,
    /// self-buffs and spells that aren't a single bolt or explosion: it is assumed never to cast
    /// them, and to fight instead.
    OtherSpells,
    /// A monster in view is given its own spells when it is made, such as a player ghost, so
    /// which it has isn't known, and none are played out.
    SpellsPerMonster,
    /// A draconian in view breathes by its colour, which isn't played out.
    DraconianBreath,
    /// A mutant beast in view is judged by its kind alone: the tier and facets it was made with
    /// aren't played out, so its reach, attacks, damage, speed and spells may all be wrong.
    MutantBeastFacets,
    /// A monster that can attack may deal damage whose adjustment for the player's resistances
    /// the engine can't settle: the character overview (`%`) hasn't been read, so the player is
    /// taken to resist nothing and to be a living mortal who worships no good god; the damage
    /// needs a resistance the overview doesn't show, such as `rSteam` or resistance to miasma; or
    /// it needs the player's holiness or god, which the Advisor can't tell, as their species,
    /// statuses or god haven't been reported.
    PlayerResistances,
    /// Monsters are assumed to cast explosions and clouds even when they'd catch themselves;
    /// crawl's casters weigh that, and often hold off up close.
    CasterSafety,
    /// A monster in view whose kind may carry a launcher, missiles or a wand is judged by its
    /// kind's Possible Gear, the odds crawl makes its kind with, +0 and unbranded, as the Advisor
    /// hasn't read its Carried Gear: it came back into view, or came with others whose message
    /// didn't list it. Its look text (`x`) tells it.
    CarriedGear,
    /// A monster in reach may carry a wand the simulation doesn't play out, such as paralysis or
    /// polymorph: it is assumed never to zap it, and to shoot or fight instead.
    OtherWands,
    /// The player's shield is assumed to block at most one attack before their next action, as
    /// without a shield, because the Advisor doesn't know which shield is worn. The character
    /// overview (`%`) tells it.
    WornShield,
    /// The player stands in a halo and may have one of their own, which keeps another's from
    /// backlighting them: they carry gear that gives one, such as the morningstar "Eos" or
    /// armour of light, and the Advisor doesn't know whether they wear it. They are assumed to
    /// have none, and to be backlit. The character overview (`%`) tells it.
    OwnHalo,
    /// The player's AC, EV and SH carry fractions crawl rolls with and the game doesn't show:
    /// each trial guesses them afresh, any as likely as another, so the chance of death is an
    /// average over what they could be, and the player's own may give more or less.
    DefenceFractions,
    /// The player is assumed to walk at standard speed, a square an action, whatever their
    /// species, statuses or gear.
    PlayerSpeed,
    /// Everyone is assumed to walk: deep water and lava stop them, even those who swim or fly.
    SwimmingAndFlying,
    /// Monsters are assumed able to cross every unexplored square, and to stand on one to
    /// attack, and the player to walk only over explored ones.
    UnexploredSquares,
    /// A staircase is cut off only by monsters that can stand beside it as the player takes it:
    /// the blows, shots and spells they could land on the way there aren't counted.
    HitsOnTheWay,
    /// Abilities and spells are judged only by their cost in magic and the statuses that stop
    /// them: what each needs of its own, such as piety, a cooldown, an exhausted berserker or a
    /// free square to blink to, isn't checked.
    OwnConditions,
    /// A monster that can attack is batty, such as the shard shrike: crawl has it flit away after
    /// it attacks (`_handle_battiness`), and the simulation has it stay and attack again.
    Batty,
    /// A monster that can attack keeps its range, such as the orb spider: crawl holds it at a
    /// firing position away from the player (`_decide_monster_firing_position`), and the
    /// simulation walks it up to fight.
    KeepsRange,
    /// A monster that can attack won't step toward the player while they watch it, such as the
    /// wandering mushroom (`_fungal_move_check`), and the simulation walks it up.
    WontMoveWhileWatched,
    /// A monster in view is a tentacle or vine, such as the snaplasher vine, which crawl moves
    /// with its parent (`mons_is_tentacle_or_tentacle_segment`): the simulation walks it on its
    /// own, so it may strike from where it is judged out of reach.
    MovesWithParent,
    /// A monster that can attack is one `handle_monster_move` acts or moves for with code of its
    /// own, such as a prism, a hellfire mortar or an orb of destruction, and the simulation has
    /// it walk, fight and cast.
    OwnActions,
    /// A monster in view swoops or flanks with its first attack (`_handle_swoop_or_flank`),
    /// moving about the player as it strikes, so it may strike from farther than the simulation
    /// judges it can.
    SwoopOrFlank,
    /// A monster that can attack has a special ability of its own, outside its spells
    /// (`mon_special_ability`), such as a slime creature's splitting or a foxfire's burst, which
    /// isn't played out.
    SpecialAbilities,
    /// A monster that can attack has a hit that tramples or drags the player, such as an
    /// elephant's (`melee_attack::mons_attack_effects`): the simulation keeps the player where
    /// they stand.
    MovesThePlayer,
    /// A monster that can attack has a hit that may blink it away from the player, such as a
    /// blink frog's (`AF_BLINK`, `melee_attack::mons_apply_attack_flavour`), and the simulation
    /// keeps it beside them.
    BlinksAway,
    /// A monster that can attack has a hit that may blink it away and take the player with it,
    /// such as a phantom's (`AF_BLINK_WITH`, `melee_attack::mons_apply_attack_flavour`): the
    /// simulation keeps both where they stand.
    BlinksWithThePlayer,
    /// A monster that can attack can't leave the lava or water it lives in, such as a lava snake
    /// (`monster_habitable_feat`), and the simulation walks it over the floor to the player.
    BoundToTerrain,
}

impl NotModelled {
    /// Whether it may let a monster strike that the simulation judges can't, so the Fact Sheet
    /// names it for any hostile in view rather than only those that can attack.
    fn strikes_from_out_of_reach(self) -> bool {
        matches!(self, Self::MovesWithParent | Self::SwoopOrFlank)
    }
}

/// What the simulation leaves out, whatever is in view.
const ALWAYS_NOT_MODELLED: [NotModelled; 4] = [
    NotModelled::AttackFlavours,
    NotModelled::Statuses,
    NotModelled::Paths,
    NotModelled::Awareness,
];

/// A fact the Advisor needs about this moment but doesn't have.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "gap", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Gap {
    /// The player's hit points haven't been reported.
    HitPoints,
    /// The player's AC, EV or SH haven't been reported.
    Defences,
    /// The game hasn't sent its map, so the monsters in view aren't known.
    MonstersInView,
    /// The game hasn't sent its map, so the level's paths aren't known.
    Map,
    /// Where the player stands hasn't been reported.
    Position,
    /// There is no Game Knowledge for the game's Crawl Version, so monsters can't be judged.
    GameKnowledge,
    /// A monster in view isn't in the Game Knowledge for the game's Crawl Version.
    MonsterKnowledge {
        /// The monster, as the game names it.
        monster: String,
    },
    /// The player hasn't opened the ability menu (`a`), so which of their abilities are Ways Out
    /// isn't known.
    AbilityMenu,
    /// The player hasn't opened the spell list (`I`), so which of their spells are Ways Out, and
    /// how often they fail, isn't known.
    SpellList,
}

impl FactSheet {
    /// What the engine makes of `visible`, what the Advisor remembers of the game and the
    /// character's `species`, as the game names it, with `knowledge` for the game's Crawl Version
    /// if the Advisor has it.
    #[must_use]
    pub fn compute(
        visible: &VisibleState,
        remembered: Option<&AccumulatedState>,
        species: Option<&str>,
        knowledge: Option<&Bundle>,
    ) -> Self {
        Self {
            threat: threat(visible, remembered, species, knowledge),
            ways_out: WaysOut::compute(visible, remembered, knowledge),
        }
    }
}

/// Monsters on these sides attack the player: crawl's `mons_att_wont_attack` (`mon-util.cc`)
/// spares only friends, good neutrals and marionettes, and strict neutrals don't hunt the player.
pub(crate) fn attacks_player(attitude: Attitude) -> bool {
    matches!(attitude, Attitude::Hostile | Attitude::Neutral)
}

fn threat(
    visible: &VisibleState,
    remembered: Option<&AccumulatedState>,
    species: Option<&str>,
    knowledge: Option<&Bundle>,
) -> Threat {
    let mut gaps = BTreeSet::new();
    let hp = visible.stats.hp;
    if hp.is_none() {
        gaps.insert(Gap::HitPoints);
    }
    let Some(monsters) = &visible.monsters else {
        gaps.insert(Gap::MonstersInView);
        return Threat::Unrated {
            gaps: gaps.into_iter().collect(),
        };
    };

    // The monsters that may attack, with their Game Knowledge.
    let mut hostile: Vec<(&VisibleMonster, &copilot_gk::Monster)> = Vec::new();
    for monster in monsters {
        let Some(attitude) = monster.attitude else {
            gaps.insert(Gap::GameKnowledge);
            continue;
        };
        if !attacks_player(attitude) {
            continue;
        }
        let Some(bundle) = knowledge else {
            gaps.insert(Gap::GameKnowledge);
            continue;
        };
        match monster.id.as_ref().and_then(|id| bundle.monster_by_id(id)) {
            Some(known) => hostile.push((monster, known)),
            None => {
                gaps.insert(Gap::MonsterKnowledge {
                    monster: monster.name.clone(),
                });
            }
        }
    }

    let stats = &visible.stats;
    let defences = match (stats.ac, stats.ev, stats.sh) {
        (Some(ac), Some(ev), Some(sh)) => Some((ac, ev, sh)),
        _ => None,
    };
    if !hostile.is_empty() {
        if defences.is_none() {
            gaps.insert(Gap::Defences);
        }
        if visible.position.is_none() {
            gaps.insert(Gap::Position);
        }
    }
    let (Some(hp), true) = (hp, gaps.is_empty()) else {
        return Threat::Unrated {
            gaps: gaps.into_iter().collect(),
        };
    };
    let (false, Some(bundle), Some(player), Some((ac, ev, sh))) =
        (hostile.is_empty(), knowledge, visible.position, defences)
    else {
        // Nothing in view attacks the player: a hostile would have needed the rest, or been a Gap.
        return Threat::Rated(Box::new(ThreatAssessment {
            level: ThreatLevel::Calm,
            hp,
            death: Chance::NONE,
            worst_hit: 0,
            hostiles_in_view: 0,
            acting: Vec::new(),
            gear: Vec::new(),
            lighting: knowledge
                .and_then(|bundle| lighting(visible, remembered, &bundle.lighting))
                .map(|(lighting, _)| lighting),
            resistances: None,
            not_modelled: Vec::new(),
        }));
    };

    let formulas = bundle.formulas;
    let worn_shield = remembered.and_then(|state| state.worn_shield.known().copied());
    let mut not_modelled = ALWAYS_NOT_MODELLED.to_vec();
    if sh > 0 && worn_shield.is_none() {
        not_modelled.push(NotModelled::WornShield);
    }
    if hostile
        .iter()
        .any(|(_, monster)| monster.spells == MonsterSpells::PerMonster)
    {
        not_modelled.push(NotModelled::SpellsPerMonster);
    }
    if hostile
        .iter()
        .any(|(_, monster)| monster.genus.0 == genera::DRACONIAN)
    {
        not_modelled.push(NotModelled::DraconianBreath);
    }
    if hostile
        .iter()
        .any(|(_, monster)| monster.id.0 == monster_types::MUTANT_BEAST)
    {
        not_modelled.push(NotModelled::MutantBeastFacets);
    }
    let (lighting, own_halo) =
        lighting(visible, remembered, &bundle.lighting).unwrap_or((Lighting::Plain, None));
    not_modelled.extend(own_halo);
    let defences = DisplayedDefences { ac, ev, sh };
    if defences.any_hides_a_fraction() {
        not_modelled.push(NotModelled::DefenceFractions);
    }
    let resists = PlayerResists::of(visible, remembered, species, bundle);
    let defender = Defender {
        hp,
        defences,
        lighting,
        shield_blocks: threat::shield_blocks_per_action(formulas, worn_shield.flatten()),
        resists: resists.resists,
    };
    let hostiles_in_view = u32::try_from(hostile.len()).unwrap_or(u32::MAX);
    // Each monster with the gear it is played with, and the monsters that can attack, each with
    // its row.
    let mut gear: Vec<GearPlayed> = Vec::new();
    let mut acting: Vec<ActingMonster> = Vec::new();
    let mut attackers: Vec<Attacker<'_>> = Vec::new();
    // The ways crawl moves a monster that the Fact Sheet names, in the order `NotModelled` lists
    // them.
    let mut moves: BTreeSet<NotModelled> = BTreeSet::new();
    // Whether a monster in view may fight with a weapon the Advisor hasn't read.
    let mut weapons_unread = false;
    for (seen, monster) in hostile {
        let distance = distance(player, seen.position);
        let attacker = Attacker::new(bundle, monster, distance);
        let carried = remembered
            .and_then(|state| state.gear_of(seen, bundle))
            .and_then(|read| match read {
                Remembered::Known(as_of) => Some((KnownOrStale::Known, as_of)),
                Remembered::Stale(as_of) => Some((KnownOrStale::Stale, as_of)),
                Remembered::Missing => None,
            });
        let (attacker, basis, unplayed) = match carried {
            Some((state, as_of)) => {
                let (attacker, unplayed) = attacker.carrying(bundle, &as_of.value);
                let basis = GearBasis::Carried {
                    state,
                    turn: as_of.turn,
                    source: as_of.source,
                    unlisted: as_of.value.unlisted,
                };
                (attacker, basis, unplayed)
            }
            None => {
                let basis = GearBasis::Possible {
                    may_shoot: monster.gear.may_shoot_or_zap(),
                };
                (attacker, basis, Vec::new())
            }
        };
        gear.push(GearPlayed {
            name: seen.name.clone(),
            position: seen.position,
            distance,
            basis,
            unplayed,
        });
        weapons_unread |= attacker.weapons_unread(formulas);
        let its_moves = moves_not_modelled(monster);
        if threat::can_attack(formulas, &attacker) {
            moves.extend(its_moves.iter().copied());
            acting.push(ActingMonster {
                name: seen.name.clone(),
                position: seen.position,
                distance: attacker.distance,
                casts: attacker
                    .spells
                    .iter()
                    .map(|cast| cast.spell.spell.name.clone())
                    .collect(),
                spells_not_modelled: spells_not_modelled(bundle, &attacker),
                moves_not_modelled: its_moves,
                shoots: attacker.shoots.clone(),
            });
            attackers.push(attacker);
        } else {
            moves.extend(
                its_moves
                    .into_iter()
                    .filter(|variant| variant.strikes_from_out_of_reach()),
            );
        }
    }
    not_modelled.extend(moves);
    if weapons_unread {
        // First, where it stood when it was listed whatever was in view.
        not_modelled.insert(0, NotModelled::MonsterEquipment);
    }
    if gear.iter().any(GearPlayed::guessed_shooter) {
        not_modelled.push(NotModelled::CarriedGear);
    }
    if acting
        .iter()
        .any(|monster| !monster.spells_not_modelled.is_empty())
    {
        not_modelled.push(NotModelled::OtherSpells);
    }
    if attackers.iter().any(|attacker| attacker.unported_wand) {
        not_modelled.push(NotModelled::OtherWands);
    }
    let needs = attackers
        .iter()
        .map(|attacker| attacker.resist_needs(formulas))
        .fold(Needs::NOTHING, Needs::and);
    if resists.unsettled(needs) {
        not_modelled.push(NotModelled::PlayerResistances);
    }
    if attackers.iter().any(|attacker| {
        let unsafe_reach = |reach: Reach, spell: &DamagingSpell<'_>| {
            reach.casts > reach.strikes || spell.zap.explodes
        };
        attacker
            .spells
            .iter()
            .any(|cast| unsafe_reach(cast.reach, &cast.spell))
            || attacker
                .wands
                .items()
                .any(|zap| unsafe_reach(zap.reach, &zap.spell))
    }) {
        not_modelled.push(NotModelled::CasterSafety);
    }
    let outcome = if attackers.is_empty() {
        Outcome {
            deaths: 0,
            trials: 0,
            worst_hit: 0,
        }
    } else {
        threat::simulate(formulas, defender, &attackers)
    };
    let death = Chance {
        times: outcome.deaths,
        trials: outcome.trials,
    };
    Threat::Rated(Box::new(ThreatAssessment {
        level: level(hp, death, outcome.worst_hit),
        hp,
        death,
        worst_hit: outcome.worst_hit,
        hostiles_in_view,
        acting,
        gear,
        lighting: Some(lighting),
        resistances: resists.applied,
        not_modelled,
    }))
}

/// The player's resistances, holiness and god as the simulation plays them, and what of them the
/// Advisor knows.
struct PlayerResists {
    /// As the simulation plays them.
    resists: Resists,
    /// The reading of the resistances applied, if they have been read.
    applied: Option<ResistancesApplied>,
    /// Whether the player's holiness and god are known, and applied.
    holiness_known: bool,
}

impl PlayerResists {
    /// The player as `visible` shows them and the Advisor remembers them, of `species`, judged
    /// through `bundle`. With no reading of their resistances, they resist nothing and are a
    /// living mortal of no good god. With one, their holiness is applied when their species is in
    /// the bundle and the game has reported their statuses and god; otherwise they are taken to
    /// be a living mortal of no good god.
    fn of(
        visible: &VisibleState,
        remembered: Option<&AccumulatedState>,
        species: Option<&str>,
        bundle: &Bundle,
    ) -> Self {
        let none = Self {
            resists: Resists::NONE,
            applied: None,
            holiness_known: false,
        };
        let (state, as_of) = match remembered.map(|state| &state.resistances) {
            Some(Remembered::Known(as_of)) => (KnownOrStale::Known, as_of),
            Some(Remembered::Stale(as_of)) => (KnownOrStale::Stale, as_of),
            Some(Remembered::Missing) | None => return none,
        };
        let Some(mut resists) = resists_read(&as_of.value) else {
            return none;
        };
        let holiness = holiness(visible, species, bundle);
        resists.holiness = holiness.unwrap_or(Holiness::MORTAL);
        Self {
            resists,
            applied: Some(ResistancesApplied {
                state,
                turn: as_of.turn,
                source: as_of.source.clone(),
                resistances: as_of.value.clone(),
            }),
            holiness_known: holiness.is_some(),
        }
    }

    /// Whether adjusting damage that `needs` this is beyond what the Advisor knows.
    fn unsettled(&self, needs: Needs) -> bool {
        needs.unshown
            || self.applied.is_none() && needs.any()
            || needs.holiness && !self.holiness_known
    }
}

/// The resistances `read` shows, as the engine takes them: `None` unless it shows each of those
/// that cut damage.
fn resists_read(read: &Resistances) -> Option<Resists> {
    // Immunity is drawn only for a resistance of 3 on a one-pip scale (`_resist_composer`).
    let level = |resistance: Option<Resistance>| match resistance? {
        Resistance::Pips(pips) => Some(i32::from(pips)),
        Resistance::Immune => Some(3),
    };
    Some(Resists {
        fire: level(read.fire)?,
        cold: level(read.cold)?,
        negative: level(read.negative)?,
        poison: level(read.poison)?,
        electricity: level(read.electricity)?,
        corrosion: level(read.corrosion)?,
        holiness: Holiness::MORTAL,
    })
}

/// The player's holiness, from their `species` and the form and petrification the status pane
/// lights in `visible`, and whether they worship a good god, judged through `bundle`: `None` when
/// the bundle doesn't know the species, or the game hasn't reported the statuses or the god.
fn holiness(visible: &VisibleState, species: Option<&str>, bundle: &Bundle) -> Option<Holiness> {
    let species = bundle.species_named(species?)?;
    let signs = &bundle.holiness;
    let lights: Vec<&str> = visible
        .statuses
        .as_ref()?
        .iter()
        .filter_map(|status| status.light.as_deref())
        .collect();
    let form = signs
        .forms
        .iter()
        .find(|form| lights.contains(&form.light.as_str()));
    let petrified = signs
        .petrified_lights
        .iter()
        .any(|light| lights.contains(&light.as_str()));
    let good_god = match visible.religion.as_ref()? {
        Religion::NoGod => false,
        Religion::God { name, .. } => signs.good_gods.contains(name),
    };
    Some(Holiness::of(
        bundle.formulas.resist_damage,
        signs,
        species,
        form,
        petrified,
        good_god,
    ))
}

/// How the light falls on the player, as crawl's `player::backlit(false)` and `player::umbra()`
/// (`player.cc:7850`, `player.cc:7864`) decide it from what the game shows, read through `signs`: a status that backlights them, or
/// the halo or umbra on their square. A halo backlights them only if they have none of their
/// own; with [`NotModelled::OwnHalo`] beside it when the Advisor can't tell. `None` when where
/// they stand isn't known.
fn lighting(
    visible: &VisibleState,
    remembered: Option<&AccumulatedState>,
    signs: &LightingSigns,
) -> Option<(Lighting, Option<NotModelled>)> {
    let square = visible.light_at_player()?;
    let backlit_by_status = visible.statuses.iter().flatten().any(|status| {
        status
            .light
            .as_ref()
            .is_some_and(|light| signs.backlighting_lights.contains(light))
    });
    if backlit_by_status {
        return Some((Lighting::Backlit, None));
    }
    Some(match square {
        SquareLight::Plain => (Lighting::Plain, None),
        SquareLight::Umbra => (Lighting::Umbra, None),
        SquareLight::Halo => match own_halo(visible, remembered, signs) {
            Some(true) => (Lighting::Plain, None),
            Some(false) => (Lighting::Backlit, None),
            None => (Lighting::Backlit, Some(NotModelled::OwnHalo)),
        },
    })
}

/// Whether the player has a halo of their own (`player::halo_radius`, `areas.cc:564`, of 0 or more): from their
/// god or a status, or from gear they carry that the character overview (`%`) last showed them
/// wearing or wielding. `None` when they carry such gear and the overview isn't Known, or the
/// inventory hasn't been sent.
fn own_halo(
    visible: &VisibleState,
    remembered: Option<&AccumulatedState>,
    signs: &LightingSigns,
) -> Option<bool> {
    let blessed = matches!(
        &visible.religion,
        Some(Religion::God { name, penance: false, .. }) if signs.halo_gods.contains(name)
    );
    let storm = visible.statuses.iter().flatten().any(|status| {
        status
            .text
            .as_ref()
            .is_some_and(|text| signs.halo_statuses.contains(text))
    });
    if blessed || storm {
        return Some(true);
    }
    let halo_gear: Vec<char> = visible
        .inventory
        .as_ref()?
        .iter()
        .filter(|item| {
            let name = item.name.to_lowercase();
            signs.halo_gear.iter().any(|gear| name.contains(gear))
        })
        .map(|item| item.letter)
        .collect();
    if halo_gear.is_empty() {
        return Some(false);
    }
    let equipped = remembered?.equipped.known()?;
    Some(halo_gear.iter().any(|letter| equipped.contains(letter)))
}

/// The spells of `attacker`'s book the simulation doesn't play out, by name: its Game
/// Knowledge's name, or crawl's identifier for a spell the bundle lacks.
fn spells_not_modelled(bundle: &Bundle, attacker: &Attacker<'_>) -> Vec<String> {
    let MonsterSpells::Book { spells } = &attacker.monster.spells else {
        return Vec::new();
    };
    spells
        .iter()
        .filter(|slot| {
            !attacker
                .spells
                .iter()
                .any(|cast| cast.spell.spell.id == slot.spell)
        })
        .map(|slot| {
            bundle
                .spell_by_id(&slot.spell)
                .map_or_else(|| slot.spell.0.clone(), |spell| spell.name.clone())
        })
        .collect()
}

/// The ways crawl moves a monster of `monster`'s kind, or acts for it, that the simulation
/// doesn't play out, in the order [`NotModelled`] lists them.
fn moves_not_modelled(monster: &copilot_gk::Monster) -> Vec<NotModelled> {
    let first_flavour = monster
        .attacks
        .first()
        .map(|attack| attack.flavour.0.as_str());
    let has_flavour = |flavours: &[&str]| {
        monster
            .attacks
            .iter()
            .any(|attack| flavours.contains(&attack.flavour.0.as_str()))
    };
    [
        (NotModelled::Batty, monster.batty),
        (NotModelled::KeepsRange, monster.keeps_range),
        (
            NotModelled::WontMoveWhileWatched,
            monster.wont_move_while_watched,
        ),
        (NotModelled::MovesWithParent, monster.tentacle),
        (NotModelled::OwnActions, monster.own_actions),
        (
            NotModelled::SwoopOrFlank,
            first_flavour.is_some_and(|flavour| {
                [attack_flavours::SWOOP, attack_flavours::FLANK].contains(&flavour)
            }),
        ),
        (NotModelled::SpecialAbilities, monster.special_ability),
        (
            NotModelled::MovesThePlayer,
            has_flavour(&[attack_flavours::TRAMPLE, attack_flavours::DRAG]),
        ),
        (
            NotModelled::BlinksAway,
            has_flavour(&[attack_flavours::BLINK]),
        ),
        (
            NotModelled::BlinksWithThePlayer,
            has_flavour(&[attack_flavours::BLINK_WITH]),
        ),
        // One that never moves never tries to leave.
        (
            NotModelled::BoundToTerrain,
            !monster.stationary && !monster.stands_on_floor(),
        ),
    ]
    .into_iter()
    .filter_map(|(variant, has)| has.then_some(variant))
    .collect()
}

/// The Threat Level for a chance of death and a worst-case hit against `hp`.
fn level(hp: i32, death: Chance, worst_hit: i32) -> ThreatLevel {
    if death.at_least_per_mille(LETHAL_DEATHS_PER_MILLE) || worst_hit >= hp {
        ThreatLevel::Lethal
    } else if death.at_least_per_mille(DANGEROUS_DEATHS_PER_MILLE)
        || worst_hit * 100 >= hp * DANGEROUS_WORST_HIT_PERCENT_OF_HP
    {
        ThreatLevel::Dangerous
    } else {
        ThreatLevel::Calm
    }
}

/// Squares between two positions, counting a diagonal step as one: crawl's `grid_distance`.
fn distance(a: Position, b: Position) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

#[cfg(test)]
mod tests {
    use copilot_gk::Library;

    use super::*;
    use crate::accumulated_state::AsOf;
    use crate::view_model::{Colour, Glyph};
    use crate::visible_state::Stats;

    fn chance(times: u32) -> Chance {
        Chance {
            times,
            trials: 10_000,
        }
    }

    #[test]
    fn a_five_percent_chance_of_death_is_lethal_and_just_under_it_dangerous() {
        assert_eq!(level(100, chance(500), 0), ThreatLevel::Lethal);
        assert_eq!(level(100, chance(499), 0), ThreatLevel::Dangerous);
    }

    #[test]
    fn a_half_percent_chance_of_death_is_dangerous_and_just_under_it_calm() {
        assert_eq!(level(100, chance(50), 0), ThreatLevel::Dangerous);
        assert_eq!(level(100, chance(49), 0), ThreatLevel::Calm);
    }

    #[test]
    fn nothing_able_to_attack_is_calm() {
        assert_eq!(level(1, Chance::NONE, 0), ThreatLevel::Calm);
    }

    #[test]
    fn a_worst_hit_of_all_hp_is_lethal_and_of_forty_percent_dangerous() {
        assert_eq!(level(50, chance(0), 50), ThreatLevel::Lethal);
        assert_eq!(level(50, chance(0), 49), ThreatLevel::Dangerous);
        assert_eq!(level(50, chance(0), 20), ThreatLevel::Dangerous);
        assert_eq!(level(50, chance(0), 19), ThreatLevel::Calm);
    }

    /// No defences to show, so no guessed fractions.
    const NO_DEFENCES: DisplayedDefences = DisplayedDefences {
        ac: 0,
        ev: 0,
        sh: 0,
    };

    /// The Threat Level with one hostile `name` in view, `distance` squares east of a player on
    /// 100 HP with no defences.
    fn assessed_with(name: &str, distance: i32) -> ThreatAssessment {
        assessed_defending(name, distance, NO_DEFENCES)
    }

    /// The Threat Level with one hostile `name` in view, `distance` squares east of a player on
    /// 100 HP who shows `defences`.
    fn assessed_defending(
        name: &str,
        distance: i32,
        defences: DisplayedDefences,
    ) -> ThreatAssessment {
        assessed_among(&[(name, distance, Attitude::Hostile)], defences)
    }

    /// The Threat Level with each `(name, distance, attitude)` in view, `distance` squares east
    /// of a player on 100 HP who shows `defences`.
    fn assessed_among(
        monsters: &[(&str, i32, Attitude)],
        defences: DisplayedDefences,
    ) -> ThreatAssessment {
        rated(&in_view(monsters, defences), None, None)
    }

    /// The shipped 0.34.1 bundle.
    fn bundle() -> &'static Bundle {
        static LIBRARY: std::sync::OnceLock<Library> = std::sync::OnceLock::new();
        LIBRARY
            .get_or_init(|| Library::shipped().unwrap())
            .for_version("0.34.1")
            .unwrap()
    }

    /// The Threat Level of `visible`, with what the Advisor remembers and the character's
    /// species.
    fn rated(
        visible: &VisibleState,
        remembered: Option<&AccumulatedState>,
        species: Option<&str>,
    ) -> ThreatAssessment {
        match FactSheet::compute(visible, remembered, species, Some(bundle())).threat {
            Threat::Rated(assessment) => *assessment,
            Threat::Unrated { gaps } => panic!("not rated: {gaps:?}"),
        }
    }

    /// Each `(name, distance, attitude)` in view, `distance` squares east of a player on 100 HP
    /// who shows `defences`.
    fn in_view(monsters: &[(&str, i32, Attitude)], defences: DisplayedDefences) -> VisibleState {
        let bundle = bundle();
        let player = Position { x: 10, y: 10 };
        let in_view = monsters
            .iter()
            .map(|&(name, distance, attitude)| {
                let monster = bundle.monsters.values().find(|m| m.name == name).unwrap();
                VisibleMonster {
                    name: name.to_owned(),
                    glyph: Glyph {
                        text: monster.glyph.clone(),
                        colour: Colour::White,
                    },
                    id: Some(monster.id.clone()),
                    position: Position {
                        x: player.x + distance,
                        y: player.y,
                    },
                    attitude: Some(attitude),
                    crawl_threat: None,
                    sighting: None,
                }
            })
            .collect();
        VisibleState {
            stats: Stats {
                hp: Some(100),
                ac: Some(defences.ac),
                ev: Some(defences.ev),
                sh: Some(defences.sh),
                ..Stats::default()
            },
            position: Some(player),
            monsters: Some(in_view),
            ..VisibleState::default()
        }
    }

    /// `name`, hostile, `distance` squares from a player with no defences who worships no god and
    /// has no statuses.
    fn facing(name: &str, distance: i32) -> VisibleState {
        VisibleState {
            religion: Some(Religion::NoGod),
            statuses: Some(Vec::new()),
            ..in_view(&[(name, distance, Attitude::Hostile)], NO_DEFENCES)
        }
    }

    /// Remembering resistances read off the character overview on turn 40, with `rFire` at
    /// `fire` pips and the rest at none, Known or, when `stale`, Stale.
    fn read_with_fire(fire: i8, stale: bool) -> AccumulatedState {
        let none = Some(Resistance::Pips(0));
        let as_of = AsOf {
            value: Resistances {
                fire: Some(Resistance::Pips(fire)),
                cold: none,
                negative: none,
                poison: none,
                electricity: none,
                corrosion: none,
                see_invisible: none,
                willpower: none,
            },
            turn: Some(40),
            source: Screen::CharacterOverview,
        };
        let mut remembered = AccumulatedState::default();
        remembered.resistances = if stale {
            Remembered::Stale(as_of)
        } else {
            Remembered::Known(as_of)
        };
        remembered
    }

    /// Whether the Fact Sheet names the player's resistances.
    fn names_resistances(assessment: &ThreatAssessment) -> bool {
        assessment
            .not_modelled
            .contains(&NotModelled::PlayerResistances)
    }

    #[test]
    fn unread_resistances_are_named_for_damage_they_would_cut_and_play_as_resisting_nothing() {
        let assessment = rated(&facing("fire giant", 2), None, Some("Human"));
        assert!(names_resistances(&assessment));
        assert_eq!(assessment.resistances, None);
    }

    #[test]
    fn unread_resistances_arent_named_for_damage_no_resistance_changes() {
        let assessment = rated(&facing("ogre", 1), None, Some("Human"));
        assert!(!names_resistances(&assessment), "{assessment:?}");
    }

    #[test]
    fn read_resistances_and_a_known_holiness_settle_an_elemental_casters_damage() {
        let remembered = read_with_fire(0, false);
        let assessment = rated(&facing("fire giant", 2), Some(&remembered), Some("Human"));
        assert!(!names_resistances(&assessment), "{assessment:?}");
        let applied = assessment.resistances.unwrap();
        assert_eq!(
            (applied.state, applied.turn),
            (KnownOrStale::Known, Some(40))
        );
    }

    #[test]
    fn steam_is_named_as_it_needs_a_resistance_the_overview_doesnt_show() {
        let remembered = read_with_fire(0, false);
        let assessment = rated(&facing("steam dragon", 2), Some(&remembered), Some("Human"));
        assert!(names_resistances(&assessment));
    }

    #[test]
    fn miasma_is_named_as_it_needs_a_resistance_the_overview_doesnt_show() {
        let remembered = read_with_fire(0, false);
        let assessment = rated(&facing("death drake", 2), Some(&remembered), Some("Human"));
        assert!(names_resistances(&assessment));
    }

    #[test]
    fn holy_fire_is_named_until_the_players_species_statuses_and_god_are_known() {
        let remembered = read_with_fire(0, false);
        let known = facing("pearl dragon", 2);
        assert!(!names_resistances(&rated(
            &known,
            Some(&remembered),
            Some("Human")
        )));
        let unknown = [
            (known.clone(), None),
            (known.clone(), Some("Not A Species")),
            (
                VisibleState {
                    statuses: None,
                    ..known.clone()
                },
                Some("Human"),
            ),
            (
                VisibleState {
                    religion: None,
                    ..known
                },
                Some("Human"),
            ),
        ];
        for (visible, species) in unknown {
            let assessment = rated(&visible, Some(&remembered), species);
            assert!(names_resistances(&assessment), "{species:?}");
        }
    }

    #[test]
    fn a_stale_reading_is_applied_and_shown_as_stale_with_its_turn() {
        let visible = facing("fire giant", 2);
        let unread = rated(&visible, None, Some("Human"));
        let stale = read_with_fire(3, true);
        let resisted = rated(&visible, Some(&stale), Some("Human"));
        let applied = resisted.resistances.as_ref().unwrap();
        assert_eq!(
            (applied.state, applied.turn),
            (KnownOrStale::Stale, Some(40))
        );
        assert_eq!(applied.resistances.fire, Some(Resistance::Pips(3)));
        assert!(
            resisted.worst_hit < unread.worst_hit,
            "{} against {}",
            resisted.worst_hit,
            unread.worst_hit
        );
    }

    #[test]
    fn a_hostile_mutant_beast_in_view_names_its_facets_once_however_many_are_in_view() {
        for in_view in [
            &[("mutant beast", 3, Attitude::Hostile)][..],
            &[
                ("mutant beast", 3, Attitude::Hostile),
                ("mutant beast", 5, Attitude::Hostile),
            ],
        ] {
            let assessment = assessed_among(in_view, NO_DEFENCES);
            let named = assessment
                .not_modelled
                .iter()
                .filter(|&&variant| variant == NotModelled::MutantBeastFacets)
                .count();
            assert_eq!(named, 1, "{in_view:?}");
        }
    }

    #[test]
    fn no_hostile_mutant_beast_in_view_names_no_facets() {
        for in_view in [
            &[("goblin", 3, Attitude::Hostile)][..],
            &[
                ("goblin", 3, Attitude::Hostile),
                ("mutant beast", 5, Attitude::Friendly),
            ],
        ] {
            let assessment = assessed_among(in_view, NO_DEFENCES);
            assert!(
                !assessment
                    .not_modelled
                    .contains(&NotModelled::MutantBeastFacets),
                "{in_view:?}"
            );
        }
    }

    #[test]
    fn the_guessed_fractions_are_named_whenever_a_displayed_defence_is_above_zero() {
        for (ac, ev, sh, named) in [
            (1, 0, 0, true),
            (0, 1, 0, true),
            (0, 0, 1, true),
            (6, 8, 4, true),
            (3, -2, 0, true),
            (0, 0, 0, false),
            (-1, -3, 0, false),
        ] {
            let assessment = assessed_defending("goblin", 1, DisplayedDefences { ac, ev, sh });
            assert_eq!(
                assessment
                    .not_modelled
                    .contains(&NotModelled::DefenceFractions),
                named,
                "AC {ac}, EV {ev}, SH {sh}"
            );
        }
    }

    /// Whether `variant` is a way crawl moves a monster, or acts for it, that the simulation
    /// doesn't play out. Matched exhaustively, so a new variant doesn't compile until it is
    /// sorted here.
    fn is_move(variant: NotModelled) -> bool {
        match variant {
            NotModelled::Batty
            | NotModelled::KeepsRange
            | NotModelled::WontMoveWhileWatched
            | NotModelled::MovesWithParent
            | NotModelled::OwnActions
            | NotModelled::SwoopOrFlank
            | NotModelled::SpecialAbilities
            | NotModelled::MovesThePlayer
            | NotModelled::BlinksAway
            | NotModelled::BlinksWithThePlayer
            | NotModelled::BoundToTerrain => true,
            NotModelled::MonsterEquipment
            | NotModelled::AttackFlavours
            | NotModelled::Statuses
            | NotModelled::Paths
            | NotModelled::Awareness
            | NotModelled::OtherSpells
            | NotModelled::SpellsPerMonster
            | NotModelled::DraconianBreath
            | NotModelled::MutantBeastFacets
            | NotModelled::PlayerResistances
            | NotModelled::CasterSafety
            | NotModelled::CarriedGear
            | NotModelled::OtherWands
            | NotModelled::WornShield
            | NotModelled::OwnHalo
            | NotModelled::DefenceFractions
            | NotModelled::PlayerSpeed
            | NotModelled::SwimmingAndFlying
            | NotModelled::UnexploredSquares
            | NotModelled::HitsOnTheWay
            | NotModelled::OwnConditions => false,
        }
    }

    /// What crawl does with a monster in view, beside the player, that the simulation doesn't
    /// play out: on the Fact Sheet, and on its row.
    fn moves_beside_the_player(name: &str) -> (Vec<NotModelled>, Vec<NotModelled>) {
        let assessment = assessed_with(name, 1);
        let [acting] = assessment.acting.as_slice() else {
            panic!("{name} beside the player doesn't act");
        };
        let moves = assessment
            .not_modelled
            .iter()
            .copied()
            .filter(|&variant| is_move(variant))
            .collect();
        (moves, acting.moves_not_modelled.clone())
    }

    #[test]
    fn a_monster_that_moves_as_crawl_moves_it_in_a_way_not_modelled_is_named_with_its_row() {
        for (name, variant) in [
            ("shard shrike", NotModelled::Batty),
            ("orb spider", NotModelled::KeepsRange),
            ("wandering mushroom", NotModelled::WontMoveWhileWatched),
            ("snaplasher vine", NotModelled::MovesWithParent),
            ("jiangshi", NotModelled::OwnActions),
            ("tengu warrior", NotModelled::SwoopOrFlank),
            ("slime creature", NotModelled::SpecialAbilities),
            ("elephant", NotModelled::MovesThePlayer),
            ("blink frog", NotModelled::BlinksAway),
            ("phantom", NotModelled::BlinksWithThePlayer),
            ("lava snake", NotModelled::BoundToTerrain),
        ] {
            assert_eq!(
                moves_beside_the_player(name),
                (vec![variant], vec![variant]),
                "{name}"
            );
        }
    }

    #[test]
    fn a_goblin_walks_up_as_crawl_walks_it() {
        assert_eq!(moves_beside_the_player("goblin"), (vec![], vec![]));
    }

    #[test]
    fn a_monster_that_may_strike_from_out_of_reach_is_named_while_it_cant_attack() {
        for (name, variant) in [
            ("snaplasher vine", NotModelled::MovesWithParent),
            ("sun moth", NotModelled::SwoopOrFlank),
        ] {
            let assessment = assessed_with(name, 7);
            assert!(assessment.acting.is_empty(), "{name} acts from 7 squares");
            assert!(assessment.not_modelled.contains(&variant), "{name}");
        }
    }

    #[test]
    fn a_monster_that_cant_attack_and_cant_strike_from_out_of_reach_isnt_named() {
        let assessment = assessed_with("elephant", 7);
        assert!(
            assessment.acting.is_empty(),
            "the elephant acts from 7 squares"
        );
        assert!(
            !assessment
                .not_modelled
                .contains(&NotModelled::MovesThePlayer)
        );
    }

    /// The Not Modelled ledger: one entry for each thing the engine leaves out.
    const LEDGER: &str = include_str!("../../../docs/not-modelled.md");

    /// The field of a ledger entry that says how the Fact Sheet surfaces it.
    const SURFACED_AS: &str = "- **Surfaced as:** ";

    /// The variants named, checked by an exhaustive match so that a new variant doesn't compile
    /// until it is listed here too.
    macro_rules! every_not_modelled {
        ($($variant:ident),+ $(,)?) => {{
            fn _exhaustive(not_modelled: NotModelled) {
                match not_modelled {
                    $(NotModelled::$variant)|+ => {}
                }
            }
            [$(NotModelled::$variant),+]
        }};
    }

    fn every_variant_name() -> BTreeSet<String> {
        every_not_modelled![
            MonsterEquipment,
            AttackFlavours,
            Statuses,
            Paths,
            Awareness,
            OtherSpells,
            SpellsPerMonster,
            DraconianBreath,
            MutantBeastFacets,
            PlayerResistances,
            CasterSafety,
            CarriedGear,
            OtherWands,
            WornShield,
            OwnHalo,
            DefenceFractions,
            PlayerSpeed,
            SwimmingAndFlying,
            UnexploredSquares,
            HitsOnTheWay,
            OwnConditions,
            Batty,
            KeepsRange,
            WontMoveWhileWatched,
            MovesWithParent,
            OwnActions,
            SwoopOrFlank,
            SpecialAbilities,
            MovesThePlayer,
            BlinksAway,
            BlinksWithThePlayer,
            BoundToTerrain,
        ]
        .into_iter()
        .map(|variant| {
            serde_json::to_value(variant)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
    }

    /// The variant each ledger entry names, by its serialized name, from the start of its
    /// "Surfaced as" field. Fails unless every entry has that field, and an entry without a
    /// variant says it is an accepted simplification or harness only.
    fn ledger_variant_names() -> Vec<&'static str> {
        let fields: Vec<&str> = LEDGER
            .lines()
            .filter_map(|line| line.strip_prefix(SURFACED_AS))
            .collect();
        let entries = LEDGER
            .lines()
            .filter(|line| line.starts_with("### "))
            .count();
        assert_eq!(
            fields.len(),
            entries,
            "each entry has one Surfaced as field"
        );
        fields
            .into_iter()
            .filter_map(|field| match field.strip_prefix('`') {
                Some(named) => named.split('`').next(),
                None => {
                    assert!(
                        field.starts_with("accepted simplification")
                            || field.starts_with("harness only"),
                        "an entry without a variant says why: {field}"
                    );
                    None
                }
            })
            .collect()
    }

    #[test]
    fn every_not_modelled_variant_has_a_ledger_entry() {
        let in_ledger: BTreeSet<&str> = ledger_variant_names().into_iter().collect();
        for name in every_variant_name() {
            assert!(
                in_ledger.contains(name.as_str()),
                "docs/not-modelled.md has no entry for `{name}`"
            );
        }
    }

    #[test]
    fn every_variant_the_ledger_names_exists_once() {
        let variants = every_variant_name();
        let mut seen = BTreeSet::new();
        for name in ledger_variant_names() {
            assert!(
                variants.contains(name),
                "docs/not-modelled.md names `{name}`, which isn't a NotModelled variant"
            );
            assert!(
                seen.insert(name),
                "docs/not-modelled.md names `{name}` twice"
            );
        }
    }
}

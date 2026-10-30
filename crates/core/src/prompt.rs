//! The renderer: one deterministic renderer turns a moment's Fact Sheet into the compact labelled
//! text an LLM is asked about (#16), in a fixed section order with ✓ and ✕ marks and Candidate
//! Action ids. A Stale fact keeps its `~` and the turn it is as of, and a Missing one reads
//! `unknown`. Nothing the player wrote reaches it: no messages, no character name, and a monster
//! named for a player, such as their ghost, by its kind.

use serde::Serialize;

use crate::candidate_action::{Candidate, CandidateAction, Candidates};
use crate::crawl_ids::monster_types;
use crate::detectors::shouted;
use crate::fact_sheet::{FactSheet, KnownOrStale, ResistancesApplied, ThreatAssessment};
use crate::fight_outcome::Fight;
use crate::gaps::Reader;
use crate::screen::Resistance;
use crate::suggested_action::SuggestedAction;
use crate::view_model::{WayOutRow, capitalised};
use crate::visible_state::VisibleState;
use crate::ways_out::Escape;

/// The most words a Tactical reason may have. From the spec (#24, "Length"), to be tuned on the
/// corpus.
pub(crate) const TACTICAL_WORDS: u32 = 25;

/// The most a Tactical moment tail should cost, in tokens: past it, the tail is trimmed. From the
/// spec (#24, "Tail budgets").
const TACTICAL_TAIL_TOKENS: usize = 900;

/// Characters to a token, as the renderer estimates a tail's cost: a rough figure for labelled
/// English, not a tokenizer's count, so a tail near its budget may be a little over or under it.
const CHARS_PER_TOKEN: usize = 4;

/// The Candidate Actions a tail over budget keeps, the Suggested Action first. From the spec
/// (#24, "Tail budgets").
const CANDIDATES_KEPT: usize = 6;

/// What the Advisor asks an LLM, rendered: the static prefix, the same for every call of one type
/// on one Crawl Version, which providers cache; and the moment tail, never cached. Strategic
/// calls will put a game block of the build and the Run Plan between them; a Tactical call has
/// none, and no Run Plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    /// The role, the closed-world rules and the answer's shape.
    pub prefix: String,
    /// This moment's facts.
    pub moment: String,
}

/// A Tactical moment, as the renderer reads it.
pub(crate) struct TacticalMoment<'a> {
    /// The followed game's Crawl Version, when it has said.
    pub(crate) crawl_version: Option<&'a str>,
    /// The character's species, as the game names it.
    pub(crate) species: Option<&'a str>,
    pub(crate) visible: &'a VisibleState,
    pub(crate) fact_sheet: &'a FactSheet,
    pub(crate) suggested: &'a SuggestedAction,
    pub(crate) candidates: &'a Candidates,
}

/// What a tail over budget leaves out, in the order the spec (#24) trims: each only once the
/// one before isn't enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Trim {
    /// Nothing.
    None,
    /// The hostiles in view that can't act before the player's next action, folded into a count.
    IdleMonsters,
    /// And the Candidate Actions past the first few.
    Candidates,
    /// And the Ways Out that can't be used now, collapsed to one line.
    UnusableWaysOut,
}

impl TacticalMoment<'_> {
    /// The prompt, and the Candidate Actions its tail lists.
    pub(crate) fn render(&self) -> (Prompt, Vec<Candidate>) {
        self.render_within(TACTICAL_TAIL_TOKENS)
    }

    /// The prompt with its tail trimmed to about `tokens`, and the Candidate Actions it lists.
    fn render_within(&self, tokens: usize) -> (Prompt, Vec<Candidate>) {
        let mut rendered = self.tail(Trim::None);
        for trim in [Trim::IdleMonsters, Trim::Candidates, Trim::UnusableWaysOut] {
            if rendered.0.chars().count() <= tokens.saturating_mul(CHARS_PER_TOKEN) {
                break;
            }
            rendered = self.tail(trim);
        }
        let (moment, listed) = rendered;
        let prompt = Prompt {
            prefix: tactical_prefix(self.crawl_version),
            moment: self.scrubbed(moment),
        };
        (prompt, listed.into_iter().cloned().collect())
    }

    /// The moment tail, trimmed by `trim` and what comes before it.
    fn tail(&self, trim: Trim) -> (String, Vec<&Candidate>) {
        let mut lines = vec![self.you(), self.statuses(), self.resistances()];
        if let Some(assessment) = self.fact_sheet.threat.assessment() {
            lines.push(threat_line(assessment));
            lines.extend(monsters(assessment, trim >= Trim::IdleMonsters));
        } else {
            lines.push("Threat: unknown".to_owned());
        }
        lines.extend(self.fight());
        lines.extend(self.ways_out(trim >= Trim::UnusableWaysOut));
        let shown: Vec<&Candidate> = if trim >= Trim::Candidates {
            self.candidates.0.iter().take(CANDIDATES_KEPT).collect()
        } else {
            self.candidates.0.iter().collect()
        };
        lines.push("Candidate Actions:".to_owned());
        lines.extend(shown.iter().map(|candidate| self.candidate(candidate)));
        lines.push(self.suggested_line(&shown));
        lines.extend(self.gaps());
        if let Some(note) = trimmed_note(trim) {
            lines.push(note);
        }
        (lines.join("\n"), shown)
    }

    /// The character: species, level, place, turn and the numbers of the status pane.
    fn you(&self) -> String {
        let stats = &self.visible.stats;
        let of = |now: Option<i32>, max: Option<i32>| match (now, max) {
            (Some(now), Some(max)) => format!("{now}/{max}"),
            (Some(now), None) => format!("{now}/unknown"),
            _ => "unknown".to_owned(),
        };
        let number =
            |value: Option<i32>| value.map_or_else(|| "unknown".to_owned(), |n| n.to_string());
        let mut parts = vec![format!(
            "You: {}",
            self.species
                .map_or_else(|| "unknown species".to_owned(), capitalised)
        )];
        if let Some(xl) = stats.xl {
            parts.push(format!("XL {xl}"));
        }
        if let Some(place) = &self.visible.place {
            parts.push(format!("on {}", place.level_name()));
        }
        if let Some(turn) = self.visible.turn {
            parts.push(format!("turn {turn}"));
        }
        parts.push(format!("HP {}", of(stats.hp, stats.hp_max)));
        parts.push(format!("MP {}", of(stats.mp, stats.mp_max)));
        parts.push(format!("AC {}", number(stats.ac)));
        parts.push(format!("EV {}", number(stats.ev)));
        parts.push(format!("SH {}", number(stats.sh)));
        parts.join(" · ")
    }

    /// The statuses the status pane lists, in words where it has them, such as `corroded (-4)`.
    fn statuses(&self) -> String {
        let Some(statuses) = &self.visible.statuses else {
            return "Statuses: unknown".to_owned();
        };
        let named: Vec<&str> = statuses
            .iter()
            .filter_map(|status| status.text.as_deref().or(status.light.as_deref()))
            .collect();
        if named.is_empty() {
            "Statuses: none".to_owned()
        } else {
            format!("Statuses: {}", named.join(", "))
        }
    }

    /// The player's resistances the Threat Level was judged with.
    fn resistances(&self) -> String {
        self.fact_sheet
            .threat
            .assessment()
            .and_then(|assessment| assessment.resistances.as_ref())
            .map_or_else(|| "Resistances: unknown".to_owned(), resistances_line)
    }

    /// The Attack Plans the Fight Outcome played out, the best first.
    fn fight(&self) -> Vec<String> {
        let fight = &self.fact_sheet.fight;
        match fight {
            Fight::NoFight => Vec::new(),
            Fight::Judged(outcome) => std::iter::once("Attack Plans:".to_owned())
                .chain(
                    outcome
                        .plans
                        .iter()
                        .map(|plan| format!("- {}", plan.summary(outcome.hp))),
                )
                .collect(),
            Fight::TooMany { hostiles } => vec![format!(
                "Attack Plans: unknown, as {hostiles} hostiles are more than the Advisor plays out"
            )],
            Fight::Unjudged { .. } => {
                vec!["Attack Plans: unknown".to_owned()]
            }
        }
    }

    /// The ground the player stands on and their Ways Out, as the panel lists them, those that
    /// can't be used `collapsed` to one line.
    fn ways_out(&self, collapsed: bool) -> Vec<String> {
        let list = self.fact_sheet.ways_out.list(Reader::Player);
        let mut lines = Vec::new();
        if let Some(ground) = list.ground {
            lines.push(format!("Ground: {ground}"));
        }
        lines.push("Ways Out:".to_owned());
        let unusable = list
            .rows
            .iter()
            .filter(|row| row.usable == Some(false))
            .count();
        lines.extend(
            list.rows
                .iter()
                .filter(|row| !(collapsed && row.usable == Some(false)))
                .map(way_out_line),
        );
        if collapsed && unusable > 0 {
            lines.push(format!("✕ {unusable} more can't be used now"));
        }
        if list.rows.is_empty() {
            lines.push("none".to_owned());
        }
        lines
    }

    /// A Candidate Action's line: its id, then what it is.
    fn candidate(&self, candidate: &Candidate) -> String {
        let value = candidate
            .action
            .value(&self.fact_sheet.fight, Reader::Player)
            .text;
        let what = match &candidate.action {
            CandidateAction::WayOut(Escape::Item { .. }) => {
                format!("Item · {value}")
            }
            CandidateAction::WayOut(_) => value,
            CandidateAction::Corridor(_) => format!("Hold · {value}"),
            CandidateAction::Fight { .. } => format!("Fight · {value}"),
        };
        format!("{} {what}", candidate.id)
    }

    /// Which Candidate Action the rules picked, by which rule, and at fight carefully the Way Out
    /// for should it turn LETHAL.
    fn suggested_line(&self, shown: &[&Candidate]) -> String {
        let id = shown
            .iter()
            .find(|candidate| candidate.suggested)
            .map_or("unlisted", |candidate| candidate.id.as_str());
        let card = self.suggested.card(&self.fact_sheet.fight, Reader::Player);
        match card.if_lethal {
            Some(if_lethal) => format!(
                "Suggested Action: {id}, by the rule \"{}\". If it turns LETHAL: {}",
                card.rule, if_lethal.text
            ),
            None => format!("Suggested Action: {id}, by the rule \"{}\"", card.rule),
        }
    }

    /// The Gaps that affected this moment, each with the screen that fills it.
    fn gaps(&self) -> Vec<String> {
        let mut sentences: Vec<String> = Vec::new();
        for gap in &self.fact_sheet.gaps {
            let sentence = gap.sentence(Reader::Player);
            if !sentences.contains(&sentence) {
                sentences.push(sentence);
            }
        }
        if sentences.is_empty() {
            return vec!["Gaps: none".to_owned()];
        }
        std::iter::once("Gaps:".to_owned())
            .chain(
                sentences
                    .into_iter()
                    .map(|sentence| format!("- {sentence}")),
            )
            .collect()
    }

    /// `text` with each monster in view named for a player, such as their ghost, named by its
    /// kind instead.
    fn scrubbed(&self, text: String) -> String {
        self.visible
            .monsters
            .iter()
            .flatten()
            .filter_map(|monster| {
                let kind = match monster.id.as_ref()?.0.as_str() {
                    monster_types::PLAYER_GHOST => "player ghost",
                    monster_types::PLAYER_ILLUSION => "player illusion",
                    _ => return None,
                };
                Some((monster.name.as_str(), kind))
            })
            .fold(text, |text, (name, kind)| {
                text.replace(name, kind)
                    .replace(&capitalised(name), &capitalised(kind))
            })
    }
}

/// The static prefix of a Tactical call on `crawl_version`: the role, the closed-world rules of
/// ADR 0003 and the answer's shape.
fn tactical_prefix(crawl_version: Option<&str>) -> String {
    let game = match crawl_version {
        Some(version) => format!("Dungeon Crawl Stone Soup {version}"),
        None => "Dungeon Crawl Stone Soup".to_owned(),
    };
    format!(
        "You are the Advisor beside a game of {game} that the player is playing. The moment below \
         is dangerous. The game engine has listed what the player can do now as Candidate \
         Actions, each with an id, and its own rules have picked one: the Suggested Action.\n\
         \n\
         Rules:\n\
         - Pick exactly one Candidate Action, by its id. Never suggest anything that isn't \
         listed.\n\
         - Quote only numbers and names that appear in the moment. Never work out new numbers.\n\
         - Anything not listed is unknown. A value with ~ is out of date, as of the turn given; \
         \"unknown\" means the Advisor can't see it.\n\
         - Keep the Suggested Action unless the facts show another is clearly better.\n\
         - Give the reason in one or two short sentences, at most {TACTICAL_WORDS} words, \
         without ids. Lead with the number in the moment that makes your pick the better one, \
         such as the death chance, the worst hit against HP, or the odds.\n\
         - Never give the Suggested Action or the engine's rules as a reason; say what in the \
         moment makes the pick right.\n\
         \n\
         Answer with JSON only: {{\"pick\": \"<Candidate Action id>\", \"reason\": \"<at most \
         {TACTICAL_WORDS} words>\"}}"
    )
}

/// The Threat Level and the numbers it was judged from.
fn threat_line(assessment: &ThreatAssessment) -> String {
    let percent = assessment.death.percent();
    let death = if percent < 0.1 {
        "under 0.1%".to_owned()
    } else {
        format!("{percent:.1}%")
    };
    let mut parts = vec![
        format!("Threat: {}", shouted(assessment.level)),
        format!("death before your next action {death}"),
        format!("worst hit {} of {} HP", assessment.worst_hit, assessment.hp),
        format!("hostiles in view {}", assessment.hostiles_in_view),
    ];
    if let Some(win) = assessment.best_plan_win {
        parts.push(format!("best plan wins ~{:.0}%", win.percent()));
    }
    parts.join(" · ")
}

/// The hostiles in view: those that can act before the player's next action, then the rest,
/// `folded` into a count.
fn monsters(assessment: &ThreatAssessment, folded: bool) -> Vec<String> {
    let mut lines = vec!["Can act before your next action:".to_owned()];
    if assessment.acting.is_empty() {
        lines.push("none".to_owned());
    }
    for monster in &assessment.acting {
        let mut parts = vec![format!("- {}", monster.name), away(monster.distance)];
        if !monster.casts.is_empty() {
            parts.push(format!("casts {}", monster.casts.join(", ")));
        }
        if !monster.shoots.is_empty() {
            parts.push(format!("shoots {}", monster.shoots.join(", ")));
        }
        if assessment
            .gear
            .iter()
            .any(|gear| gear.position == monster.position && gear.guessed_shooter())
        {
            parts.push("gear guessed from its kind".to_owned());
        }
        lines.push(parts.join(" · "));
    }
    let idle: Vec<_> = assessment
        .gear
        .iter()
        .filter(|gear| {
            !assessment
                .acting
                .iter()
                .any(|acting| acting.position == gear.position)
        })
        .collect();
    if folded {
        if !idle.is_empty() {
            lines.push(format!(
                "{} more hostiles in view can't act before your next action",
                idle.len()
            ));
        }
    } else if !idle.is_empty() {
        lines.push("In view, can't act before your next action:".to_owned());
        lines.extend(
            idle.iter()
                .map(|gear| format!("- {} · {}", gear.name, away(gear.distance))),
        );
    }
    lines
}

/// How far a monster is, such as `3 squares away`.
fn away(distance: i32) -> String {
    match distance {
        1 => "1 square away".to_owned(),
        distance => format!("{distance} squares away"),
    }
}

/// A Way Out's row as a line: ✓ or ✕, or `?` for one a Gap hides, its name and fact, and why
/// it can't be used.
fn way_out_line(row: &WayOutRow) -> String {
    let mark = match row.usable {
        Some(true) => "✓",
        Some(false) => "✕",
        None => "?",
    };
    let mut line = format!("{mark} {}", row.name);
    if let Some(detail) = &row.detail {
        line.push_str(&format!(" · {}", detail.text));
    }
    if let Some(why_not) = &row.why_not {
        line.push_str(&format!(" — {}", why_not.text));
    }
    line
}

/// The resistances applied, `~` and as of the turn read when Stale.
fn resistances_line(applied: &ResistancesApplied) -> String {
    let resistances = &applied.resistances;
    let named = [
        ("rF", resistances.fire),
        ("rC", resistances.cold),
        ("rN", resistances.negative),
        ("rPois", resistances.poison),
        ("rElec", resistances.electricity),
        ("rCorr", resistances.corrosion),
        ("SInv", resistances.see_invisible),
        ("Will", resistances.willpower),
    ];
    let said: Vec<String> = named
        .into_iter()
        .filter_map(|(name, resistance)| match resistance? {
            Resistance::Immune => Some(format!("{name} immune")),
            Resistance::Pips(0) => None,
            Resistance::Pips(pips) if pips > 0 => Some(format!(
                "{name}{}",
                "+".repeat(usize::from(pips.unsigned_abs()))
            )),
            Resistance::Pips(pips) => Some(format!(
                "{name}{}",
                "-".repeat(usize::from(pips.unsigned_abs()))
            )),
        })
        .collect();
    let said = if said.is_empty() {
        "none".to_owned()
    } else {
        said.join(" ")
    };
    match (applied.state, applied.turn) {
        (KnownOrStale::Known, _) => format!("Resistances: {said}"),
        (KnownOrStale::Stale, Some(turn)) => format!("Resistances: ~{said} as of turn {turn}"),
        (KnownOrStale::Stale, None) => format!("Resistances: ~{said}"),
    }
}

/// The note saying what `trim` left out, if anything.
fn trimmed_note(trim: Trim) -> Option<String> {
    let mut left_out = Vec::new();
    if trim >= Trim::IdleMonsters {
        left_out.push("hostiles that can't act folded into a count".to_owned());
    }
    if trim >= Trim::Candidates {
        left_out.push(format!(
            "only the first {CANDIDATES_KEPT} Candidate Actions listed"
        ));
    }
    if trim >= Trim::UnusableWaysOut {
        left_out.push("Ways Out that can't be used collapsed to one line".to_owned());
    }
    (!left_out.is_empty()).then(|| format!("Trimmed to fit: {}", left_out.join("; ")))
}

#[cfg(test)]
mod tests {
    use copilot_gk::Relief;

    use super::*;
    use crate::fact_sheet::{ActingMonster, Chance, GearBasis, GearPlayed, Threat, ThreatLevel};
    use crate::visible_state::Position;
    use crate::ways_out::{Blocker, Usable, WayOut, WaysOut};

    fn gear(name: &str, x: i32) -> GearPlayed {
        GearPlayed {
            name: name.to_owned(),
            position: Position { x, y: 0 },
            distance: x,
            basis: GearBasis::Possible { may_shoot: false },
            unplayed: Vec::new(),
        }
    }

    fn scroll(letter: char, usable: Usable) -> WayOut {
        WayOut {
            option: Escape::Item {
                letter,
                name: format!("scroll of blinking ({letter})"),
                kind_name: format!("blinking {letter}"),
                quantity: 1,
                effect: Relief::Escapes,
            },
            usable,
        }
    }

    /// A LETHAL moment with an ogre beside the player, many jackals further off, and many
    /// scrolls, some of which can't be read.
    fn crowded() -> FactSheet {
        let unusable = || Usable::No {
            because: vec![Blocker::Useless],
        };
        FactSheet {
            threat: Threat::Rated(Box::new(ThreatAssessment {
                level: ThreatLevel::Lethal,
                hp: 10,
                death: Chance {
                    times: 300,
                    trials: 1000,
                },
                worst_hit: 30,
                hostiles_in_view: 11,
                best_plan_win: None,
                acting: vec![ActingMonster {
                    name: "ogre".to_owned(),
                    position: Position { x: 1, y: 0 },
                    distance: 1,
                    casts: Vec::new(),
                    spells_not_modelled: Vec::new(),
                    moves_not_modelled: Vec::new(),
                    shoots: Vec::new(),
                }],
                gear: std::iter::once(gear("ogre", 1))
                    .chain((2..12).map(|x| gear("jackal", x)))
                    .collect(),
                lighting: None,
                resistances: None,
                not_modelled: Vec::new(),
                gaps: Vec::new(),
            })),
            ways_out: WaysOut {
                options: ('a'..='h')
                    .map(|letter| scroll(letter, Usable::Yes))
                    .chain(('i'..='n').map(|letter| scroll(letter, unusable())))
                    .collect(),
                ..WaysOut::default()
            },
            fight: Fight::TooMany { hostiles: 11 },
            exceptional: Vec::new(),
            dangerous_statuses: Vec::new(),
            gaps: Vec::new(),
        }
    }

    /// The moment tail of `fact_sheet` rendered within `tokens`.
    fn tail_within(fact_sheet: &FactSheet, tokens: usize) -> (String, usize) {
        tail_seeing(fact_sheet, &VisibleState::default(), tokens)
    }

    /// The moment tail of `fact_sheet`, with `visible` in view, rendered within `tokens`.
    fn tail_seeing(
        fact_sheet: &FactSheet,
        visible: &VisibleState,
        tokens: usize,
    ) -> (String, usize) {
        let suggested = SuggestedAction::steady(None, fact_sheet, Some(20)).unwrap();
        let candidates = Candidates::at(fact_sheet, suggested.action());
        let moment = TacticalMoment {
            crawl_version: Some("0.34.1"),
            species: Some("Minotaur"),
            visible,
            fact_sheet,
            suggested: &suggested,
            candidates: &candidates,
        };
        let (prompt, listed) = moment.render_within(tokens);
        (prompt.moment, listed.len())
    }

    #[test]
    fn a_tail_within_its_budget_is_whole() {
        let (tail, listed) = tail_within(&crowded(), usize::MAX);

        assert_eq!(listed, 9, "eight scrolls and a fight");
        assert_eq!(tail.matches("- jackal").count(), 10);
        assert!(!tail.contains("Trimmed"));
    }

    #[test]
    fn a_tail_over_its_budget_folds_idle_monsters_first_and_trims_no_further_than_it_must() {
        let fact_sheet = crowded();
        let (whole, _) = tail_within(&fact_sheet, usize::MAX);
        let (folded, listed) =
            tail_within(&fact_sheet, whole.chars().count() / CHARS_PER_TOKEN - 1);

        assert!(folded.contains("10 more hostiles in view can't act before your next action"));
        assert!(folded.contains("- ogre · 1 square away"));
        assert_eq!(listed, 9);
        assert!(folded.ends_with("Trimmed to fit: hostiles that can't act folded into a count"));
    }

    #[test]
    fn a_tail_still_over_lists_the_first_candidate_actions_then_collapses_unusable_ways_out() {
        let (tail, listed) = tail_within(&crowded(), 0);

        assert_eq!(listed, CANDIDATES_KEPT);
        assert!(tail.contains("Suggested Action: a1"));
        assert!(tail.contains("✕ 6 more can't be used now"));
        assert!(!tail.contains("✕ Blinking"));
        assert!(tail.ends_with(
            "Trimmed to fit: hostiles that can't act folded into a count; only the first 6 \
             Candidate Actions listed; Ways Out that can't be used collapsed to one line"
        ));
    }

    #[test]
    fn a_monster_named_for_a_player_is_named_by_its_kind() {
        let mut fact_sheet = crowded();
        if let Threat::Rated(assessment) = &mut fact_sheet.threat {
            assessment.acting[0].name = "Mira's ghost".to_owned();
            assessment.gear[0].name = "Mira's ghost".to_owned();
        }
        let visible = VisibleState {
            monsters: Some(vec![crate::visible_state::VisibleMonster {
                name: "Mira's ghost".to_owned(),
                glyph: crate::view_model::Glyph {
                    text: "W".to_owned(),
                    colour: crate::view_model::Colour::White,
                },
                id: Some(copilot_gk::EnumId(monster_types::PLAYER_GHOST.to_owned())),
                position: Position { x: 1, y: 0 },
                attitude: None,
                crawl_threat: None,
                sighting: None,
            }]),
            ..VisibleState::default()
        };

        let (tail, _) = tail_seeing(&fact_sheet, &visible, usize::MAX);

        assert!(!tail.contains("Mira"), "{tail}");
        assert!(tail.contains("- player ghost · 1 square away"), "{tail}");
    }
}

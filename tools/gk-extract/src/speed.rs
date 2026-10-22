//! What the game shows that tells how fast the player moves: the status lights, mutations, banes,
//! gods and gear `player_speed` and `player_movement_speed` (`player.cc`) read. Which lights mean
//! what, and which mutations and banes count, are ported by hand for each release series and
//! checked against crawl's source; every other light the status pane may show is read from it,
//! so a light the bundle doesn't list is one the Advisor can't classify.

use std::collections::BTreeSet;

use copilot_gk::{
    EnumId, FormBody, GodRank, SpeedBane, SpeedLight, SpeedMutation, SpeedSigns, SpeedStatus,
};

use crate::cpp::{Source, Token};
use crate::{Crawl, ExtractError, Shown};

/// What a release series reads of the player's speed, each checked against crawl's source.
pub struct SpeedRules {
    /// The status lights of the statuses that change speed.
    pub statuses: &'static [(Shown, SpeedStatus)],
    /// The mutations `player_movement_speed` reads, by `mutation_type` identifier.
    pub mutations: &'static [&'static str],
    /// The banes it reads, by `bane_type` identifier.
    pub banes: &'static [&'static str],
    /// The messages crawl gives as a bane is gained, grows stronger and is lifted, each with
    /// `{}` where `bane_name` names the bane.
    pub bane_messages: &'static [Shown],
    /// How `bane_name` names a bane: `{}` where its `name` goes.
    pub bane_name: Shown,
    /// The gods whose followers are slowed and never hastened by a rage.
    pub slowing_gods: &'static [Shown],
    /// The gods whose followers walk on water, with the piety rank they do from, each with what
    /// `god-passive.cc` must hold for it.
    pub water_walking_gods: &'static [(Shown, u32, Shown)],
    /// What the name of armour that slows its wearer holds.
    pub ponderous_gear: &'static [Shown],
    /// What the name of armour that quickens its wearer holds.
    pub quickening_gear: &'static [Shown],
}

/// What crawl 0.34 reads of the player's speed: `player_speed` (`player.cc:1921`),
/// `player_movement_speed` (`player.cc:1843`), the speed and liquefied lights of
/// `fill_status_info` (`status.cc:427`, `status.cc:431`, `status.cc:1215`) and `have_passive`
/// (`god-passive.cc:290`).
pub const SPEED_RULES_0_34: SpeedRules = SpeedRules {
    statuses: &[
        (Shown::quoted("Fast", "status.cc"), SpeedStatus::Hasted),
        (Shown::quoted("Slow", "status.cc"), SpeedStatus::Slowed),
        (
            Shown::quoted("Fast+Slow", "status.cc"),
            SpeedStatus::HastedAndSlowed,
        ),
        (
            Shown::quoted("Berserk", "duration-data.h"),
            SpeedStatus::Berserk,
        ),
        (
            Shown::quoted("Swift", "duration-data.h"),
            SpeedStatus::Swift,
        ),
        (Shown::quoted("-Swift", "status.cc"), SpeedStatus::Unswift),
        (
            Shown::quoted("Frozen", "duration-data.h"),
            SpeedStatus::Frozen,
        ),
        (
            Shown::quoted("Petr", "duration-data.h"),
            SpeedStatus::Petrifying,
        ),
        (
            Shown::quoted("SlowM", "status.cc"),
            SpeedStatus::SlowedMovement,
        ),
        (Shown::quoted("Fly", "status.cc"), SpeedStatus::Flying),
        (
            Shown::quoted("Para", "duration-data.h"),
            SpeedStatus::Helpless,
        ),
        (
            Shown::quoted("Stone", "duration-data.h"),
            SpeedStatus::Helpless,
        ),
    ],
    mutations: &[
        "MUT_FAST",
        "MUT_SLOW",
        "MUT_FROG_LEGS",
        "MUT_CONSTRICTING_TAIL",
        "MUT_NIMBLE_SWIMMER",
    ],
    banes: &["BANE_LETHARGY"],
    bane_messages: &[
        // `add_bane`, for a new bane and one gained again.
        Shown {
            text: "You are stricken with the {}.",
            file: "mutation.cc",
            evidence: Some("\"You are stricken with the %s.\""),
        },
        Shown {
            text: "Your {} grows stronger.",
            file: "mutation.cc",
            evidence: Some("\"Your %s grows stronger.\""),
        },
        // `remove_bane`.
        Shown {
            text: "The {} upon you is lifted.",
            file: "mutation.cc",
            evidence: Some("\"The %s upon you is lifted.\""),
        },
    ],
    bane_name: Shown {
        text: "Bane of {}",
        file: "mutation.cc",
        evidence: Some("\"Bane of %s\""),
    },
    slowing_gods: &[Shown {
        text: "Cheibriados",
        file: "god-passive.cc",
        evidence: Some(
            "// Cheibriados\n{\n{ -1, passive_t::no_haste },\n{ -1, passive_t::slowed },",
        ),
    }],
    water_walking_gods: &[(
        Shown::quoted("Beogh", "religion.cc"),
        5,
        Shown {
            text: "Beogh",
            file: "god-passive.cc",
            evidence: Some(
                "// Beogh\n{\n{ 1, passive_t::convert_orcs },\n{ 5, passive_t::water_walk },",
            ),
        },
    )],
    ponderous_gear: &[Shown {
        text: "ponderousness",
        file: "item-name.cc",
        evidence: Some("case SPARM_PONDEROUSNESS: return \"ponderousness\";"),
    }],
    quickening_gear: &[Shown {
        text: "lightning scales",
        file: "art-data.txt",
        evidence: Some("NAME: lightning scales"),
    }],
};

/// The file the status lights crawl draws in code are in.
const STATUS_FILE: &str = "status.cc";

/// The file the lights of simple durations are in.
const DURATION_FILE: &str = "duration-data.h";

/// The file the mutations are defined in.
const MUTATION_FILE: &str = "mutation-data.h";

/// The file the banes are defined in.
const BANE_FILE: &str = "bane-data.h";

/// What `fill_status_info` sets a light to without a literal of its own, which the extractor
/// reads elsewhere: a duration's light from `duration_data` (`ddef`, and `def` as `status.cc`'s
/// own copy names it), a form's from `dat/forms` (`form`), and a channelled spell's charge from
/// `_charge_strings` (`_charge_text`).
const LIGHTS_READ_ELSEWHERE: &[&str] = &["ddef", "def", "form", "_charge_text"];

/// The table of the lights a channelled spell's charge cycles through.
const CHARGE_STRINGS: &str = "_charge_strings";

impl Crawl {
    /// What the game shows of the player's speed, as `rules` reads it, with `unformed`, the body
    /// of the form `none`. Every status light crawl may show is classified: those of `rules`
    /// by what they tell, and every other, read from `duration-data.h` and `status.cc`, as
    /// telling nothing, but forms', which [`copilot_gk::HolinessSigns::forms`] lists.
    pub(crate) fn speed(
        &self,
        rules: &SpeedRules,
        unformed: FormBody,
    ) -> Result<SpeedSigns, ExtractError> {
        let mut lights: Vec<SpeedLight> = rules
            .statuses
            .iter()
            .map(|(shown, status)| {
                Ok(SpeedLight {
                    light: self.shown(shown)?,
                    status: Some(*status),
                })
            })
            .collect::<Result<_, ExtractError>>()?;
        let mut others = self.duration_lights()?;
        others.extend(self.status_lights()?);
        for light in others {
            if !lights.iter().any(|known| known.light == light) {
                lights.push(SpeedLight {
                    light,
                    status: None,
                });
            }
        }
        let shown = |shown: &[Shown]| -> Result<Vec<String>, ExtractError> {
            shown.iter().map(|shown| self.shown(shown)).collect()
        };
        Ok(SpeedSigns {
            lights,
            unformed,
            mutations: self.speed_mutations(rules.mutations)?,
            banes: self.speed_banes(rules)?,
            slowing_gods: shown(rules.slowing_gods)?,
            water_walking_gods: rules
                .water_walking_gods
                .iter()
                .map(|(god, piety_rank, passive)| {
                    self.shown(passive)?;
                    Ok(GodRank {
                        god: self.shown(god)?,
                        piety_rank: *piety_rank,
                    })
                })
                .collect::<Result<_, ExtractError>>()?,
            ponderous_gear: shown(rules.ponderous_gear)?,
            quickening_gear: shown(rules.quickening_gear)?,
        })
    }

    /// The lights of the simple durations, `duration_data`'s third field where it isn't empty.
    fn duration_lights(&self) -> Result<Vec<String>, ExtractError> {
        let source = self.parse(DURATION_FILE)?;
        let mut lights = Vec::new();
        for fields in source.entries("duration_data")? {
            let Some([Token::Str(light)]) = fields.get(2).map(Vec::as_slice) else {
                return Err(ExtractError::Unreadable {
                    file: DURATION_FILE.to_owned(),
                    what: "an entry of duration_data".to_owned(),
                });
            };
            if !light.is_empty() {
                lights.push(light.clone());
            }
        }
        Ok(lights)
    }

    /// The lights `status.cc` sets in code: each literal it sets `light_text` to, the words
    /// before what changes of one it builds, as `Torch` of `make_stringf("Torch (%d)", ...)` or
    /// `Winding` of `"Winding" + string(...)`, and the charges of `_charge_strings`. A light set
    /// any other way fails the extraction, unless it is read elsewhere
    /// ([`LIGHTS_READ_ELSEWHERE`]).
    fn status_lights(&self) -> Result<Vec<String>, ExtractError> {
        let source = self.parse(STATUS_FILE)?;
        let tokens = source.tokens();
        let mut lights = BTreeSet::new();
        let unreadable = |what: &str| ExtractError::Unreadable {
            file: STATUS_FILE.to_owned(),
            what: what.to_owned(),
        };
        for (at, window) in tokens.windows(2).enumerate() {
            let [Token::Ident(name), Token::Punct('=')] = window else {
                continue;
            };
            if name != "light_text" {
                continue;
            }
            let set = tokens.get(at + 2..).unwrap_or_default();
            match set {
                [Token::Str(light), Token::Punct('+'), ..] => {
                    lights.insert(light.trim_end().to_owned());
                }
                [Token::Str(light), ..] => {
                    lights.insert(light.clone());
                }
                [Token::Ident(function), Token::Punct('('), args @ ..]
                    if function == "make_stringf" =>
                {
                    let formats: Vec<&String> = args
                        .iter()
                        .take_while(|token| **token != Token::Punct(';'))
                        .filter_map(|token| match token {
                            Token::Str(format) if format.contains('%') => Some(format),
                            _ => None,
                        })
                        .collect();
                    if formats.is_empty() {
                        return Err(unreadable("a light built by make_stringf"));
                    }
                    for format in formats {
                        lights.insert(stem(format));
                    }
                }
                [Token::Ident(source_name), ..]
                    if LIGHTS_READ_ELSEWHERE.contains(&source_name.as_str()) =>
                {
                    if source_name == "_charge_text" {
                        lights.extend(charges(&source)?);
                    }
                }
                _ => return Err(unreadable("a status light")),
            }
        }
        lights.retain(|light| !light.is_empty());
        Ok(lights.into_iter().collect())
    }

    /// The mutations of `ids`, from `mut_data`, each with its descriptions on the mutations
    /// screen and its messages for each of its levels.
    fn speed_mutations(&self, ids: &[&str]) -> Result<Vec<SpeedMutation>, ExtractError> {
        let source = self.parse(MUTATION_FILE)?;
        let entries = source.entries("mut_data")?;
        let unreadable = |what: String| ExtractError::Unreadable {
            file: MUTATION_FILE.to_owned(),
            what,
        };
        ids.iter()
            .map(|&id| {
                let fields = entries
                    .iter()
                    .find(|fields| {
                        fields.first().map(Vec::as_slice) == Some(&[Token::Ident(id.to_owned())])
                    })
                    .ok_or_else(|| ExtractError::Missing {
                        file: MUTATION_FILE.to_owned(),
                        what: id.to_owned(),
                    })?;
                let [_, _, levels, flags, _, have, gain, lose, ..] = fields.as_slice() else {
                    return Err(unreadable(format!("the entry of {id}")));
                };
                let levels = match levels.as_slice() {
                    [Token::Number(levels)] => levels
                        .parse::<usize>()
                        .map_err(|_| unreadable(format!("the levels of {id}")))?,
                    _ => return Err(unreadable(format!("the levels of {id}"))),
                };
                let texts = |field: &[Token], what: &str| -> Result<Vec<String>, ExtractError> {
                    let [Token::Punct('{'), inner @ .., Token::Punct('}')] = field else {
                        return Err(unreadable(format!("the {what} of {id}")));
                    };
                    inner
                        .iter()
                        .filter(|token| **token != Token::Punct(','))
                        .take(levels)
                        .map(|token| match token {
                            Token::Str(text) => Ok(text.clone()),
                            _ => Err(unreadable(format!("the {what} of {id}"))),
                        })
                        .collect()
                };
                let shown = texts(have, "descriptions")?;
                if shown.len() != levels || shown.iter().any(String::is_empty) {
                    return Err(unreadable(format!("the descriptions of {id}")));
                }
                let mut messages: Vec<String> = Vec::new();
                for message in texts(gain, "gain messages")?
                    .into_iter()
                    .chain(texts(lose, "lose messages")?)
                {
                    if !message.is_empty() && !messages.contains(&message) {
                        messages.push(message);
                    }
                }
                Ok(SpeedMutation {
                    id: EnumId(id.to_owned()),
                    shown,
                    anatomy: flags.contains(&Token::Ident("anatomy".to_owned())),
                    messages,
                })
            })
            .collect()
    }

    /// The banes of `rules`, from `bane_data`, each with its description on the mutations
    /// screen and the messages crawl gives as it comes and goes.
    fn speed_banes(&self, rules: &SpeedRules) -> Result<Vec<SpeedBane>, ExtractError> {
        let source = self.parse(BANE_FILE)?;
        let entries = source.entries("bane_data")?;
        let name_of = self.shown(&rules.bane_name)?;
        let templates: Vec<String> = rules
            .bane_messages
            .iter()
            .map(|shown| self.shown(shown))
            .collect::<Result<_, _>>()?;
        rules
            .banes
            .iter()
            .map(|&id| {
                let fields = entries
                    .iter()
                    .find(|fields| {
                        fields.first().map(Vec::as_slice) == Some(&[Token::Ident(id.to_owned())])
                    })
                    .ok_or_else(|| ExtractError::Missing {
                        file: BANE_FILE.to_owned(),
                        what: id.to_owned(),
                    })?;
                let [_, _, name, description] = fields.as_slice() else {
                    return Err(ExtractError::Unreadable {
                        file: BANE_FILE.to_owned(),
                        what: format!("the entry of {id}"),
                    });
                };
                let ([Token::Str(name)], [Token::Str(shown)]) =
                    (name.as_slice(), description.as_slice())
                else {
                    return Err(ExtractError::Unreadable {
                        file: BANE_FILE.to_owned(),
                        what: format!("the entry of {id}"),
                    });
                };
                let named = name_of.replace("{}", name);
                Ok(SpeedBane {
                    id: EnumId(id.to_owned()),
                    shown: shown.clone(),
                    messages: templates
                        .iter()
                        .map(|template| template.replace("{}", &named))
                        .collect(),
                })
            })
            .collect()
    }
}

/// The words of a light `format` builds before the first thing it fills in, without the spaces
/// and punctuation that lead up to it: `Torch` of `Torch (%d)`, `Slay` of `Slay +%d`.
fn stem(format: &str) -> String {
    let before = format.split('%').next().unwrap_or_default();
    before
        .trim_end_matches(|c: char| !c.is_alphanumeric())
        .to_owned()
}

/// The lights of `_charge_strings`, a list of string literals.
fn charges(source: &Source) -> Result<Vec<String>, ExtractError> {
    source
        .initializer(CHARGE_STRINGS)?
        .iter()
        .filter(|token| **token != Token::Punct(','))
        .map(|token| match token {
            Token::Str(light) => Ok(light.clone()),
            _ => Err(ExtractError::Unreadable {
                file: STATUS_FILE.to_owned(),
                what: format!("an entry of {CHARGE_STRINGS}"),
            }),
        })
        .collect()
}

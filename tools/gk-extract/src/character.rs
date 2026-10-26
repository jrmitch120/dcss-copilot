//! What the game says as the character changes: the messages crawl gives as a skill's level
//! changes, a spell is learned or forgotten, a god's power comes or goes, and a mutation or bane
//! is gained or lost. The skill, spell and power messages, which crawl writes in code, are ported
//! by hand for each release series and checked against crawl's source; the mutations' and banes'
//! are read from their tables.

use copilot_gk::{Bane, CharacterSigns, EnumId, Mutation, SkillSigns, SpellSigns, Template};

use crate::cpp::Token;
use crate::{Crawl, ExtractError, Shown};

/// The messages a release series gives as the character changes, each checked against crawl's
/// source, with `{}` where crawl fills something in.
pub struct CharacterRules {
    /// As a skill's level rises or falls, the skill's name first and its new level last.
    pub skill_changed: &'static [Shown],
    /// As a skill reaches the most it can.
    pub skill_mastered: Shown,
    /// The most a skill's level can be.
    pub most_skill: Shown,
    /// What comes before the skill's name when the level it says is the base level.
    pub base_skill: Shown,
    /// The gods whose followers' skills differ from their base levels by their piety or penance,
    /// each with what `player::skill` must hold for it.
    pub shifting_gods: &'static [(Shown, Shown)],
    /// As the player forgets a spell.
    pub spell_forgotten: Shown,
    /// As the player finishes memorising a spell.
    pub spell_memorised: Shown,
    /// As the player learns a spell without memorising it.
    pub spell_learned: &'static [Shown],
    /// As the player gains or loses a god's power.
    pub powers: &'static [Shown],
    /// As a bane is gained and grows stronger, each with `{}` where `bane_name` names the bane.
    pub bane_gained: &'static [Shown],
    /// As a bane is lifted, with `{}` where `bane_name` names the bane.
    pub bane_lifted: Shown,
    /// How `bane_name` names a bane: `{}` where its `name` goes.
    pub bane_name: Shown,
}

/// What crawl 0.34 says as the character changes: `_change_skill_level` (`skills.cc:319`),
/// `player::skill` (`player.cc:6212`), `del_spell_from_memory_by_slot` (`spl-util.cc:424`),
/// `MemoriseDelay::finish` (`delay.cc:756`), `_gain_innate_spells` (`player.cc:2869`),
/// `_revenant_spell_gift` (`player.cc:2920`), the callers of `god_power::display`
/// (`religion.cc:2398`, `religion.cc:2649`), and `add_bane` and `remove_bane`
/// (`mutation.cc:3470`).
pub const CHARACTER_RULES_0_34: CharacterRules = CharacterRules {
    skill_changed: &[
        Shown {
            text: "Your {} skill increases to level {}!",
            file: "skills.cc",
            evidence: Some(SKILL_CHANGED_0_34),
        },
        Shown {
            text: "Your {} skill decreases to level {}!",
            file: "skills.cc",
            evidence: Some(SKILL_CHANGED_0_34),
        },
        Shown {
            text: "Your {} skill gained {} levels and is now at level {}!",
            file: "skills.cc",
            evidence: Some(SKILLS_CHANGED_0_34),
        },
        Shown {
            text: "Your {} skill lost {} levels and is now at level {}!",
            file: "skills.cc",
            evidence: Some(SKILLS_CHANGED_0_34),
        },
    ],
    skill_mastered: Shown {
        text: "You have mastered {}!",
        file: "skills.cc",
        evidence: Some("\"You have mastered %s!\", skill_name(exsk)"),
    },
    most_skill: Shown {
        text: "27",
        file: "defines.h",
        evidence: Some("const int MAX_SKILL_LEVEL = 27;"),
    },
    base_skill: Shown {
        text: "base ",
        file: "skills.cc",
        evidence: Some("specify_base ? \"base \" : \"\""),
    },
    shifting_gods: &[(
        Shown::quoted("Ashenzari", "religion.cc"),
        Shown {
            text: "Ashenzari",
            file: "player.cc",
            evidence: Some(
                "if (penance[GOD_ASHENZARI])\n{\nif (temp)\nlevel = max(level - 4 * scale, level / 2);\n}\nelse if (ash_has_skill_boost(sk))\nlevel = ash_skill_boost(sk, scale);",
            ),
        },
    )],
    spell_forgotten: Shown {
        text: "Your memory of {} unravels.",
        file: "spl-util.cc",
        evidence: Some("\"Your memory of %s unravels.\""),
    },
    spell_memorised: Shown::quoted("You finish memorising.", "delay.cc"),
    spell_learned: &[
        Shown {
            text: "The power to cast {} wells up from within.",
            file: "player.cc",
            evidence: Some("\"The power to cast %s wells up from within.\""),
        },
        Shown {
            text: "(You can now cast {}.)",
            file: "player.cc",
            evidence: Some("\"(You can now cast %s.)\""),
        },
    ],
    powers: &[
        Shown {
            text: "You can now {}.",
            file: "religion.cc",
            evidence: Some("power.display(true, \"You can now %s.\");"),
        },
        Shown {
            text: "You can no longer {}.",
            file: "religion.cc",
            evidence: Some("power.display(false, \"You can no longer %s.\");"),
        },
    ],
    bane_gained: &[
        // For a new bane, then for one gained again.
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
    ],
    bane_lifted: Shown {
        text: "The {} upon you is lifted.",
        file: "mutation.cc",
        evidence: Some("\"The %s upon you is lifted.\""),
    },
    bane_name: Shown {
        text: "Bane of {}",
        file: "mutation.cc",
        evidence: Some("\"Bane of %s\""),
    },
};

/// How crawl 0.34 writes a skill's level rising or falling by one.
const SKILL_CHANGED_0_34: &str = "\"Your %s%s skill %s to level %d!\", specify_base ? \"base \" : \"\", skill_name(exsk), (n > 0) ? \"increases\" : \"decreases\"";

/// How crawl 0.34 writes a skill's level rising or falling by several.
const SKILLS_CHANGED_0_34: &str = "\"Your %s%s skill %s %d levels and is now \" \"at level %d!\", specify_base ? \"base \" : \"\", skill_name(exsk), (n > 0) ? \"gained\" : \"lost\"";

/// The file the mutations are defined in.
pub(crate) const MUTATION_FILE: &str = "mutation-data.h";

/// The file the banes are defined in.
pub(crate) const BANE_FILE: &str = "bane-data.h";

/// A mutation as `mut_data` defines it, with whether it is of the player's anatomy
/// (`mutflag::anatomy`), which stops working in a form that changes it.
pub struct MutationEntry {
    pub mutation: Mutation,
    pub anatomy: bool,
}

impl Crawl {
    /// What the game says as the character changes, as `rules` ports it, with every mutation
    /// and bane crawl defines.
    pub(crate) fn character(
        &self,
        rules: &CharacterRules,
        mutations: &[MutationEntry],
        banes: Vec<Bane>,
    ) -> Result<CharacterSigns, ExtractError> {
        let template = |shown: &Shown| self.shown(shown).map(Template);
        let templates = |shown: &[Shown]| -> Result<Vec<Template>, ExtractError> {
            shown.iter().map(template).collect()
        };
        let most = self.shown(&rules.most_skill)?;
        Ok(CharacterSigns {
            skills: SkillSigns {
                changed: templates(rules.skill_changed)?,
                mastered: template(&rules.skill_mastered)?,
                most: most.parse().map_err(|_| ExtractError::Unreadable {
                    file: rules.most_skill.file.to_owned(),
                    what: "MAX_SKILL_LEVEL".to_owned(),
                })?,
                base: self.shown(&rules.base_skill)?,
                shifting_gods: rules
                    .shifting_gods
                    .iter()
                    .map(|(god, code)| {
                        self.shown(code)?;
                        self.shown(god)
                    })
                    .collect::<Result<_, _>>()?,
            },
            spells: SpellSigns {
                forgotten: template(&rules.spell_forgotten)?,
                memorised: self.shown(&rules.spell_memorised)?,
                learned: templates(rules.spell_learned)?,
            },
            powers: templates(rules.powers)?,
            mutations: mutations
                .iter()
                .map(|entry| entry.mutation.clone())
                .collect(),
            banes,
        })
    }

    /// Every mutation in `mut_data`, each with its descriptions on the mutations screen and its
    /// messages for each of its levels.
    pub(crate) fn mutations(&self) -> Result<Vec<MutationEntry>, ExtractError> {
        let source = self.parse(MUTATION_FILE)?;
        let unreadable = |what: String| ExtractError::Unreadable {
            file: MUTATION_FILE.to_owned(),
            what,
        };
        source
            .entries("mut_data")?
            .iter()
            .map(|fields| {
                let [id, _, levels, flags, _, have, gain, lose, ..] = fields.as_slice() else {
                    return Err(unreadable("an entry of mut_data".to_owned()));
                };
                let [Token::Ident(id)] = id.as_slice() else {
                    return Err(unreadable("an entry of mut_data".to_owned()));
                };
                let levels = match levels.as_slice() {
                    [Token::Number(levels)] => levels
                        .parse::<usize>()
                        .map_err(|_| unreadable(format!("the levels of {id}")))?,
                    _ => return Err(unreadable(format!("the levels of {id}"))),
                };
                // Each of `have`, `gain` and `lose` lists at least one text per level.
                let texts = |field: &[Token], what: &str| -> Result<Vec<String>, ExtractError> {
                    let [Token::Punct('{'), inner @ .., Token::Punct('}')] = field else {
                        return Err(unreadable(format!("the {what} of {id}")));
                    };
                    let texts: Vec<String> = inner
                        .iter()
                        .filter(|token| **token != Token::Punct(','))
                        .take(levels)
                        .map(|token| match token {
                            Token::Str(text) => Ok(text.clone()),
                            _ => Err(unreadable(format!("the {what} of {id}"))),
                        })
                        .collect::<Result<_, _>>()?;
                    if texts.len() == levels {
                        Ok(texts)
                    } else {
                        Err(unreadable(format!("the {what} of {id}")))
                    }
                };
                let messages = |field: &[Token], what: &str| -> Result<_, ExtractError> {
                    Ok(texts(field, what)?
                        .into_iter()
                        .map(|text| (!text.is_empty()).then_some(text))
                        .collect())
                };
                // An empty description would describe every row of the mutations screen.
                let shown = texts(have, "descriptions")?;
                if shown.iter().any(String::is_empty) {
                    return Err(unreadable(format!("the descriptions of {id}")));
                }
                Ok(MutationEntry {
                    mutation: Mutation {
                        id: EnumId(id.clone()),
                        shown,
                        gained: messages(gain, "gain messages")?,
                        lost: messages(lose, "lose messages")?,
                    },
                    anatomy: flags.contains(&Token::Ident("anatomy".to_owned())),
                })
            })
            .collect()
    }

    /// Every bane in `bane_data`, each with its description on the mutations screen and the
    /// messages crawl gives as it comes and goes, as `rules` names them.
    pub(crate) fn banes(&self, rules: &CharacterRules) -> Result<Vec<Bane>, ExtractError> {
        let source = self.parse(BANE_FILE)?;
        let name_of = self.shown(&rules.bane_name)?;
        let gained: Vec<String> = rules
            .bane_gained
            .iter()
            .map(|shown| self.shown(shown))
            .collect::<Result<_, _>>()?;
        let lifted = self.shown(&rules.bane_lifted)?;
        source
            .entries("bane_data")?
            .iter()
            .map(|fields| {
                let unreadable = || ExtractError::Unreadable {
                    file: BANE_FILE.to_owned(),
                    what: "an entry of bane_data".to_owned(),
                };
                let [id, _, name, description] = fields.as_slice() else {
                    return Err(unreadable());
                };
                let ([Token::Ident(id)], [Token::Str(name)], [Token::Str(shown)]) =
                    (id.as_slice(), name.as_slice(), description.as_slice())
                else {
                    return Err(unreadable());
                };
                let named = name_of.replace("{}", name);
                Ok(Bane {
                    id: EnumId(id.clone()),
                    shown: shown.clone(),
                    gained: gained
                        .iter()
                        .map(|template| template.replace("{}", &named))
                        .collect(),
                    lifted: lifted.replace("{}", &named),
                })
            })
            .collect()
    }
}

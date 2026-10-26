//! What the game's messages tell of the character, as Game Knowledge words them: a skill's level
//! changing, a spell forgotten, memorised or learned, a god's power coming or going, and a
//! mutation or bane gained or lost.

use copilot_gk::{Bane, Bundle, CharacterSigns, Mutation, Template};

/// One thing a message tells of the character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Told<'b> {
    /// A skill's level changed.
    Skill {
        /// The skill, as the skills screen names it.
        name: String,
        /// Its new level, whole: crawl's message gives no fraction.
        level: u32,
        /// The level is the skill's base level, which differs from the level in effect the
        /// skills screen shows.
        base: bool,
    },
    /// The player forgot a spell, by its name.
    SpellForgotten(String),
    /// The player finished memorising a spell the message doesn't name.
    SpellMemorised,
    /// The player learned a spell without memorising it, so how often it fails isn't told.
    SpellLearned,
    /// One of their god's powers came or went.
    Power,
    /// A level of a mutation came or went: each change the message may tell, as some mutations
    /// share their messages, such as `You feel robust.`, which tells of robustness gained or
    /// frailty lost.
    Mutation(Vec<MutationChange<'b>>),
    /// A bane came, grew stronger or was lifted.
    Bane {
        /// The bane.
        bane: &'b Bane,
        /// It was lifted, rather than gained.
        lifted: bool,
    },
}

/// A mutation's level changing, as one of its messages tells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MutationChange<'b> {
    /// The mutation.
    pub(crate) mutation: &'b Mutation,
    /// Its level before, 0 for none.
    pub(crate) from: usize,
    /// Its level after.
    pub(crate) to: usize,
}

/// What a line of the game's messages tells of the character, through `bundle`. Crawl writes
/// messages given together on one line, a space apart, so the line is read as the messages it
/// holds, each the shortest run of whole words from where the last ended that reads as one.
pub(crate) fn told<'b>(line: &str, bundle: &'b Bundle) -> Vec<Told<'b>> {
    let starts: Vec<usize> = std::iter::once(0)
        .chain(line.match_indices([' ', '\n']).map(|(at, _)| at + 1))
        .collect();
    let mut news = Vec::new();
    let mut from = 0;
    for &start in &starts {
        if start < from {
            continue;
        }
        let rest = line.get(start..).unwrap_or_default();
        let ends = rest
            .match_indices([' ', '\n'])
            .map(|(at, _)| at)
            .chain([rest.len()]);
        for end in ends {
            let said = rest.get(..end).unwrap_or_default();
            if let Some(found) = one(said, bundle) {
                news.push(found);
                from = start + end;
                break;
            }
        }
    }
    news
}

/// What one message tells of the character, if anything.
fn one<'b>(said: &str, bundle: &'b Bundle) -> Option<Told<'b>> {
    let signs = &bundle.character;
    skill(said, bundle)
        .or_else(|| spell(said, signs))
        .or_else(|| {
            signs
                .powers
                .iter()
                .any(|template| template.names(said).is_some())
                .then_some(Told::Power)
        })
        .or_else(|| mutation(said, signs))
        .or_else(|| bane(said, signs))
}

/// A skill's level changing, as `said` tells it.
fn skill(said: &str, bundle: &Bundle) -> Option<Told<'static>> {
    let signs = &bundle.character.skills;
    let (named, level) = signs
        .changed
        .iter()
        .find_map(|template| {
            let names = template.names(said)?;
            let level = names.last()?.parse().ok()?;
            Some((*names.first()?, level))
        })
        .or_else(|| {
            let names = signs.mastered.names(said)?;
            Some((*names.first()?, signs.most))
        })?;
    let (name, base) = match named.strip_prefix(signs.base.as_str()) {
        Some(name) => (name, true),
        None => (named, false),
    };
    bundle
        .skills
        .values()
        .any(|skill| skill == name)
        .then(|| Told::Skill {
            name: name.to_owned(),
            level,
            base,
        })
}

/// A spell forgotten, memorised or learned, as `said` tells it.
fn spell(said: &str, signs: &CharacterSigns) -> Option<Told<'static>> {
    let spells = &signs.spells;
    if said == spells.memorised {
        return Some(Told::SpellMemorised);
    }
    if let Some(names) = spells.forgotten.names(said) {
        return Some(Told::SpellForgotten((*names.first()?).to_owned()));
    }
    spells
        .learned
        .iter()
        .any(|template: &Template| template.names(said).is_some())
        .then_some(Told::SpellLearned)
}

/// A level of a mutation coming or going, as `said` tells it: every change it may tell.
fn mutation<'b>(said: &str, signs: &'b CharacterSigns) -> Option<Told<'b>> {
    let mut changes = Vec::new();
    for mutation in &signs.mutations {
        for (at, gained) in mutation.gained.iter().enumerate() {
            if gained.as_deref() == Some(said) {
                changes.push(MutationChange {
                    mutation,
                    from: at,
                    to: at + 1,
                });
            }
        }
        // Crawl gives the message for losing a level by the level left (`delete_mutation`).
        for (at, lost) in mutation.lost.iter().enumerate() {
            if lost.as_deref() == Some(said) {
                changes.push(MutationChange {
                    mutation,
                    from: at + 1,
                    to: at,
                });
            }
        }
    }
    (!changes.is_empty()).then_some(Told::Mutation(changes))
}

/// A bane coming, growing stronger or being lifted, as `said` tells it.
fn bane<'b>(said: &str, signs: &'b CharacterSigns) -> Option<Told<'b>> {
    signs.banes.iter().find_map(|bane| {
        if bane.lifted == said {
            Some(Told::Bane { bane, lifted: true })
        } else {
            bane.gained
                .iter()
                .any(|gained| gained == said)
                .then_some(Told::Bane {
                    bane,
                    lifted: false,
                })
        }
    })
}

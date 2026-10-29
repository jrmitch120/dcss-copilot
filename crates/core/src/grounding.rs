//! Grounding Checks: what the Advisor keeps of the reason an LLM gives. Every number and every Game
//! Knowledge name it says must be in the prompt it was given (ADR 0003); a sentence that says one
//! that isn't is dropped silently, and the pick it explains is kept. What is left is trimmed to
//! whole sentences within the call's word limit.

use std::collections::BTreeSet;

use copilot_gk::Bundle;

/// Game Knowledge names that are everyday English words, which a reason uses as such: `fire`, a
/// wand, in "under fire"; `slow`, a spell, in "too slow". The entity check passes over them, as
/// a sentence using one is far likelier plain English than an invented monster or item. Tuned on
/// the corpus (#45).
const EVERYDAY_WORDS: &[&str] = &[
    "acid", "agony", "air", "blood", "bolt", "bone", "chaos", "charm", "cold", "dark", "death",
    "decay", "dig", "earth", "faith", "fear", "fire", "flame", "fog", "frost", "giant", "gloom",
    "haste", "hexes", "ice", "iron", "light", "magic", "melee", "might", "noise", "orb", "pain",
    "plant", "power", "roll", "roots", "shock", "sleep", "slow", "sting", "stone", "throw",
    "touch",
];

/// The Game Knowledge names an answer may say only if its prompt does: monsters (uniques among
/// them), item kinds, spells, species, branches and the gods the bundle names, lowercased.
#[derive(Debug, Clone, Default)]
pub(crate) struct EntityNames(BTreeSet<String>);

impl EntityNames {
    /// The names in `knowledge`, but for everyday English words.
    pub(crate) fn of(knowledge: &Bundle) -> Self {
        let monsters = knowledge.monsters.values().map(|monster| &monster.name);
        let items = knowledge
            .item_classes
            .values()
            .flat_map(|class| class.sub_types.values().map(|kind| &kind.name));
        let spells = knowledge.spells.values().map(|spell| &spell.name);
        let species = knowledge.species.iter().map(|species| &species.name);
        let branches = knowledge.branches.iter().map(|branch| &branch.place);
        let gods = knowledge
            .holiness
            .good_gods
            .iter()
            .chain(&knowledge.lighting.halo_gods);
        Self(
            monsters
                .chain(items)
                .chain(spells)
                .chain(species)
                .chain(branches)
                .chain(gods)
                .map(|name| name.to_lowercase())
                .filter(|name| !name.is_empty() && !EVERYDAY_WORDS.contains(&name.as_str()))
                .collect(),
        )
    }
}

/// What is kept of an LLM's `reason`, given the `prompt` it answered: its sentences that say no
/// number and no name of `names` the prompt doesn't, trimmed to whole sentences within `words`,
/// the first always kept. `None` when no sentence is left.
pub(crate) fn kept_reason(
    reason: &str,
    prompt: &str,
    names: &EntityNames,
    words: u32,
) -> Option<String> {
    let prompt_numbers: BTreeSet<&str> = numbers(prompt).collect();
    let prompt_lower = prompt.to_lowercase();
    let grounded = sentences(reason).into_iter().filter(|sentence| {
        numbers(sentence).all(|number| prompt_numbers.contains(number)) && {
            let lower = sentence.to_lowercase();
            names
                .0
                .iter()
                .filter(|name| says(&lower, name))
                .all(|name| says(&prompt_lower, name))
        }
    });
    let mut kept: Vec<&str> = Vec::new();
    let mut said = 0;
    for sentence in grounded {
        let count = u32::try_from(sentence.split_whitespace().count()).unwrap_or(u32::MAX);
        if !kept.is_empty() && said + count > words {
            break;
        }
        said += count;
        kept.push(sentence);
    }
    (!kept.is_empty()).then(|| kept.join(" "))
}

/// The sentences of `text`, each ending at `.`, `!` or `?` before a space or the end, with what
/// follows the last one as a sentence of its own.
fn sentences(text: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((index, char)) = chars.next() {
        let ends = matches!(char, '.' | '!' | '?')
            && chars.peek().is_none_or(|(_, next)| next.is_whitespace());
        if ends {
            let end = index + char.len_utf8();
            sentences.push(&text[start..end]);
            start = end;
        }
    }
    sentences.push(&text[start..]);
    sentences
        .into_iter()
        .map(str::trim)
        .filter(|sentence| !sentence.is_empty())
        .collect()
}

/// The numbers written in `text` with digits, such as `62` or `1.5`.
fn numbers(text: &str) -> impl Iterator<Item = &str> {
    let bytes = text.as_bytes();
    let mut index = 0;
    std::iter::from_fn(move || {
        while index < bytes.len() && !bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == bytes.len() {
            return None;
        }
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        // A decimal point counts only between digits, so a sentence's full stop doesn't.
        if bytes.get(index) == Some(&b'.') && bytes.get(index + 1).is_some_and(u8::is_ascii_digit) {
            index += 1;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
        }
        text.get(start..index)
    })
}

/// Whether lowercase `text` says `name` as words of its own, or as their plural.
fn says(text: &str, name: &str) -> bool {
    let word_char = |char: char| char.is_alphanumeric() || char == '_';
    text.match_indices(name).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = &text[at + name.len()..];
        let rest = after
            .strip_prefix("es")
            .or_else(|| after.strip_prefix('s'))
            .filter(|rest| !rest.starts_with(word_char));
        before.is_none_or(|char| !word_char(char))
            && (!after.starts_with(word_char) || rest.is_some())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(names: &[&str]) -> EntityNames {
        EntityNames(names.iter().map(|&name| name.to_owned()).collect())
    }

    const PROMPT: &str = "Threat: LETHAL · death 12.5% · worst hit 30 of 24 HP\n\
                          - ogre · 1 square away\n\
                          a1 Scroll of teleportation";

    #[test]
    fn a_sentence_quoting_a_number_the_prompt_doesnt_is_dropped() {
        let kept = kept_reason(
            "You have 24 HP. You'll last 3 turns. Death is 12.5% likely.",
            PROMPT,
            &names(&[]),
            100,
        );

        assert_eq!(
            kept.as_deref(),
            Some("You have 24 HP. Death is 12.5% likely.")
        );
    }

    #[test]
    fn a_sentence_naming_a_monster_item_or_unique_the_prompt_doesnt_is_dropped() {
        let names = names(&["ogre", "orc priest", "teleportation", "sigmund", "orc"]);

        let kept = kept_reason(
            "Two ogres close in. An orc priest is near. Sigmund waits. Teleportation saves you.",
            PROMPT,
            &names,
            100,
        );

        assert_eq!(
            kept.as_deref(),
            Some("Two ogres close in. Teleportation saves you.")
        );
    }

    #[test]
    fn a_name_inside_another_word_isnt_said() {
        let names = names(&["rat", "orc"]);

        let kept = kept_reason(
            "The rate of damage is high. Force your way out.",
            PROMPT,
            &names,
            100,
        );

        assert_eq!(
            kept.as_deref(),
            Some("The rate of damage is high. Force your way out.")
        );
    }

    #[test]
    fn whole_sentences_are_kept_within_the_word_limit_and_the_first_always() {
        assert_eq!(
            kept_reason(
                "Read it now. The ogre hits hard and fast.",
                PROMPT,
                &names(&[]),
                5
            )
            .as_deref(),
            Some("Read it now.")
        );
        assert_eq!(
            kept_reason(
                "The ogre is beside you and hits very hard. Go.",
                PROMPT,
                &names(&[]),
                3
            )
            .as_deref(),
            Some("The ogre is beside you and hits very hard.")
        );
    }

    #[test]
    fn nothing_is_kept_when_every_sentence_fails() {
        assert_eq!(
            kept_reason("You have 99 HP.", PROMPT, &names(&[]), 25),
            None
        );
        assert_eq!(kept_reason("  ", PROMPT, &names(&[]), 25), None);
    }

    #[test]
    fn a_decimal_point_isnt_a_sentence_end_and_a_full_stop_isnt_a_decimal() {
        assert_eq!(
            sentences("Death 12.5% now. Go!"),
            ["Death 12.5% now.", "Go!"]
        );
        assert_eq!(
            numbers("worst hit 30. Then 1.5").collect::<Vec<_>>(),
            ["30", "1.5"]
        );
    }
}

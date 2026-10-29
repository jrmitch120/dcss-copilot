//! What the game says of the level the player is on: the items on the square they come to, a
//! shop's stock, exploring finding nothing more and a monster dying. Crawl writes each in code,
//! so they are ported by hand for each release series and checked against crawl's source.

use copilot_gk::{LevelSigns, Template};

use crate::{Crawl, ExtractError, Shown};

/// What a release series says of the level, each checked against crawl's source, with `{}`
/// where crawl fills something in.
pub struct LevelRules {
    /// As the player comes to a square one item lies on.
    pub item_here: Shown,
    /// Before the message naming the few items on the square the player comes to.
    pub items_here: Shown,
    /// What that message names its items apart with.
    pub items_apart: Shown,
    /// As the player comes to a square more items lie on than crawl names.
    pub items_unnamed: &'static [Shown],
    /// As exploring finds nothing more to go to.
    pub done_exploring: Shown,
    /// The tag of a shop's menu.
    pub shop_tag: Shown,
    /// The title of a shop's menu, the shop's name filled in.
    pub shop_title: Shown,
    /// An item of a shop's menu, after its hotkey and its mark: its price, then its name.
    pub shop_item: Shown,
    /// The marks a shop's menu puts after an item's hotkey.
    pub shop_marks: Shown,
    /// As a monster dies in view, the monster as crawl names it with `the`.
    pub monster_died: &'static [Shown],
}

/// How `monster_die` (`mon-death.cc:3033`) says the player killed a monster.
const PLAYER_KILLED_0_34: &str =
    "\"You %s %s!\", exploded ? \"blow up\" : destroyed ? \"destroy\" : \"kill\"";

/// How it says a monster the player confused died.
const CONFUSED_KILLED_0_34: &str = "\"%s is %s!\", mons.name(DESC_THE).c_str(), exploded ? \"blown up\" : destroyed ? \"destroyed\" : \"killed\"";

/// How it says a monster died otherwise (`simple_monster_message`).
const DIED_0_34: &str =
    "exploded ? \" is blown up!\" : destroyed ? \" is destroyed!\" : \" dies!\";";

/// What crawl 0.34 says of the level: `item_check` and `item_message` (`items.cc:909`),
/// `explore_discoveries` (`travel.cc:775`), `ShopMenu` and `ShopEntry` (`shopping.cc:992`) and
/// `monster_die` (`mon-death.cc:3015`).
pub const LEVEL_RULES_0_34: LevelRules = LevelRules {
    item_here: Shown {
        text: "You see here {}.",
        file: "items.cc",
        evidence: Some("strm << \"You see here \" << name << '.' << endl;"),
    },
    items_here: Shown::quoted("Things that are here:", "items.cc"),
    items_apart: Shown {
        text: "; ",
        file: "items.cc",
        evidence: Some("join_strings(colour_names.begin(), colour_names.end(), \"; \")"),
    },
    items_unnamed: &[
        Shown {
            text: "Items here: {}.",
            file: "items.cc",
            evidence: Some("\"Items here: %s.\", desc_string.c_str()"),
        },
        Shown {
            text: "There are many items here.",
            file: "items.cc",
            evidence: Some("strm << \"There are many items here.\" << endl;"),
        },
    ],
    done_exploring: Shown::quoted("Done exploring.", "travel.cc"),
    shop_tag: Shown {
        text: "shop",
        file: "shopping.cc",
        evidence: Some("set_tag(\"shop\");"),
    },
    shop_title: Shown {
        text: "Welcome to {}! What would you like to do?",
        file: "shopping.cc",
        evidence: Some("\"Welcome to \" + shop_name(shop) + \"! What would you \" \"like to do?\""),
    },
    shop_item: Shown {
        text: "{} gold   {}",
        file: "shopping.cc",
        evidence: Some("%4d gold   %s%s"),
    },
    shop_marks: Shown {
        text: "-+$",
        file: "shopping.cc",
        evidence: Some("selected() ? '+' : on_list ? '$' : '-'"),
    },
    monster_died: &[
        Shown {
            text: "You kill {}!",
            file: "mon-death.cc",
            evidence: Some(PLAYER_KILLED_0_34),
        },
        Shown {
            text: "You destroy {}!",
            file: "mon-death.cc",
            evidence: Some(PLAYER_KILLED_0_34),
        },
        Shown {
            text: "You blow up {}!",
            file: "mon-death.cc",
            evidence: Some(PLAYER_KILLED_0_34),
        },
        Shown {
            text: "{} is killed!",
            file: "mon-death.cc",
            evidence: Some(CONFUSED_KILLED_0_34),
        },
        Shown {
            text: "{} is destroyed!",
            file: "mon-death.cc",
            evidence: Some(DIED_0_34),
        },
        Shown {
            text: "{} is blown up!",
            file: "mon-death.cc",
            evidence: Some(DIED_0_34),
        },
        Shown {
            text: "{} dies!",
            file: "mon-death.cc",
            evidence: Some(DIED_0_34),
        },
    ],
};

impl Crawl {
    /// What the game says of the level, as `rules` ports it.
    pub(crate) fn level(&self, rules: &LevelRules) -> Result<LevelSigns, ExtractError> {
        let template = |shown: &Shown| self.shown(shown).map(Template);
        Ok(LevelSigns {
            item_here: template(&rules.item_here)?,
            items_here: self.shown(&rules.items_here)?,
            items_apart: self.shown(&rules.items_apart)?,
            items_unnamed: rules
                .items_unnamed
                .iter()
                .map(template)
                .collect::<Result<_, _>>()?,
            done_exploring: self.shown(&rules.done_exploring)?,
            shop_tag: self.shown(&rules.shop_tag)?,
            shop_title: template(&rules.shop_title)?,
            shop_item: template(&rules.shop_item)?,
            shop_marks: self.shown(&rules.shop_marks)?,
            monster_died: rules
                .monster_died
                .iter()
                .map(template)
                .collect::<Result<_, _>>()?,
        })
    }
}

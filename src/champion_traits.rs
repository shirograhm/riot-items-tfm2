//! What Smart Builds needs to know about a champion: what its damage scales
//! with, to pick its boots, its class and whether it tanks or keeps allies
//! alive, whether its kit can immobilize an enemy (a support that can builds
//! Imperial Mandate), and whether it attacks from range (ranged and melee
//! champions keep different items).
//!
//! # Where the answer comes from
//!
//! The game's own champion data, asked for at runtime through
//! `StableClient::champion_brief`. That covers champions added by other mods,
//! which declare the same `category` and `tags` a vanilla one does. The build
//! paths cannot ask themselves — the item-build context carries only the
//! champion's key, and two of the three paths are not on the client at all —
//! so [`learn`] asks on the client frame loop and caches the answers here.
//!
//! `champion_brief` had never been called on this host when this was written
//! (its sibling `champion_names()` returns nothing), so it is not trusted to
//! answer. Two fallbacks back it up: [`VANILLA`] for the base game's
//! champions, and [`MOD_CHAMPIONS`] for everyone else's, read from the other
//! mods' own `.data_champion` files at startup. A champion none of the three
//! knows gets no restriction at all: a missing tag must never cost a build an
//! item.
//!
//! The file fallback is also what covers a champion's *first* build. The host
//! is only asked on the client frame after a build path wants the answer, so
//! with the host alone the first build decided for a modded champion (a
//! Twitch from another mod, 2026-09-21, building pure AP) went unchecked.
//!
//! The host's answer has no attack range in it, so whether a champion is
//! ranged always comes from the fallbacks where they know it: both carry the
//! reach of the basic attack itself. See [`ranged_of`]. Whether a kit
//! immobilizes is not in the host's answer either, only the broader `CC` tag,
//! so that comes from the fallbacks the same way.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};

use mod_api_stable::{ChampionCategoryV1, ChampionTagV1, StableClient};

/// What a champion's damage scales with, from its `AD`/`AP` tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scaling {
    Ad,
    Ap,
    Hybrid,
}

/// The champion's `category`: what the game groups it under in the draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Melee,
    Range,
    Magician,
    Util,
    Assassin,
}

impl Class {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "Melee" => Some(Class::Melee),
            "Range" => Some(Class::Range),
            "Magician" => Some(Class::Magician),
            "Util" => Some(Class::Util),
            "Assassin" => Some(Class::Assassin),
            _ => None,
        }
    }

    fn from_category(category: ChampionCategoryV1) -> Self {
        match category {
            ChampionCategoryV1::Melee => Class::Melee,
            ChampionCategoryV1::Range => Class::Range,
            ChampionCategoryV1::Magician => Class::Magician,
            ChampionCategoryV1::Util => Class::Util,
            ChampionCategoryV1::Assassin => Class::Assassin,
        }
    }
}

/// What a kit immobilizes with: the crowd control that takes movement out of
/// an enemy's hands, the set Imperial Mandate's Command answers to. A slow, a
/// silence or a disarm is none of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Immobilize {
    /// A taunt. Kept apart because a taunt names its taunter, so Command
    /// credits it without guessing.
    pub taunt: bool,
    /// Anything else: a stun, root, knock-up, knockback, pull, fear or charm.
    pub other: bool,
    /// How long the kit immobilizes for in all, in ticks: every stun, root,
    /// knock-up, knockback, pull, fear, charm and taunt of its abilities and
    /// its ultimate, added up. An immobilize the source puts no figure on
    /// adds nothing.
    pub ticks: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChampionTraits {
    /// `None` for a champion tagged neither way, which the rules leave alone.
    pub scaling: Option<Scaling>,
    /// `None` when the source did not say.
    pub class: Option<Class>,
    /// Tagged `Tank`.
    pub tank: bool,
    /// Tagged `Heal` or `Shield`. The tags do not say whom: a Vampire's healing
    /// is all its own.
    pub sustains: bool,
    /// Whether a basic ability of its kit heals, shields or buffs an ally,
    /// which is what the items that answer to that need. Narrower than
    /// `sustains` (the Vampire) and wider (a Bard only buffs). `None` when
    /// only the tags are known.
    pub aids: Option<bool>,
    /// Tagged `CC`: its kit slows, stuns or otherwise holds enemies.
    pub cc: bool,
    /// What its abilities immobilize with, which is narrower than `cc`. `None`
    /// when only the tag is known.
    pub immobilize: Option<Immobilize>,
    /// Whether its basic attack reaches past [`MELEE_ATTACK_RANGE`]. `None`
    /// when nothing says, which the rules leave alone.
    pub ranged: Option<bool>,
}

impl ChampionTraits {
    /// `attack_range` is the reach of the basic attack in world units, where
    /// the source states one, `immobilize` what its kit says about that, and
    /// `aids` whether it says the kit aids allies.
    fn from_flags(
        flags: u8,
        class: Option<Class>,
        attack_range: Option<usize>,
        immobilize: Option<Immobilize>,
        aids: Option<bool>,
    ) -> Self {
        Self {
            scaling: scaling_of(flags & AD != 0, flags & AP != 0),
            class,
            tank: flags & TANK != 0,
            sustains: flags & (HEAL | SHIELD) != 0,
            aids,
            cc: flags & CC != 0,
            immobilize,
            ranged: ranged_of(attack_range, flags & RANGE != 0, flags & MELEE != 0, class),
        }
    }

    /// Whether the kit can set off Imperial Mandate: what the kit itself says
    /// where it is known, else the `CC` tag, which also counts slows.
    pub(crate) fn can_immobilize(&self) -> bool {
        self.immobilize
            .map_or(self.cc, |immobilize| immobilize.taunt || immobilize.other)
    }

    /// Whether the kit immobilizes for at least `ticks` in all, where it is
    /// known; else the `CC` tag, which also counts slows.
    pub(crate) fn immobilizes_for(&self, ticks: usize) -> bool {
        self.immobilize
            .map_or(self.cc, |immobilize| immobilize.ticks >= ticks)
    }

    /// Whether the kit can set off the items that answer to healing, shielding
    /// or buffing an ally: what the kit itself says where it is known, else
    /// the `Heal` and `Shield` tags.
    pub(crate) fn aids_allies(&self) -> bool {
        self.aids.unwrap_or(self.sustains)
    }
}

/// The longest basic attack that still counts as melee, in the range units
/// tooltips use: a champion reaching further is ranged. The same 35 the melee
/// items measure their falloff from (`effect_melee_distance`), so a champion
/// is ranged exactly when its own attacks land at reduced strength. Vanilla
/// melee champions reach 23-30 and the ranged ones 40-80.
const MELEE_ATTACK_RANGE: usize = 35;

/// Whether a champion attacks from range, `None` when nothing says: the reach
/// of its basic attack (world units) where the source states one, else its
/// `Range` or `Melee` tag, else its class where that settles it. The tags and
/// classes are a last resort: only six vanilla champions carry either tag,
/// Vampire is a Magician that attacks at 25, and Clown an Assassin at 40.
fn ranged_of(
    attack_range: Option<usize>,
    range_tag: bool,
    melee_tag: bool,
    class: Option<Class>,
) -> Option<bool> {
    if let Some(range) = attack_range {
        return Some(range > MELEE_ATTACK_RANGE * crate::DISTANCE_UNITS_PER_RANGE);
    }
    match (range_tag, melee_tag, class) {
        (true, false, _) => Some(true),
        (false, true, _) => Some(false),
        (false, false, Some(Class::Range)) => Some(true),
        (false, false, Some(Class::Melee)) => Some(false),
        _ => None,
    }
}

fn scaling_of(ad: bool, ap: bool) -> Option<Scaling> {
    match (ad, ap) {
        (true, true) => Some(Scaling::Hybrid),
        (true, false) => Some(Scaling::Ad),
        (false, true) => Some(Scaling::Ap),
        (false, false) => None,
    }
}

const AD: u8 = 1;
const AP: u8 = 2;
const TANK: u8 = 4;
const HEAL: u8 = 8;
const SHIELD: u8 = 16;
const CC: u8 = 32;
const RANGE: u8 = 64;
const MELEE: u8 = 128;

/// The flag bits for a champion's `tags`, by tag name.
fn flags_of<'a>(tags: impl IntoIterator<Item = &'a str>) -> u8 {
    tags.into_iter().fold(0, |flags, tag| {
        flags
            | match tag.to_ascii_lowercase().as_str() {
                "ad" => AD,
                "ap" => AP,
                "tank" => TANK,
                "heal" => HEAL,
                "shield" => SHIELD,
                "cc" => CC,
                "range" => RANGE,
                "melee" => MELEE,
                _ => 0,
            }
    })
}

/// The base game's champions, from `setting/champion_info` (`category`, `tags`
/// and `attack.range`), including the eight it ships under `mod_champions`.
/// Only a fallback: an answer from the host always wins. Kept true to the
/// game's tags; corrections go in [`SCALING_OVERRIDES`], which beat both.
///
/// The last column is the reach of the basic attack in range units (the
/// game's value over 1000), which the host does not report. The `Range` and
/// `Melee` tags are left out of the flags: the reach says the same and more.
const VANILLA: &[(&str, u8, Class, usize)] = &[
    ("alchemist", AP | CC, Class::Magician, 60),
    ("android", AD | TANK | CC, Class::Melee, 25),
    ("archer", AD, Class::Range, 70),
    ("astrologer", AP | CC, Class::Magician, 60),
    ("bard", AP, Class::Util, 80),
    ("barrier_magician", AP | SHIELD, Class::Util, 60),
    ("berserker", AD, Class::Melee, 25),
    ("bomber", AD, Class::Range, 60),
    ("boomerang_hunter", AD, Class::Range, 60),
    ("cavalry_knight", AD | CC, Class::Melee, 27),
    ("chef", AP | TANK | HEAL, Class::Util, 25),
    ("circus_blade", AD, Class::Assassin, 23),
    ("clown", AD, Class::Assassin, 40),
    ("crossbowman", AD, Class::Range, 60),
    ("dancer", AD, Class::Range, 60),
    ("dark_mage", AP | CC, Class::Magician, 60),
    ("demon", AD | CC, Class::Assassin, 23),
    ("dokkaebi", AD | TANK | SHIELD, Class::Melee, 25),
    ("druid", AP, Class::Magician, 60),
    ("dual_blader", AD | CC, Class::Melee, 25),
    ("enchanter", AP, Class::Util, 60),
    ("executioner", AD | CC, Class::Melee, 25),
    ("exorcist", AD | TANK, Class::Util, 25),
    ("fighter", AD | TANK | CC, Class::Melee, 23),
    ("gambler", AD, Class::Range, 70),
    ("ghost", AD, Class::Assassin, 23),
    ("guardian_spirit", AP | HEAL | SHIELD, Class::Util, 60),
    ("gunner", AD, Class::Range, 50),
    ("hammerer", AD | TANK | CC, Class::Melee, 25),
    ("harpooner", AD | CC, Class::Range, 60),
    ("hitman", AD | CC, Class::Assassin, 40),
    ("hunter", AD, Class::Assassin, 23),
    ("ice_mage", AP | CC, Class::Magician, 60),
    ("illusionist", AP | CC, Class::Magician, 60),
    ("inquisitor", AD, Class::Assassin, 25),
    ("jiangshi", AD | TANK | CC, Class::Melee, 25),
    ("knight", AD | TANK | SHIELD | CC, Class::Melee, 25),
    ("lancer", AD, Class::Melee, 30),
    ("lightning_mage", AP | CC, Class::Magician, 60),
    ("magic_knight", AD | AP | CC, Class::Melee, 26),
    ("monk", AP | TANK | HEAL | SHIELD | CC, Class::Util, 23),
    ("necromancer", AP, Class::Magician, 60),
    ("nightmare", AD, Class::Assassin, 23),
    ("ninja", AD, Class::Assassin, 23),
    ("ogre", AD | TANK | CC, Class::Melee, 28),
    ("plague_doctor", AD | TANK, Class::Util, 25),
    ("poison_dart_hunter", AD, Class::Range, 50),
    ("pole_warrior", AD | CC, Class::Melee, 30),
    ("priest", AP | HEAL | SHIELD, Class::Util, 60),
    ("prisoner", AD | TANK | CC, Class::Melee, 28),
    ("pyromancer", AP, Class::Magician, 60),
    ("pythoness", AP | HEAL, Class::Util, 60),
    ("sand_mage", AP | CC, Class::Magician, 60),
    ("shadowmancer", AP | CC, Class::Magician, 60),
    ("shield_bearer", AD | TANK | SHIELD | CC, Class::Melee, 25),
    ("siege_breaker", AD | TANK, Class::Melee, 25),
    ("soldier", AD, Class::Range, 60),
    ("spellbreaker", AD | AP | CC, Class::Melee, 28),
    ("spirit_caller", AP | HEAL, Class::Util, 60),
    ("strongman", AD | TANK | SHIELD | CC, Class::Melee, 28),
    ("swordman", AD, Class::Melee, 25),
    ("taoist", AP | CC, Class::Util, 60),
    ("vampire", AP | HEAL, Class::Magician, 25),
    ("voodoo_shaman", AP | CC, Class::Magician, 60),
    ("werewolf", AD | HEAL | CC, Class::Assassin, 25),
    ("whip_master", AD, Class::Range, 40),
    ("white_mage", AP, Class::Magician, 60),
    ("wind_mage", AP | CC, Class::Magician, 60),
];

/// The base game's champions with an ability that taunts ([`Immobilize::taunt`]).
/// Knight and Shield Bearer immobilize with nothing else. Sorted, for the
/// binary search.
const VANILLA_TAUNTERS: &[&str] = &["illusionist", "knight", "prisoner", "shield_bearer"];

/// The base game's champions with an ability that immobilizes some other way
/// ([`Immobilize::other`]). Both lists are read off each kit's parameters in
/// `setting/champion_info` and its skill text (0.6.2). Not the `CC` tag: four
/// tagged champions only slow, silence or disarm (Alchemist, Astrologer,
/// Taoist, Voodoo Shaman), and thirteen with a stun, root, knockback, fear or
/// charm carry no tag at all. Werewolf is here for the pull among its charge's
/// parameters, though its text speaks only of the slow. Sorted, for the binary
/// search.
const VANILLA_IMMOBILIZERS: &[&str] = &[
    "android",
    "archer",
    "barrier_magician",
    "berserker",
    "bomber",
    "cavalry_knight",
    "circus_blade",
    "dark_mage",
    "demon",
    "dokkaebi",
    "druid",
    "dual_blader",
    "executioner",
    "fighter",
    "gambler",
    "hammerer",
    "harpooner",
    "hitman",
    "ice_mage",
    "illusionist",
    "inquisitor",
    "jiangshi",
    "lancer",
    "lightning_mage",
    "magic_knight",
    "monk",
    "ogre",
    "pole_warrior",
    "prisoner",
    "sand_mage",
    "shadowmancer",
    "spellbreaker",
    "spirit_caller",
    "strongman",
    "werewolf",
    "whip_master",
    "white_mage",
    "wind_mage",
];

/// How long each of the base game's champions immobilizes for in all, in
/// ticks ([`Immobilize::ticks`]): every immobilizing parameter of its
/// abilities and its ultimate added up, from `setting/champion_info` and the
/// champions that are data kits of their own (0.6.3; the comment says which
/// ability holds what). A champion not listed has none with a figure on it.
/// Three have an immobilize the data gives no length for, which adds nothing:
/// Archer's stun ("briefly"), Executioner's grab and Harpooner's pull. Knight's
/// taunt is its skill's own `tick`. Sorted, for the binary search.
const VANILLA_IMMOBILIZE_TICKS: &[(&str, usize)] = &[
    ("android", 60),          // skill2 60
    ("barrier_magician", 60), // ult 60
    ("berserker", 30),        // skill2 30
    ("bomber", 90),           // skill2 90
    ("cavalry_knight", 60),   // skill 60
    ("circus_blade", 12),     // ult 12
    ("dark_mage", 60),        // skill 60
    ("demon", 60),            // ult 60
    ("dokkaebi", 60),         // ult 60
    ("druid", 8),             // ult 8
    ("dual_blader", 120),     // skill 60 + skill2 60
    ("fighter", 120),         // skill 60 + ult 60
    ("gambler", 36),          // ult 36
    ("hammerer", 150),        // skill 60 + ult 90
    ("hitman", 45),           // skill 45
    ("ice_mage", 105),        // skill 30 + skill2 60 + ult 15
    ("illusionist", 120),     // skill 60 + skill2 60
    ("inquisitor", 60),       // ult 60
    ("jiangshi", 60),         // skill 60
    ("knight", 60),           // skill 60
    ("lancer", 70),           // skill 30 + ult 40
    ("lightning_mage", 30),   // skill2 30
    ("magic_knight", 10),     // skill2 10
    ("monk", 60),             // skill2 60
    ("ogre", 60),             // skill2 60
    ("pole_warrior", 60),     // ult 60
    ("prisoner", 90),         // skill2 30 + ult 60
    ("sand_mage", 12),        // ult 12
    ("shadowmancer", 42),     // skill 42
    ("shield_bearer", 120),   // ult 120
    ("spellbreaker", 60),     // ult 60
    ("spirit_caller", 45),    // ult 45
    ("strongman", 132),       // skill 30 + skill2 12 + ult 90
    ("werewolf", 15),         // skill2 15
    ("whip_master", 10),      // skill2 10
    ("white_mage", 60),       // ult 60
    ("wind_mage", 40),        // ult 40
];

/// The base game's champions with a basic ability that heals, shields or
/// buffs an ally ([`ChampionTraits::aids`]), read off each kit's skill text
/// (0.6.2). Not the `Heal` and `Shield` tags: Vampire, Werewolf and Dokkaebi
/// carry one for what they do to themselves, and Bard, Enchanter, Exorcist
/// and Plague Doctor aid allies all day with neither. An ultimate alone does
/// not count (Android's shield, Strongman's throw, Knight's ward): the items
/// this is for want it done every few seconds. Sorted, for the binary
/// search.
const VANILLA_ALLY_AIDS: &[&str] = &[
    "bard",
    "barrier_magician",
    "chef",
    "enchanter",
    "exorcist",
    "guardian_spirit",
    "monk",
    "plague_doctor",
    "priest",
    "pythoness",
    "shield_bearer",
    "spirit_caller",
];

/// How a `.data_champion` kit spells one kind of crowd control: the effect
/// types that apply it, and the parameters a `Native` effect borrowed from the
/// base game names it by.
struct KitSign {
    effects: &'static [&'static str],
    params: &'static [&'static str],
}

const TAUNT_SIGN: KitSign = KitSign {
    effects: &["Taunt"],
    params: &["taunt_duration"],
};

/// Every immobilize but the taunt.
const IMMOBILIZE_SIGN: KitSign = KitSign {
    effects: &[
        "Airborne",
        "Bind",
        "Charm",
        "Fear",
        "Grab",
        "Knockback",
        "Pull",
        "Stun",
    ],
    params: &[
        "airborne",
        "airborne_tick",
        "airborne_time",
        "bind",
        "bind_duration",
        "bind_tick",
        "charm_duration",
        "fear_duration",
        "fear_tick",
        "grab",
        "pull_speed",
        "stun",
        "stun_duration",
    ],
};

/// Whether anything in this part of a `.data_champion` kit does what `sign`
/// spells to an enemy. What a kit does to its own caster (`WithSelf`) holds no
/// one.
fn kit_shows(kit: &serde_json::Value, sign: &KitSign) -> bool {
    match kit {
        serde_json::Value::Object(fields) => {
            let effect = fields.get("type").and_then(serde_json::Value::as_str);
            if effect == Some("WithSelf") {
                return false;
            }
            effect.is_some_and(|effect| sign.effects.contains(&effect))
                || fields.iter().any(|(name, value)| {
                    let set = value.as_u64().is_some_and(|amount| amount > 0)
                        || value.as_bool() == Some(true);
                    (set && sign.params.contains(&name.as_str())) || kit_shows(value, sign)
                })
        }
        serde_json::Value::Array(items) => items.iter().any(|item| kit_shows(item, sign)),
        _ => false,
    }
}

/// The parameters a kit names an immobilize's length by, in ticks, and the
/// fields an immobilizing effect gives its own in. The signs' other
/// parameters are speeds and switches.
const TICK_PARAMS: &[&str] = &[
    "airborne",
    "airborne_tick",
    "airborne_time",
    "bind",
    "bind_duration",
    "bind_tick",
    "charm_duration",
    "fear_duration",
    "fear_tick",
    "knockback_tick",
    "pull_time",
    "stun",
    "stun_duration",
    "taunt_duration",
];
const TICK_FIELDS: &[&str] = &["duration", "tick"];

/// How long everything in this part of a `.data_champion` kit immobilizes an
/// enemy for, added up, in ticks ([`Immobilize::ticks`]). What a kit does to
/// its own caster (`WithSelf`) holds no one.
fn kit_ticks(kit: &serde_json::Value) -> usize {
    let ticks = |value: &serde_json::Value| value.as_u64().unwrap_or(0) as usize;
    match kit {
        serde_json::Value::Object(fields) => {
            let effect = fields.get("type").and_then(serde_json::Value::as_str);
            if effect == Some("WithSelf") {
                return 0;
            }
            let immobilizes = effect.is_some_and(|effect| {
                TAUNT_SIGN.effects.contains(&effect) || IMMOBILIZE_SIGN.effects.contains(&effect)
            });
            let own = if immobilizes {
                TICK_FIELDS
                    .iter()
                    .filter_map(|field| fields.get(*field))
                    .map(ticks)
                    .max()
                    .unwrap_or(0)
            } else {
                0
            };
            own + fields
                .iter()
                .map(|(name, value)| {
                    if TICK_PARAMS.contains(&name.as_str()) {
                        ticks(value)
                    } else {
                        kit_ticks(value)
                    }
                })
                .sum::<usize>()
        }
        serde_json::Value::Array(items) => items.iter().map(kit_ticks).sum(),
        _ => 0,
    }
}

/// Whether this part of a kit runs a `Native` effect: code in the mod's own
/// DLL, which may stun or root with nothing in the file to show for it (another
/// mod's Brand, tagged `CC`, does exactly that).
fn runs_native_code(kit: &serde_json::Value) -> bool {
    match kit {
        serde_json::Value::Object(fields) => {
            fields.get("type").and_then(serde_json::Value::as_str) == Some("Native")
                || fields.values().any(runs_native_code)
        }
        serde_json::Value::Array(items) => items.iter().any(runs_native_code),
        _ => false,
    }
}

/// Whether this part of a `.data_champion` kit is aimed at, or applied to, an
/// ally other than its caster: a `casting_target`, `applied_target` or
/// `target` that names allies. `AllyOnlySelf` is the caster alone.
fn aims_at_allies(kit: &serde_json::Value) -> bool {
    const TARGETS: [&str; 3] = ["casting_target", "applied_target", "target"];
    match kit {
        serde_json::Value::Object(fields) => fields.iter().any(|(name, value)| {
            let allies = value
                .as_str()
                .is_some_and(|target| target.starts_with("Ally") && target != "AllyOnlySelf");
            (allies && TARGETS.contains(&name.as_str())) || aims_at_allies(value)
        }),
        serde_json::Value::Array(items) => items.iter().any(aims_at_allies),
        _ => false,
    }
}

/// The `.data_champion` effects that heal, shield or buff whoever the ability
/// is applied to. `AddCasterBuff` is not one: it is the caster's own.
const AID_EFFECTS: [&str; 4] = ["Heal", "Shield", "AddBuff", "AddStatScaledBuff"];

/// Whether anything in this part of a `.data_champion` kit heals, shields or
/// buffs someone other than its caster: one of [`AID_EFFECTS`] not marked for
/// the caster (`heal_type` or `target` of `Caster`).
fn shows_aid(kit: &serde_json::Value) -> bool {
    match kit {
        serde_json::Value::Object(fields) => {
            let effect = fields.get("type").and_then(serde_json::Value::as_str);
            let own = ["heal_type", "target"].iter().any(|name| {
                fields.get(*name).and_then(serde_json::Value::as_str) == Some("Caster")
            });
            (effect.is_some_and(|effect| AID_EFFECTS.contains(&effect)) && !own)
                || fields.values().any(shows_aid)
        }
        serde_json::Value::Array(items) => items.iter().any(shows_aid),
        _ => false,
    }
}

/// Champions other mods add, by id, from the `tags`, `category`, `attack.range`
/// and abilities in their `.data_champion` files. Filled once by [`load_mod_champions`].
static MOD_CHAMPIONS: OnceLock<HashMap<String, ChampionTraits>> = OnceLock::new();

/// Steam app id, which names the game's Workshop content folder.
const STEAM_APP_ID: &str = "3009300";

/// How far below a mods root a `.data_champion` file may sit: `<mod>/champion/`
/// is the usual place, and a Workshop item may nest its mod one folder deeper.
const SCAN_DEPTH: usize = 4;

/// Reads every `.data_champion` file under the game's `mods` folder and its
/// Workshop content folder into [`MOD_CHAMPIONS`]. Called once from `init`, on
/// the main thread, because the build paths that read the result run on sim
/// workers, where file IO has no business. About 50 files and under 1 MB with
/// the usual champion mods installed.
///
/// Disabled mods are read too. That costs nothing, because a champion no
/// enabled mod adds never reaches a build, and ids are unique across mods.
pub(crate) fn load_mod_champions() {
    MOD_CHAMPIONS.get_or_init(|| {
        let mut found = HashMap::new();
        for root in mod_roots() {
            scan(&root, SCAN_DEPTH, &mut found);
        }
        found
    });
}

fn mod_roots() -> Vec<PathBuf> {
    let Some(game) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    else {
        return Vec::new();
    };
    let mut roots = vec![game.join("mods")];
    // steamapps/common/<game> -> steamapps/workshop/content/<app id>
    if let Some(steamapps) = game.parent().and_then(Path::parent) {
        roots.push(steamapps.join("workshop").join("content").join(STEAM_APP_ID));
    }
    roots
}

fn scan(dir: &Path, depth: usize, found: &mut HashMap<String, ChampionTraits>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.is_dir() {
            if depth > 0 {
                scan(&path, depth - 1, found);
            }
        } else if path.extension().is_some_and(|ext| ext == "data_champion") {
            if let Some((id, traits)) = read_champion(&path) {
                found.entry(id).or_insert(traits);
            }
        }
    }
}

/// The fields of a `.data_champion` file this needs; serde skips the rest.
#[derive(serde::Deserialize)]
struct ChampionFile {
    id: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    tags: Vec<String>,
    /// The basic attack, of which only `range` is read. Left untyped so that an
    /// attack written some other way costs the champion its reach, not the
    /// rest of what the file says.
    #[serde(default)]
    attack: serde_json::Value,
    /// Everything else, the abilities among it: read for what they immobilize
    /// with. The basic attack is left out, since Imperial Mandate only answers
    /// to abilities.
    #[serde(flatten)]
    rest: HashMap<String, serde_json::Value>,
}

fn read_champion(path: &Path) -> Option<(String, ChampionTraits)> {
    let text = std::fs::read_to_string(path).ok()?;
    let file: ChampionFile = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let flags = flags_of(file.tags.iter().map(String::as_str));
    let attack_range = file
        .attack
        .get("range")
        .and_then(|range| range.as_u64().or_else(|| range.as_f64().map(|range| range as u64)))
        .map(|range| range as usize);
    let shows = |sign: &KitSign| file.rest.values().any(|part| kit_shows(part, sign));
    let (taunt, other) = (shows(&TAUNT_SIGN), shows(&IMMOBILIZE_SIGN));
    // Native code may immobilize with nothing in the file to show for it. A kit
    // that shows nothing at all is then not known to have none, so its `CC` tag
    // decides; one that shows only a taunt is taken to have more.
    let native = file.rest.values().any(runs_native_code);
    let immobilize = (taunt || other || !native).then_some(Immobilize {
        taunt,
        other: other || native,
        // What the file shows, added up. Native code may add to it with
        // nothing to show; a kit that shows none at all and runs some is not
        // known either way (`None`, above), and its `CC` tag decides.
        ticks: file.rest.values().map(kit_ticks).sum(),
    });
    // Aiding allies takes both: a basic ability aimed at one, and a heal, shield
    // or buff in it. Aimed at an ally alone is not enough (another mod's Dummy
    // sets a decoy down beside one, and was handed Dream Maker for it). The
    // same doubt as above over native code, which may aid with nothing in the
    // file to show: a kit that runs some and shows no aid is not known either
    // way, so its `Heal` and `Shield` tags decide.
    let aids = ["skill", "skill2"]
        .iter()
        .filter_map(|part| file.rest.get(*part))
        .any(|part| aims_at_allies(part) && shows_aid(part));
    let aids = (aids || !native).then_some(aids);
    let traits = ChampionTraits::from_flags(
        flags,
        Class::from_name(&file.category),
        attack_range,
        immobilize,
        aids,
    );
    Some((file.id, traits))
}

/// What is known about `champion` without the host: [`VANILLA`] for the base
/// game, then [`MOD_CHAMPIONS`].
fn fallback(champion: &str) -> Option<ChampionTraits> {
    vanilla(champion).or_else(|| {
        MOD_CHAMPIONS
            .get()?
            .get(champion)
            .copied()
    })
}

/// Settled answers, by champion key: the host's where it gave one, otherwise
/// the [`fallback`] or `None`. Misses are settled too, so a champion the
/// host does not know costs one lookup here rather than a trip to [`PENDING`]
/// on every item the build hook scores.
static LEARNED: RwLock<Option<HashMap<String, Option<ChampionTraits>>>> = RwLock::new(None);

/// Keys already put to the host, answered or not, so a champion the host does
/// not know is asked once rather than every frame.
static ASKED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Keys a build path asked about before the client had learned them. The build
/// paths are not on the client, so they leave the question here for [`learn`].
static PENDING: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Champions whose `AD`/`AP` tags misstate what their kit scales with, and
/// what Smart Builds treats them as instead. These win over every source,
/// the host included, which is why they are not simply edits to [`VANILLA`].
///
/// Magic Knight is tagged both, but only its basic attack is physical: both
/// abilities deal magic damage off Ability Power, and its ultimate's attack
/// speed scales with Ability Power too. Left hybrid, rule 5 never touched it
/// and the engine had it building full AD (2026-09-26, in a match where it
/// dealt ~20k damage, nearly all of it magic).
const SCALING_OVERRIDES: &[(&str, Scaling)] = &[("magic_knight", Scaling::Ap)];

/// What is known about `champion`, or `None` when nothing is — which the rules
/// read as "no restriction". A miss is queued for the next [`learn`].
/// [`SCALING_OVERRIDES`] apply on top of whatever answered.
pub(crate) fn traits(champion: &str) -> Option<ChampionTraits> {
    let mut traits = looked_up(champion);
    if let Some(&(_, scaling)) = SCALING_OVERRIDES.iter().find(|(key, _)| *key == champion) {
        traits.get_or_insert_with(ChampionTraits::default).scaling = Some(scaling);
    }
    traits
}

/// [`traits`] as the sources give it: the host's settled answer, else the
/// [`fallback`].
fn looked_up(champion: &str) -> Option<ChampionTraits> {
    if let Some(settled) = LEARNED
        .read()
        .ok()
        .and_then(|learned| learned.as_ref()?.get(champion).copied())
    {
        return settled;
    }
    if let Ok(mut pending) = PENDING.lock() {
        pending
            .get_or_insert_with(HashSet::new)
            .insert(champion.to_string());
    }
    fallback(champion)
}

fn vanilla(champion: &str) -> Option<ChampionTraits> {
    VANILLA
        .binary_search_by_key(&champion, |(key, _, _, _)| key)
        .ok()
        .map(|index| {
            let (_, flags, class, attack_range) = VANILLA[index];
            let attack_range = attack_range * crate::DISTANCE_UNITS_PER_RANGE;
            let immobilize = Immobilize {
                taunt: VANILLA_TAUNTERS.binary_search(&champion).is_ok(),
                other: VANILLA_IMMOBILIZERS.binary_search(&champion).is_ok(),
                ticks: VANILLA_IMMOBILIZE_TICKS
                    .binary_search_by_key(&champion, |(key, _)| key)
                    .map_or(0, |index| VANILLA_IMMOBILIZE_TICKS[index].1),
            };
            let aids = VANILLA_ALLY_AIDS.binary_search(&champion).is_ok();
            ChampionTraits::from_flags(
                flags,
                Some(class),
                Some(attack_range),
                Some(immobilize),
                Some(aids),
            )
        })
}

/// Asks the host about every champion not asked about yet: the whole roster
/// once the first match has recorded it, plus any champion a build path has
/// queued. Cheap when there is nothing new, so it runs every client frame.
pub(crate) fn learn(ctx: &StableClient<'_>) {
    // The roster only grows, and copying it every frame to find nothing new
    // is the common case, so its length decides whether it is read at all.
    static ROSTER_SEEN: AtomicUsize = AtomicUsize::new(0);
    let roster_len = crate::build_config::champion_roster_len();
    let mut wanted: Vec<String> = if ROSTER_SEEN.swap(roster_len, Ordering::Relaxed) != roster_len {
        crate::build_config::champion_roster()
    } else {
        Vec::new()
    };
    if let Ok(mut pending) = PENDING.lock() {
        if let Some(pending) = pending.as_mut() {
            wanted.extend(pending.drain());
        }
    }
    let fresh: Vec<String> = {
        let Ok(mut asked) = ASKED.lock() else {
            return;
        };
        let asked = asked.get_or_insert_with(HashSet::new);
        wanted
            .into_iter()
            .filter(|key| asked.insert(key.clone()))
            .collect()
    };
    if fresh.is_empty() {
        return;
    }

    let mut answered = Vec::new();
    let mut unanswered = Vec::new();
    for key in fresh {
        match ctx.champion_brief(&key) {
            Some(brief) => {
                let has = |tag: ChampionTagV1| brief.tags.contains(&tag);
                let class = brief.category.map(Class::from_category);
                let traits = ChampionTraits {
                    scaling: scaling_of(has(ChampionTagV1::Ad), has(ChampionTagV1::Ap)),
                    class,
                    tank: has(ChampionTagV1::Tank),
                    sustains: has(ChampionTagV1::Heal) || has(ChampionTagV1::Shield),
                    cc: has(ChampionTagV1::Cc),
                    // The brief says neither what the kit immobilizes with, nor
                    // whether it aids allies, nor how far the attack reaches, so
                    // the fallbacks' answers stand where they have one.
                    aids: fallback(&key).and_then(|known| known.aids),
                    immobilize: fallback(&key).and_then(|known| known.immobilize),
                    ranged: fallback(&key).and_then(|known| known.ranged).or_else(|| {
                        let (range, melee) = (ChampionTagV1::Range, ChampionTagV1::Melee);
                        ranged_of(None, has(range), has(melee), class)
                    }),
                };
                answered.push((key, traits));
            }
            None => unanswered.push(key),
        }
    }

    if let Ok(mut learned) = LEARNED.write() {
        let learned = learned.get_or_insert_with(HashMap::new);
        for (key, traits) in &answered {
            learned.insert(key.clone(), Some(*traits));
        }
        for key in &unanswered {
            learned.insert(key.clone(), fallback(key));
        }
    }
}

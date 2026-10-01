//! What Smart Builds needs to know about a champion: what its damage scales
//! with, to pick its boots, its class and whether it tanks or keeps allies
//! alive, whether its kit has crowd control (a support that does builds
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
//! reach of the basic attack itself. See [`ranged_of`].

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChampionTraits {
    /// `None` for a champion tagged neither way, which the rules leave alone.
    pub scaling: Option<Scaling>,
    /// `None` when the source did not say.
    pub class: Option<Class>,
    /// Tagged `Tank`.
    pub tank: bool,
    /// Tagged `Heal` or `Shield`: it keeps allies alive.
    pub sustains: bool,
    /// Tagged `CC`: its kit slows, stuns or otherwise holds enemies.
    pub cc: bool,
    /// Whether its basic attack reaches past [`MELEE_ATTACK_RANGE`]. `None`
    /// when nothing says, which the rules leave alone.
    pub ranged: Option<bool>,
}

impl ChampionTraits {
    /// `attack_range` is the reach of the basic attack in world units, where
    /// the source states one.
    fn from_flags(flags: u8, class: Option<Class>, attack_range: Option<usize>) -> Self {
        Self {
            scaling: scaling_of(flags & AD != 0, flags & AP != 0),
            class,
            tank: flags & TANK != 0,
            sustains: flags & (HEAL | SHIELD) != 0,
            cc: flags & CC != 0,
            ranged: ranged_of(attack_range, flags & RANGE != 0, flags & MELEE != 0, class),
        }
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

/// Champions other mods add, by id, from the `tags`, `category` and
/// `attack.range` in their `.data_champion` files. Filled once by [`load_mod_champions`].
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
    let traits = ChampionTraits::from_flags(flags, Class::from_name(&file.category), attack_range);
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
            ChampionTraits::from_flags(flags, Some(class), Some(attack_range))
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
                    // The brief has no attack range, so the fallbacks' answer
                    // stands where they have one.
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

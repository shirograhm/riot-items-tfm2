//! What Smart Builds needs to know about a champion: what its damage scales
//! with, and, to pick its boots, its class and whether it tanks or keeps allies
//! alive.
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
}

impl ChampionTraits {
    fn from_flags(flags: u8, class: Option<Class>) -> Self {
        Self {
            scaling: scaling_of(flags & AD != 0, flags & AP != 0),
            class,
            tank: flags & TANK != 0,
            sustains: flags & (HEAL | SHIELD) != 0,
        }
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
                _ => 0,
            }
    })
}

/// The base game's champions, from `setting/champion_info` (`category` and `tags`),
/// including the eight it ships under `mod_champions`. Only a
/// fallback: an answer from the host always wins.
const VANILLA: &[(&str, u8, Class)] = &[
    ("alchemist", AP, Class::Magician),
    ("android", AD | TANK, Class::Melee),
    ("archer", AD, Class::Range),
    ("astrologer", AP, Class::Magician),
    ("bard", AP, Class::Util),
    ("barrier_magician", AP | SHIELD, Class::Util),
    ("berserker", AD, Class::Melee),
    ("bomber", AD, Class::Range),
    ("boomerang_hunter", AD, Class::Range),
    ("cavalry_knight", AD, Class::Melee),
    ("chef", AP | TANK | HEAL, Class::Util),
    ("circus_blade", AD, Class::Assassin),
    ("clown", AD, Class::Assassin),
    ("crossbowman", AD, Class::Range),
    ("dancer", AD, Class::Range),
    ("dark_mage", AP, Class::Magician),
    ("demon", AD, Class::Assassin),
    ("dokkaebi", AD | TANK | SHIELD, Class::Melee),
    ("druid", AP, Class::Magician),
    ("dual_blader", AD, Class::Melee),
    ("enchanter", AP, Class::Util),
    ("executioner", AD, Class::Melee),
    ("exorcist", AD | TANK, Class::Util),
    ("fighter", AD | TANK, Class::Melee),
    ("gambler", AD, Class::Range),
    ("ghost", AD, Class::Assassin),
    ("guardian_spirit", AP | HEAL | SHIELD, Class::Util),
    ("gunner", AD, Class::Range),
    ("hammerer", AD | TANK, Class::Melee),
    ("harpooner", AD, Class::Range),
    ("hitman", AD, Class::Assassin),
    ("hunter", AD, Class::Assassin),
    ("ice_mage", AP, Class::Magician),
    ("illusionist", AP, Class::Magician),
    ("inquisitor", AD, Class::Assassin),
    ("jiangshi", AD | TANK, Class::Melee),
    ("knight", AD | TANK | SHIELD, Class::Melee),
    ("lancer", AD, Class::Melee),
    ("lightning_mage", AP, Class::Magician),
    ("magic_knight", AD | AP, Class::Melee),
    ("monk", AP | TANK | HEAL | SHIELD, Class::Util),
    ("necromancer", AP, Class::Magician),
    ("nightmare", AD, Class::Assassin),
    ("ninja", AD, Class::Assassin),
    ("ogre", AD | TANK, Class::Melee),
    ("plague_doctor", AD | TANK, Class::Util),
    ("poison_dart_hunter", AD, Class::Range),
    ("pole_warrior", AD, Class::Melee),
    ("priest", AP | HEAL | SHIELD, Class::Util),
    ("prisoner", AD | TANK, Class::Melee),
    ("pyromancer", AP, Class::Magician),
    ("pythoness", AP | HEAL, Class::Util),
    ("sand_mage", AP, Class::Magician),
    ("shadowmancer", AP, Class::Magician),
    ("shield_bearer", AD | TANK | SHIELD, Class::Melee),
    ("siege_breaker", AD | TANK, Class::Melee),
    ("soldier", AD, Class::Range),
    ("spellbreaker", AD | AP, Class::Melee),
    ("spirit_caller", AP | HEAL, Class::Util),
    ("strongman", AD | TANK | SHIELD, Class::Melee),
    ("swordman", AD, Class::Melee),
    ("taoist", AP, Class::Util),
    ("vampire", AP | HEAL, Class::Magician),
    ("voodoo_shaman", AP, Class::Magician),
    ("werewolf", AD | HEAL, Class::Assassin),
    ("whip_master", AD, Class::Range),
    ("white_mage", AP, Class::Magician),
    ("wind_mage", AP, Class::Magician),
];

/// Champions other mods add, by id, from the `tags` and `category` in their
/// `.data_champion` files. Filled once by [`load_mod_champions`].
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
}

fn read_champion(path: &Path) -> Option<(String, ChampionTraits)> {
    let text = std::fs::read_to_string(path).ok()?;
    let file: ChampionFile = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let flags = flags_of(file.tags.iter().map(String::as_str));
    Some((file.id, ChampionTraits::from_flags(flags, Class::from_name(&file.category))))
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

/// What is known about `champion`, or `None` when nothing is — which the rules
/// read as "no restriction". A miss is queued for the next [`learn`].
pub(crate) fn traits(champion: &str) -> Option<ChampionTraits> {
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
        .binary_search_by_key(&champion, |(key, _, _)| key)
        .ok()
        .map(|index| {
            let (_, flags, class) = VANILLA[index];
            ChampionTraits::from_flags(flags, Some(class))
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
                let traits = ChampionTraits {
                    scaling: scaling_of(has(ChampionTagV1::Ad), has(ChampionTagV1::Ap)),
                    class: brief.category.map(Class::from_category),
                    tank: has(ChampionTagV1::Tank),
                    sustains: has(ChampionTagV1::Heal) || has(ChampionTagV1::Shield),
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

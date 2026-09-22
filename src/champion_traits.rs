//! What Smart Builds needs to know about a champion: what its damage scales
//! with.
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

use mod_api_stable::{ChampionTagV1, StableClient};

/// What a champion's damage scales with, from its `AD`/`AP` tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scaling {
    Ad,
    Ap,
    Hybrid,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChampionTraits {
    /// `None` for a champion tagged neither way, which the rules leave alone.
    pub scaling: Option<Scaling>,
}

impl ChampionTraits {
    fn from_flags(flags: u8) -> Self {
        Self {
            scaling: scaling_of(flags & AD != 0, flags & AP != 0),
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

/// The base game's champions, from `setting/champion_info` (`tags`),
/// including the eight it ships under `mod_champions`. Only a
/// fallback: an answer from the host always wins.
const VANILLA: &[(&str, u8)] = &[
    ("alchemist", AP),
    ("android", AD),
    ("archer", AD),
    ("astrologer", AP),
    ("bard", AP),
    ("barrier_magician", AP),
    ("berserker", AD),
    ("bomber", AD),
    ("boomerang_hunter", AD),
    ("cavalry_knight", AD),
    ("chef", AP),
    ("circus_blade", AD),
    ("clown", AD),
    ("crossbowman", AD),
    ("dancer", AD),
    ("dark_mage", AP),
    ("demon", AD),
    ("dokkaebi", AD),
    ("druid", AP),
    ("dual_blader", AD),
    ("enchanter", AP),
    ("executioner", AD),
    ("exorcist", AD),
    ("fighter", AD),
    ("gambler", AD),
    ("ghost", AD),
    ("guardian_spirit", AP),
    ("gunner", AD),
    ("hammerer", AD),
    ("harpooner", AD),
    ("hitman", AD),
    ("hunter", AD),
    ("ice_mage", AP),
    ("illusionist", AP),
    ("inquisitor", AD),
    ("jiangshi", AD),
    ("knight", AD),
    ("lancer", AD),
    ("lightning_mage", AP),
    ("magic_knight", AD | AP),
    ("monk", AP),
    ("necromancer", AP),
    ("nightmare", AD),
    ("ninja", AD),
    ("ogre", AD),
    ("plague_doctor", AD),
    ("poison_dart_hunter", AD),
    ("pole_warrior", AD),
    ("priest", AP),
    ("prisoner", AD),
    ("pyromancer", AP),
    ("pythoness", AP),
    ("sand_mage", AP),
    ("shadowmancer", AP),
    ("shield_bearer", AD),
    ("siege_breaker", AD),
    ("soldier", AD),
    ("spellbreaker", AD | AP),
    ("spirit_caller", AP),
    ("strongman", AD),
    ("swordman", AD),
    ("taoist", AP),
    ("vampire", AP),
    ("voodoo_shaman", AP),
    ("werewolf", AD),
    ("whip_master", AD),
    ("white_mage", AP),
    ("wind_mage", AP),
];

/// Champions other mods add, by id, as `AD`/`AP` flags from the `tags` in
/// their `.data_champion` files. Filled once by [`load_mod_champions`].
static MOD_CHAMPIONS: OnceLock<HashMap<String, u8>> = OnceLock::new();

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

fn scan(dir: &Path, depth: usize, found: &mut HashMap<String, u8>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.is_dir() {
            if depth > 0 {
                scan(&path, depth - 1, found);
            }
        } else if path.extension().is_some_and(|ext| ext == "data_champion") {
            if let Some((id, flags)) = read_champion(&path) {
                found.entry(id).or_insert(flags);
            }
        }
    }
}

/// The two fields of a `.data_champion` file this needs; serde skips the rest.
#[derive(serde::Deserialize)]
struct ChampionFile {
    id: String,
    #[serde(default)]
    tags: Vec<String>,
}

fn read_champion(path: &Path) -> Option<(String, u8)> {
    let text = std::fs::read_to_string(path).ok()?;
    let file: ChampionFile = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let has = |tag: &str| file.tags.iter().any(|t| t.eq_ignore_ascii_case(tag));
    let flags = (if has("AD") { AD } else { 0 }) | (if has("AP") { AP } else { 0 });
    Some((file.id, flags))
}

/// What is known about `champion` without the host: [`VANILLA`] for the base
/// game, then [`MOD_CHAMPIONS`].
fn fallback(champion: &str) -> Option<ChampionTraits> {
    vanilla(champion).or_else(|| {
        MOD_CHAMPIONS
            .get()?
            .get(champion)
            .map(|&flags| ChampionTraits::from_flags(flags))
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
        .binary_search_by_key(&champion, |(key, _)| key)
        .ok()
        .map(|index| ChampionTraits::from_flags(VANILLA[index].1))
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
                let traits = ChampionTraits {
                    scaling: scaling_of(
                        brief.tags.contains(&ChampionTagV1::Ad),
                        brief.tags.contains(&ChampionTagV1::Ap),
                    ),
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

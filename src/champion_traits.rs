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
//! answer. [`VANILLA`] backs it up for the base game's champions, and a
//! champion neither source knows gets no restriction at all: a missing tag must
//! never cost a build an item. `champion_traits.txt` beside the DLL records how
//! many champions the host actually answered for.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, RwLock};

use mod_api_stable::{ChampionCategoryV1, ChampionTagV1, StableClient};

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

/// Settled answers, by champion key: the host's where it gave one, otherwise
/// the [`VANILLA`] entry or `None`. Misses are settled too, so a champion the
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
    vanilla(champion)
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
                answered.push((key, traits, brief.category, brief.tags));
            }
            None => unanswered.push(key),
        }
    }

    if let Ok(mut learned) = LEARNED.write() {
        let learned = learned.get_or_insert_with(HashMap::new);
        for (key, traits, _, _) in &answered {
            learned.insert(key.clone(), Some(*traits));
        }
        for key in &unanswered {
            learned.insert(key.clone(), vanilla(key));
        }
    }
    report(&answered, &unanswered);
}

/// Writes one batch to `champion_traits.txt`: how many champions the host
/// answered for, what it said, and which it did not know.
fn report(
    answered: &[(
        String,
        ChampionTraits,
        Option<ChampionCategoryV1>,
        Vec<ChampionTagV1>,
    )],
    unanswered: &[String],
) {
    use std::fmt::Write as _;
    use std::io::Write as _;

    let mut text = format!(
        "champion_brief: answered {} of {}\n",
        answered.len(),
        answered.len() + unanswered.len()
    );
    for (key, traits, category, tags) in answered {
        let _ = writeln!(
            text,
            "  {key}: {category:?} {tags:?} -> scaling={:?}",
            traits.scaling
        );
    }
    for key in unanswered {
        let known = if vanilla(key).is_some() {
            "vanilla fallback"
        } else {
            "no restriction"
        };
        let _ = writeln!(text, "  {key}: no answer ({known})");
    }

    // Fresh each session, appended to within one: the first batch is the
    // roster, later ones are champions the build paths queued.
    static STARTED: AtomicBool = AtomicBool::new(false);
    let first = !STARTED.swap(true, Ordering::Relaxed);
    let path = crate::config::mod_dir().join("champion_traits.txt");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(!first)
        .truncate(first)
        .open(path)
    {
        let _ = file.write_all(text.as_bytes());
    }
}

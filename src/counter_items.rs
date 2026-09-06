//! Comp-aware nudges for the AI's *default* item build: anti-heal against a
//! healing lineup, anti-shield against a shielding one — and only ever on a
//! champion whose damage the item is built for.
//!
//! When a champion has a configured build, `build_config` decides and this is
//! irrelevant. When it does not — the "no builds" case — `score_item` is the
//! only say the mod has in what the engine buys: the engine ranks every
//! candidate and the mod may push one up. This module supplies the push that
//! depends on who is on the *other* side.
//!
//! # Nothing here is a list of champions or items
//!
//! Every side of the decision is read from tags the game already keeps, so none
//! of it goes stale:
//!
//! - Champions carry [`ChampionTagV1::Heal`] / [`ChampionTagV1::Shield`] (the
//!   base sheet tags eight of each; `priest`, `monk` and `guardian_spirit` carry
//!   both) and [`ChampionTagV1::Ad`] / [`ChampionTagV1::Ap`] (all 68 carry one,
//!   `spellbreaker` and `magic_knight` carry both). Those tags are only
//!   reachable through `StableClient::champion_brief`, which is client-side,
//!   while the item-build hook runs with no client at all — hence [`prime`],
//!   which caches the table once from the client tick for the hook to read back.
//! - Items carry [`ItemTagV1::HealReduce`] / [`ItemTagV1::ShieldBreak`] for what
//!   they answer, and [`ItemTagV1::Ad`] / [`ItemTagV1::Ap`] for who should hold
//!   them. The mod's items already declare both in `tags()`: Executioner's
//!   Calling and Mortal Reminder are AD anti-heal, Oblivion Orb and
//!   Morellonomicon are AP anti-heal, Serpent's Fang is AD anti-shield.
//!   [`note_registered`] captures them at registration, the way
//!   `strategy_ui::note_final_item` captures categories.
//!
//! So a future item tagged `HealReduce` joins in without this file changing, and
//! is routed to AD or AP carriers by the damage tag it already declares.

use mod_api_stable::{ChampionTagV1, ItemTagV1, StableClient};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};

/// How much a matching counter item is worth on top of the engine's own score.
///
/// The engine's scale is not documented anywhere and cannot be read off the
/// hook, so this is the one number to tune if the AI over- or under-buys these:
/// it is deliberately larger than `item_build_hook::MOD_ITEM_SCORE_BONUS`,
/// which only has to lift a mod item into contention, while this has to beat
/// whatever the engine already preferred.
const COUNTER_BONUS: f32 = 1.0;

/// Multiplier once **two or more** enemies bring the thing being countered. One
/// healer is a reason to consider anti-heal; a whole healing comp is a reason to
/// rush it.
const STACKED_THREAT_MULTIPLIER: f32 = 1.5;

/// What a lineup brings, and symmetrically what an item answers.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
struct Threats {
    heal: bool,
    shield: bool,
}

impl Threats {
    fn any(&self) -> bool {
        self.heal || self.shield
    }
}

/// Which damage a champion deals, and which damage an item is built for.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
struct Damage {
    ad: bool,
    ap: bool,
}

impl Damage {
    fn from_champion(tags: &[ChampionTagV1]) -> Self {
        Self {
            ad: tags.contains(&ChampionTagV1::Ad),
            ap: tags.contains(&ChampionTagV1::Ap),
        }
    }

    fn from_item(tags: &[ItemTagV1]) -> Self {
        Self {
            ad: tags.contains(&ItemTagV1::Ad),
            ap: tags.contains(&ItemTagV1::Ap),
        }
    }

    fn declared(&self) -> bool {
        self.ad || self.ap
    }

    /// Whether an item for `self` belongs on a champion dealing `carrier`.
    ///
    /// An undeclared side means "no opinion" and passes: an item with neither
    /// tag is damage-agnostic, and a champion with neither tag (none in the base
    /// sheet, but a modded champion could be) must not be locked out of counter
    /// items entirely. Only two declared sides that *disagree* block — which is
    /// the whole point: Serpent's Fang and Executioner's Calling are AD, so they
    /// stop being offered to a pure-AP carrier, and Morellonomicon is AP, so it
    /// stops being offered to a pure-AD one. The two hybrids
    /// (`spellbreaker`, `magic_knight`) are tagged both and so take either.
    fn suits(&self, carrier: Damage) -> bool {
        if !self.declared() || !carrier.declared() {
            return true;
        }
        (self.ad && carrier.ad) || (self.ap && carrier.ap)
    }
}

// -- items ----------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct ItemProfile {
    counters: Threats,
    damage: Damage,
}

/// Registered mod items that carry a counter tag, filled during `init`.
static COUNTER_ITEMS: Mutex<Vec<(String, ItemProfile)>> = Mutex::new(Vec::new());

/// Read-side snapshot of [`COUNTER_ITEMS`]. Same pattern as
/// `strategy_ui::MOD_FINAL_SET`: the hook reads this on every candidate, and
/// registration is long finished by the time a match runs.
static COUNTER_ITEM_SET: OnceLock<Vec<(String, ItemProfile)>> = OnceLock::new();

/// Records an item's counter and damage tags. Called for every registered item —
/// base and radiant — from the registration macros in `lib.rs`; items with no
/// counter tag are dropped here rather than at the call site.
pub(crate) fn note_registered(key: &str, tags: &[ItemTagV1]) {
    let counters = Threats {
        heal: tags.contains(&ItemTagV1::HealReduce),
        shield: tags.contains(&ItemTagV1::ShieldBreak),
    };
    if !counters.any() {
        return;
    }
    if let Ok(mut items) = COUNTER_ITEMS.lock() {
        items.push((
            key.to_string(),
            ItemProfile {
                counters,
                damage: Damage::from_item(tags),
            },
        ));
    }
}

fn item_profile(key: &str) -> Option<ItemProfile> {
    COUNTER_ITEM_SET
        .get_or_init(|| {
            COUNTER_ITEMS
                .lock()
                .map(|items| items.clone())
                .unwrap_or_default()
        })
        .iter()
        .find(|(item, _)| item.as_str() == key)
        .map(|(_, profile)| *profile)
}

/// Whether this candidate is a counter item at all — the cheap gate that keeps
/// the lineup lookup off the hot path, since almost no candidate is one.
pub(crate) fn is_counter_item(key: &str) -> bool {
    item_profile(key).is_some()
}

// -- champions ------------------------------------------------------------

#[derive(Clone, Copy, Default, Debug)]
struct ChampionProfile {
    threats: Threats,
    damage: Damage,
}

/// Champion key -> what it brings and what it deals.
///
/// Holds **every** champion, not just the tagged ones: the enemy lookup only
/// needs healers and shielders, but the carrier lookup needs the damage type of
/// whoever is being built for. A `Vec` because it is ~68 entries scanned against
/// a lineup of five plus one carrier.
static CHAMPIONS: RwLock<Vec<(String, ChampionProfile)>> = RwLock::new(Vec::new());

/// Whether [`prime`] has a table and can stop looking.
static PRIMED: AtomicBool = AtomicBool::new(false);

/// Caches champion tags off the client, once per run.
///
/// Called unconditionally from the client tick because the item-build hook has
/// no client of its own: `champion_brief` lives on `StableClient`, and by the
/// time a build is being decided there is none in reach. Cheap after the first
/// success — one relaxed atomic load.
///
/// Returns early while `champion_names()` is empty (the sheet is not up during
/// early startup) so the next frame tries again.
pub(crate) fn prime(ctx: &StableClient<'_>) {
    if PRIMED.load(Ordering::Relaxed) {
        return;
    }
    let names = ctx.champion_names();
    if names.is_empty() {
        return;
    }
    let total = names.len();

    let mut table = Vec::with_capacity(total);
    let mut tagged = 0;
    for name in names {
        let Some(brief) = ctx.champion_brief(&name) else {
            continue;
        };
        let threats = Threats {
            heal: brief.tags.contains(&ChampionTagV1::Heal),
            shield: brief.tags.contains(&ChampionTagV1::Shield),
        };
        if threats.any() {
            tagged += 1;
        }
        table.push((
            name,
            ChampionProfile {
                threats,
                damage: Damage::from_champion(&brief.tags),
            },
        ));
    }

    // Primed even when nothing came back tagged: the sheet answered, so this is
    // a roster with nothing to counter rather than a roster that is not loaded,
    // and re-scanning every frame would not change it.
    if let Ok(mut cached) = CHAMPIONS.write() {
        eprintln!(
            "riot_items_tfm2: counter_items primed champions={total} read={} threat_tagged={tagged}",
            table.len()
        );
        *cached = table;
        PRIMED.store(true, Ordering::Relaxed);
    }
}

/// The damage type of the champion this build is for. Defaults to "no opinion"
/// for a champion the cache never saw, which [`Damage::suits`] treats as
/// passing — an unknown carrier is not a reason to withhold every counter item.
fn carrier_damage(champion: &str) -> Damage {
    CHAMPIONS
        .read()
        .ok()
        .and_then(|table| {
            table
                .iter()
                .find(|(key, _)| key.as_str() == champion)
                .map(|(_, profile)| profile.damage)
        })
        .unwrap_or_default()
}

/// What a lineup brings, and how many champions bring each thing.
fn lineup_threats(lineup: &[&str]) -> (Threats, usize, usize) {
    let Ok(table) = CHAMPIONS.read() else {
        return (Threats::default(), 0, 0);
    };
    let (mut healers, mut shielders) = (0usize, 0usize);
    for champion in lineup {
        if let Some((_, profile)) = table.iter().find(|(key, _)| key.as_str() == *champion) {
            if profile.threats.heal {
                healers += 1;
            }
            if profile.threats.shield {
                shielders += 1;
            }
        }
    }
    (
        Threats {
            heal: healers > 0,
            shield: shielders > 0,
        },
        healers,
        shielders,
    )
}

// -- the nudge ------------------------------------------------------------

/// The score bonus this candidate earns, or `None` when it earns none: not a
/// counter item, wrong damage type for this carrier, or nothing on the other
/// side to counter.
///
/// `enemy` is `StableItemBuildContext::enemy_champions`, which is the enemy *of
/// the champion being built for*, so this needs no notion of which side is the
/// player's — unlike `own_team_only`, which does and is why that toggle exists.
pub(crate) fn bonus(key: &str, champion: &str, enemy: &[&str]) -> Option<f32> {
    let profile = item_profile(key)?;

    // Who may hold it, before what it is worth: a pure-AP carrier gets no push
    // toward Serpent's Fang however much the enemy shields.
    if !profile.damage.suits(carrier_damage(champion)) {
        return None;
    }

    let (threats, healers, shielders) = lineup_threats(enemy);
    let matched =
        (profile.counters.heal && threats.heal) || (profile.counters.shield && threats.shield);
    if !matched {
        return None;
    }

    // An item that answers both (none today) takes the stronger of the two.
    let sources = profile
        .counters
        .heal
        .then_some(healers)
        .unwrap_or(0)
        .max(profile.counters.shield.then_some(shielders).unwrap_or(0));

    let scale = if sources >= 2 {
        STACKED_THREAT_MULTIPLIER
    } else {
        1.0
    };
    Some(COUNTER_BONUS * scale)
}

/// One-shot report that the lineup actually reached the hook.
///
/// `enemy_champions` is filled by the host, and a host that leaves it empty
/// would turn this whole module into a silent no-op — the failure mode this
/// codebase keeps running into. Logging the first decision makes that visible
/// instead: `enemy=0` means the lineup never arrived, not that the comp was
/// clean, and `ad=false ap=false` means the carrier key missed the cache.
pub(crate) fn note_first_decision(champion: &str, enemy: &[&str]) {
    static REPORTED: AtomicBool = AtomicBool::new(false);
    if REPORTED.swap(true, Ordering::Relaxed) {
        return;
    }
    let (threats, healers, shielders) = lineup_threats(enemy);
    let damage = carrier_damage(champion);
    eprintln!(
        "riot_items_tfm2: counter_items champion={champion} ad={} ap={} enemy={} \
         healers={healers} shielders={shielders} heal={} shield={}",
        damage.ad,
        damage.ap,
        enemy.len(),
        threats.heal,
        threats.shield
    );
}

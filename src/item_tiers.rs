//! Item Rules: the Options setting (Competition tab, `crate::options_row`) for
//! how far builds upgrade: All Radiants (the default), First Item Radiant, or
//! No Radiants, where every slot stops at its legendary.
//!
//! The setting lives in the save under [`SAVE_KEY`], so each save has its own.
//! The save namespace reads empty around loads, so a missing value only counts
//! after [`ABSENT_GRACE`], and a click stands until the save reads it back.
//!
//! A slot is held at legendary by rewriting its target key in
//! `tactics::buy_replace_ctx`, since the engine upgrades a slot only until its
//! key matches; everything that decides builds still deals in radiants. Mod
//! radiants are `radiant_<key>` over a legendary `<key>` and the game's six are
//! listed in `item_catalog::GAME_RADIANTS`, so callers check that the other
//! form exists unless [`is_game_tier`].

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

use mod_api_stable::StableClient;

use crate::item_catalog::GAME_RADIANTS;

const RADIANT_PREFIX: &str = "radiant_";

// The setting's three sides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rule {
    // Every slot is bought up to its radiant.
    AllRadiants,
    // The first slot is; every other stops at its legendary.
    FirstItemRadiant,
    // Every slot stops at its legendary.
    NoRadiants,
}

impl Rule {
    // Every side, in the order the Options row shows them.
    pub(crate) const ALL: [Rule; 3] = [Rule::AllRadiants, Rule::FirstItemRadiant, Rule::NoRadiants];

    // A small number for the side: what an atomic and the buy detour's memo
    // hold. All Radiants is 0, and nothing else is.
    pub(crate) const fn code(self) -> u8 {
        match self {
            Rule::AllRadiants => 0,
            Rule::FirstItemRadiant => 1,
            Rule::NoRadiants => 2,
        }
    }

    fn from_code(code: u8) -> Rule {
        match code {
            1 => Rule::FirstItemRadiant,
            2 => Rule::NoRadiants,
            _ => Rule::AllRadiants,
        }
    }

    // What the save holds for the side. A save holding anything else is on
    // All Radiants.
    fn saved(self) -> &'static str {
        match self {
            Rule::AllRadiants => "all_radiants",
            Rule::FirstItemRadiant => "first_item_radiant",
            Rule::NoRadiants => "no_radiants",
        }
    }

    fn from_saved(saved: &str) -> Rule {
        Rule::ALL
            .into_iter()
            .find(|rule| rule.saved() == saved)
            .unwrap_or(Rule::AllRadiants)
    }
}

// The setting's key in the save.
const SAVE_KEY: &str = "item_rules";

// Frames between two looks at the save for the setting.
const READ_EVERY: u32 = 30;

// Frames the save has to have had no setting, or not been there at all, for
// that to count: `item_stats::LOAD_GRACE`'s ten seconds, for its reason.
const ABSENT_GRACE: u32 = 600;

// The side the save now loaded is on, as [`Rule::code`]. An atomic because
// the buy detour reads it, on whichever thread a match is simulating.
static RULE: AtomicU8 = AtomicU8::new(0);

// A choice made in the Options dialog that the save has not been read back
// agreeing with yet, as [`Rule::code`] plus one (0 is none), and whether the
// save has taken the write of it.
const NO_CHOICE: u8 = 0;
static CHOICE: AtomicU8 = AtomicU8::new(NO_CHOICE);
static WRITTEN: AtomicBool = AtomicBool::new(false);

// The `rules.` lines of `match-builds.log` when its switch is on: clicks,
// writes and reads.
fn log(key: &str, text: impl FnOnce() -> String) {
    crate::match_builds::log(key, text);
}

// Frames [`sync`] has run, and how many of them on end found no setting.
static FRAME: AtomicU32 = AtomicU32::new(0);
static ABSENT: AtomicU32 = AtomicU32::new(0);

// The side the save now loaded is on.
pub(crate) fn rule() -> Rule {
    Rule::from_code(RULE.load(Ordering::Relaxed))
}

// The player's click on the Options row. It rules from this moment, and
// [`sync`] writes it to the save on the next frame: a click handler is the
// one place the save cannot be written from.
pub(crate) fn choose(rule: Rule) {
    RULE.store(rule.code(), Ordering::Relaxed);
    WRITTEN.store(false, Ordering::Relaxed);
    CHOICE.store(rule.code() + 1, Ordering::Relaxed);
    log("rules.click", || format!("{rule:?}"));
}

// A save's server has started: another save, or this one loaded again. A
// click that had not reached the last one is not this one's.
pub(crate) fn server_started() {
    CHOICE.store(NO_CHOICE, Ordering::Relaxed);
    WRITTEN.store(false, Ordering::Relaxed);
    ABSENT.store(0, Ordering::Relaxed);
}

// Counts `frames` more without a setting, and falls back to All Radiants
// once that has gone on for [`ABSENT_GRACE`]: a click still waiting for a
// save goes with it.
fn absent_for(frames: u32) {
    let absent = ABSENT.load(Ordering::Relaxed).saturating_add(frames);
    ABSENT.store(absent, Ordering::Relaxed);
    if absent >= ABSENT_GRACE {
        RULE.store(Rule::AllRadiants.code(), Ordering::Relaxed);
        CHOICE.store(NO_CHOICE, Ordering::Relaxed);
    }
}

// Keeps the setting in step with the save. Every client frame.
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    let writable = ctx.save_can_write();
    log("rules.save", || format!("can be written={writable}"));
    if !writable {
        // The menu, or between saves.
        absent_for(1);
        return;
    }

    // The player's click, handed to the save until it takes it.
    let choice = CHOICE.load(Ordering::Relaxed);
    let chosen = choice
        .checked_sub(1)
        .map(|code| Rule::from_code(code).saved());
    if let Some(chosen) = chosen {
        if !WRITTEN.load(Ordering::Relaxed) {
            let taken = ctx.save_set_string(SAVE_KEY, chosen);
            WRITTEN.store(taken, Ordering::Relaxed);
            log("rules.write", || format!("`{chosen}` taken={taken}"));
        }
    }

    if FRAME.fetch_add(1, Ordering::Relaxed) % READ_EVERY != 0 {
        return;
    }
    let saved = ctx.save_get_string(SAVE_KEY);
    log("rules.read", || {
        format!("the save holds {saved:?}; a click still stands: {chosen:?}")
    });
    if let Some(chosen) = chosen {
        // The click stands until the save is read back saying the same. Only
        // then is the save the one to ask again.
        ABSENT.store(0, Ordering::Relaxed);
        if saved.as_deref() == Some(chosen) {
            // Unless another click came in meanwhile.
            let _ =
                CHOICE.compare_exchange(choice, NO_CHOICE, Ordering::Relaxed, Ordering::Relaxed);
        }
        return;
    }
    match saved {
        Some(saved) => {
            RULE.store(Rule::from_saved(&saved).code(), Ordering::Relaxed);
            ABSENT.store(0, Ordering::Relaxed);
        }
        None => absent_for(READ_EVERY),
    }
}

// Whether `key` is one of the game's six radiants or the legendary under one:
// its other form is known for certain, so a caller need not check that it
// exists.
pub(crate) fn is_game_tier(key: &str) -> bool {
    GAME_RADIANTS
        .iter()
        .any(|&(_, radiant, legendary)| radiant == key || legendary == key)
}

// The legendary under the radiant `key`, by key. `None` for anything that is
// not a radiant. The key it gives may not exist: see the module notes.
pub(crate) fn legendary_of(key: &str) -> Option<String> {
    let game = GAME_RADIANTS.iter().find(|&&(_, radiant, _)| radiant == key);
    if let Some(&(_, _, legendary)) = game {
        return Some(legendary.to_string());
    }
    key.strip_prefix(RADIANT_PREFIX).map(str::to_string)
}

// The radiant over the legendary `key`, by key. `None` for a radiant. For
// anything else it is a guess at a key that may not exist: see the module
// notes.
pub(crate) fn radiant_of(key: &str) -> Option<String> {
    let game = GAME_RADIANTS.iter().find(|&&(_, _, legendary)| legendary == key);
    if let Some(&(_, radiant, _)) = game {
        return Some(radiant.to_string());
    }
    let a_radiant = GAME_RADIANTS.iter().any(|&(_, radiant, _)| radiant == key);
    if key.starts_with(RADIANT_PREFIX) || a_radiant {
        return None;
    }
    Some(format!("{RADIANT_PREFIX}{key}"))
}

// The other form of `key` that the setting has a build hold in `slot`, where
// `key` is not that form already and the setting holds anything back at
// all: the legendary in a slot it caps, the radiant in the one it does not.
pub(crate) fn for_slot(key: &str, slot: usize) -> Option<String> {
    match rule() {
        Rule::AllRadiants => None,
        Rule::FirstItemRadiant if slot == 0 => radiant_of(key),
        Rule::FirstItemRadiant | Rule::NoRadiants => legendary_of(key),
    }
}

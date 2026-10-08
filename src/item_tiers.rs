//! Item Rules: the setting in the game's Options dialog (Competition tab,
//! `crate::options_row`) that says how far a build is upgraded.
//!
//! - **All Radiants**, the default: every slot of a build is bought up to its
//!   radiant, as it always was.
//! - **First Item Radiant**: only the first slot is. Every other slot stops at
//!   its legendary, the tier under the radiant.
//!
//! The user asked for it (2026-10-08): "limit all games to 1 radiant item",
//! "make the first slot the radiant", and a pinned item goes by the setting
//! like any other ("the setting will infer the behavior").
//!
//! # Where the setting lives
//!
//! In the save, one a save: this mod's key-value namespace there, beside the
//! item stats (`item_stats::sync`), under [`SAVE_KEY`]. The user asked for
//! that the same day, "that way people can have multiple saves with different
//! settings"; it was a line of `mod-settings.json` for the hour before. A
//! save that has never been told is on All Radiants.
//!
//! The namespace has the two habits `item_stats` documents: a value written
//! to it reaches disk only when the player saves the game, and it reads empty
//! on some frames around a load. So the value is looked up again every
//! [`READ_EVERY`] frames, a value found is taken at once, and "nothing there"
//! only counts once it has been the answer for [`ABSENT_GRACE`] frames on end
//! ([`sync`]).
//!
//! A third habit is suspected. The first version took whatever the next look
//! returned, half a second after writing the player's click, and the player
//! could not switch the setting back once the save held one (2026-10-08). If
//! a read can still return the old value for a while after a write, that is
//! exactly what it would do. So the click now stands until a look at the
//! save returns the same value, however long that takes, or until the save
//! goes ([`server_started`], or no save for [`ABSENT_GRACE`]). [`LOG`] says
//! what the save answers, to settle whether that was it.
//!
//! # How a slot is held at legendary
//!
//! By what the slot *targets*. The engine buys a build strictly in order and
//! upgrades the item it is on until that item's key equals the key the slot
//! names; then it starts the next slot, and never comes back (read out of the
//! 0.6.3 executable, see the buy detour). A slot naming `collector` instead of
//! `radiant_collector` is finished at Collector. So nothing is forbidden or
//! refunded: `tactics::buy_replace_ctx` rewrites the targets of the slots not
//! started yet, and that is the whole mechanism.
//!
//! Everything that *decides* a build (both item-build hooks, Smart Builds, the
//! pins, the picks for the 5th and 6th slots) still deals in radiants, the
//! only finals there are. This module is the map between the two forms, and
//! nothing else.
//!
//! # The two forms
//!
//! Every radiant this mod registers is `radiant_<key>` over a legendary
//! registered as `<key>` (checked across `lib.rs`, 98 of 98), so the prefix is
//! the map. The game's own six radiants keep the game's keys and are listed.
//! An item with no other form (boots, which top out at their third tier, or a
//! component) is not mapped, and neither is another mod's unless it follows
//! the same prefix: both callers check that the other form exists before they
//! use it.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

use mod_api_stable::StableClient;

const RADIANT_PREFIX: &str = "radiant_";

/// The setting's key in the save, and what it holds for each side.
const SAVE_KEY: &str = "item_rules";
const SAVED_ALL: &str = "all_radiants";
const SAVED_FIRST: &str = "first_item_radiant";

/// Frames between two looks at the save for the setting.
const READ_EVERY: u32 = 30;

/// Frames the save has to have had no setting, or not been there at all, for
/// that to count: `item_stats::LOAD_GRACE`'s ten seconds, for its reason.
const ABSENT_GRACE: u32 = 600;

/// Whether the save now loaded is on First Item Radiant. An atomic because
/// the buy detour reads it, on whichever thread a match is simulating.
static FIRST: AtomicBool = AtomicBool::new(false);

/// A choice made in the Options dialog that the save has not been read back
/// agreeing with yet, and whether the save has taken the write of it.
const NO_CHOICE: u8 = 0;
const CHOSE_ALL: u8 = 1;
const CHOSE_FIRST: u8 = 2;
static CHOICE: AtomicU8 = AtomicU8::new(NO_CHOICE);
static WRITTEN: AtomicBool = AtomicBool::new(false);

/// Whether `match-builds.log` gets a line when the setting's dealings with
/// the save change: a click, whether the save took the write, what a look at
/// it returned. On until the setting has been seen to switch both ways and
/// to last through a save and a load. Turn it off before a release.
const LOG: bool = true;

fn log(key: &str, text: impl FnOnce() -> String) {
    if LOG {
        crate::match_builds::log_note(key, text);
    }
}

/// Frames [`sync`] has run, and how many of them on end found no setting.
static FRAME: AtomicU32 = AtomicU32::new(0);
static ABSENT: AtomicU32 = AtomicU32::new(0);

/// The game's own radiants and the legendary under each: the top two tiers of
/// its attack, attack speed, armor, magic resistance, magic power and health
/// lines (Bloodthirster, Phantom Dancer, Thornmail, Dragon's Claw, Luden's
/// Tempest and Sunfire Cape, as the mod draws them).
const GAME_TIERS: [(&str, &str); 6] = [
    ("warlords_final_judgement", "conquerors_greatsword"),
    ("storm_sovereign", "thunderclaw"),
    ("impregnable_fortress", "eternal_iron_plate"),
    ("veil_of_annihilation", "souls_edge"),
    ("prophet_of_the_abyss", "angels_fang"),
    ("giants_horn_shard", "hourglass_of_eternity"),
];

/// Whether the save now loaded is on First Item Radiant.
pub(crate) fn first_item_radiant() -> bool {
    FIRST.load(Ordering::Relaxed)
}

/// The player's click on the Options row. It rules from this moment, and
/// [`sync`] writes it to the save on the next frame: a click handler is the
/// one place the save cannot be written from.
pub(crate) fn choose(first: bool) {
    FIRST.store(first, Ordering::Relaxed);
    WRITTEN.store(false, Ordering::Relaxed);
    CHOICE.store(
        if first { CHOSE_FIRST } else { CHOSE_ALL },
        Ordering::Relaxed,
    );
    log("rules.click", || format!("first_item_radiant={first}"));
}

/// A save's server has started: another save, or this one loaded again. A
/// click that had not reached the last one is not this one's.
pub(crate) fn server_started() {
    CHOICE.store(NO_CHOICE, Ordering::Relaxed);
    WRITTEN.store(false, Ordering::Relaxed);
    ABSENT.store(0, Ordering::Relaxed);
}

/// Counts `frames` more without a setting, and falls back to All Radiants
/// once that has gone on for [`ABSENT_GRACE`]: a click still waiting for a
/// save goes with it.
fn absent_for(frames: u32) {
    let absent = ABSENT.load(Ordering::Relaxed).saturating_add(frames);
    ABSENT.store(absent, Ordering::Relaxed);
    if absent >= ABSENT_GRACE {
        FIRST.store(false, Ordering::Relaxed);
        CHOICE.store(NO_CHOICE, Ordering::Relaxed);
    }
}

/// Keeps the setting in step with the save. Every client frame.
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
    let chosen = match choice {
        CHOSE_FIRST => Some(SAVED_FIRST),
        CHOSE_ALL => Some(SAVED_ALL),
        _ => None,
    };
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
            FIRST.store(saved == SAVED_FIRST, Ordering::Relaxed);
            ABSENT.store(0, Ordering::Relaxed);
        }
        None => absent_for(READ_EVERY),
    }
}

/// The legendary under the radiant `key`, by key. `None` for anything that is
/// not a radiant. The key it gives may not exist: see the module notes.
pub(crate) fn legendary_of(key: &str) -> Option<String> {
    if let Some(&(_, legendary)) = GAME_TIERS.iter().find(|(radiant, _)| *radiant == key) {
        return Some(legendary.to_string());
    }
    key.strip_prefix(RADIANT_PREFIX).map(str::to_string)
}

/// The radiant over the legendary `key`, by key. `None` for a radiant. For
/// anything else it is a guess at a key that may not exist: see the module
/// notes.
pub(crate) fn radiant_of(key: &str) -> Option<String> {
    if let Some(&(radiant, _)) = GAME_TIERS.iter().find(|(_, legendary)| *legendary == key) {
        return Some(radiant.to_string());
    }
    if key.starts_with(RADIANT_PREFIX) || GAME_TIERS.iter().any(|(radiant, _)| *radiant == key) {
        return None;
    }
    Some(format!("{RADIANT_PREFIX}{key}"))
}

/// The other form of `key` that First Item Radiant has a build hold in
/// `slot`, where `key` is not that form already: the radiant in the first
/// slot, the legendary in every other.
pub(crate) fn for_slot(key: &str, slot: usize) -> Option<String> {
    if slot == 0 {
        radiant_of(key)
    } else {
        legendary_of(key)
    }
}

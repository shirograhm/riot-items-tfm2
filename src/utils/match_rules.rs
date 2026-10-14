//! The match rules the mod changes: keys of the game's own game settings
//! (`setting/game_setting` in the base game) with the mod's numbers for them,
//! set in [`RULES`]. Today that is the starting gold alone.
//!
//! # Why the server is told, and not the asset
//!
//! The first try was the asset: a `setting/game_setting.game_setting` with
//! the one key in it, merged over the game's by `mod.override_info`. It did
//! nothing, twice over. The game reads such a file as the whole settings, so
//! one that has a single key is refused when the assets load (the game's
//! `log.log` says "missing field width"). And a merge that does load is
//! still not what a
//! match runs on: the server keeps settings of its own, which for the items
//! were seen to be the base game's on a new game and a loaded one alike
//! (`item_stats::sync_server_items`).
//!
//! So the numbers are written into the server's settings, which the stable
//! API calls the game rule settings and which "affect matches created
//! afterwards": once for each server, from its start, before it creates a
//! match and before the client takes its copy of them. A write the server
//! refuses changes nothing: its settings must still read as settings.
//!
//! Only what the mod changes is listed. A whole copy of the game's settings
//! would put back, at every start, whatever a game update or the game's own
//! patches had changed since the copy was made.

use std::sync::atomic::{AtomicBool, Ordering};

use mod_api_stable::{SettingTargetV1, StableServerCtx};
use serde_json::Value;

// The game settings the mod changes, and its number for each: starting gold
// 250, where the base game gives 500.
const RULES: &[(&str, i64)] = &[("start_gold", 250)];

// Whether the server of the save now loaded has been given the rules.
static SYNCED: AtomicBool = AtomicBool::new(false);

// A new server: its settings have not been looked at.
pub(crate) fn server_started() {
    SYNCED.store(false, Ordering::Relaxed);
}

// Makes the server's game settings hold the mod's rules. From the server's
// start and then from its ticks until its settings could be read; one atomic
// load after that.
pub(crate) fn sync_server(ctx: &mut StableServerCtx<'_>) {
    if SYNCED.load(Ordering::Relaxed) {
        return;
    }
    let mut read = 0;
    let mut report: Vec<String> = Vec::new();
    for &(name, wanted) in RULES {
        let held = ctx
            .setting_get_json(SettingTargetV1::GameSetting, name)
            .and_then(|json| serde_json::from_str::<Value>(&json).ok());
        // No such setting, or none to show yet.
        let Some(held) = held else {
            continue;
        };
        read += 1;
        let wanted = Value::from(wanted);
        if crate::item_stats::same_setting(&wanted, &held) {
            report.push(format!("{name} is {held} already"));
            continue;
        }
        let written = ctx.setting_set_json(SettingTargetV1::GameSetting, name, &wanted.to_string());
        report.push(format!("{name} {held} -> {wanted}, written={written}"));
    }
    // Nothing read: the server has no settings to show yet. The next tick
    // asks again.
    if read == 0 {
        return;
    }
    SYNCED.store(true, Ordering::Relaxed);
    crate::match_builds::log("server rules", || {
        format!("{read} of {} read; {}", RULES.len(), report.join("; "))
    });
}

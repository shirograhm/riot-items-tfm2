//! The match rules the mod changes: game settings keys (`setting/game_setting`)
//! with the mod's numbers, in [`RULES`]; today only the starting gold.
//!
//! They are written into the server's settings once per server, before it
//! creates a match: an asset override was refused (a partial settings file
//! fails to load), and matches run on the server's own settings anyway. Only
//! changed keys are listed, so a game update's other changes are not
//! overwritten.

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

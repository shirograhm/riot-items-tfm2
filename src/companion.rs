//! Detection of companion mods whose presence changes what this mod offers.
//!
//! Right now that is exactly one mod: `tfm2_item_tactics`, which raises the
//! game's item slots from three to four. When it is installed, enabled, and set
//! to four slots, the in-game build editor offers a fourth item per champion;
//! otherwise nothing about this mod changes.
//!
//! Everything here is read-only observation of another mod's files. Nothing is
//! written, and every failure path falls back to the vanilla three slots, so a
//! renamed file or a future format change in that mod costs this one nothing.

use serde::Deserialize;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Mod id of the four-item mod, as it appears both in its `mod.mod_info` and in
/// the game's `enabled_mods` list.
const ITEM_TACTICS_ID: &str = "tfm2_item_tactics";

/// Item slots the game has without help: the three the strategy screen shows.
pub const VANILLA_SLOTS: usize = 3;

/// Item slots with `tfm2_item_tactics` active and set to four.
pub const EXTENDED_SLOTS: usize = 4;

/// Game version this mod was loaded against, recorded at init because the
/// checks below run from UI and hook paths that have no host handle.
static GAME_VERSION: OnceLock<(u32, u32, u32)> = OnceLock::new();

/// Records the running game version. Called once from `init`, before anything
/// can ask for [`item_slots`].
pub fn record_game_version(version: (u32, u32, u32)) {
    let _ = GAME_VERSION.set(version);
}

/// Schema of `config/game/mods.json`, of which only the enabled list matters.
/// Every other key in that file (`known_workshop_mods`, `known_workshop_items`,
/// …) is ignored by omission.
#[derive(Deserialize)]
struct ModsFile {
    #[serde(default)]
    enabled_mods: Vec<String>,
}

/// How many item slots a build has: [`VANILLA_SLOTS`], or [`EXTENDED_SLOTS`]
/// when `tfm2_item_tactics` is installed, enabled, and configured for four.
///
/// Resolved once. Both inputs are read at startup by the mod that owns them —
/// `4items.cfg` documents that changing it needs a game restart, and the enabled
/// list cannot change while the game runs — so re-reading them per call would
/// buy nothing but disk hits on a UI path.
pub fn item_slots() -> usize {
    static SLOTS: OnceLock<usize> = OnceLock::new();
    *SLOTS.get_or_init(detect_item_slots)
}

fn detect_item_slots() -> usize {
    if !is_mod_enabled(ITEM_TACTICS_ID) {
        return VANILLA_SLOTS;
    }
    if !supports_this_game_version(ITEM_TACTICS_ID) {
        return VANILLA_SLOTS;
    }
    match item_tactics_slot_count() {
        Some(EXTENDED_SLOTS) => EXTENDED_SLOTS,
        // Present and enabled but toggled back to three slots: the fourth slot
        // does not exist in the game, so offering it would write builds whose
        // last item goes nowhere.
        _ => VANILLA_SLOTS,
    }
}

/// The mods directory this mod is installed in — the parent of its own folder.
fn mods_dir() -> Option<PathBuf> {
    crate::config::dll_dir()?.parent().map(PathBuf::from)
}

/// Whether `mod_id` is in the game's enabled list.
///
/// `config/game/mods.json` sits two levels above this mod's folder
/// (`<game>/mods/riot_items_tfm2` -> `<game>`). This is the same file, and the
/// same key, that `tfm2_item_tactics` itself reads to decide which mods' items
/// to offer, so the two agree on what "enabled" means by construction rather
/// than by coincidence.
///
/// A missing or malformed file means "not enabled": the fourth slot is the
/// claim that needs evidence, and the vanilla three always work.
fn is_mod_enabled(mod_id: &str) -> bool {
    let Some(game_root) = mods_dir().and_then(|dir| dir.parent().map(PathBuf::from)) else {
        return false;
    };
    let path = game_root.join("config").join("game").join("mods.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    serde_json::from_str::<ModsFile>(&text)
        .map(|mods| mods.enabled_mods.iter().any(|id| id == mod_id))
        .unwrap_or(false)
}

/// Schema of another mod's `mod.mod_info`, of which only the dependency list
/// matters here.
#[derive(Deserialize)]
struct ModInfo {
    #[serde(default)]
    dependencies: Vec<ModDependency>,
}

#[derive(Deserialize)]
struct ModDependency {
    mod_id: String,
    #[serde(default)]
    version: String,
}

/// Whether `mod_id` declares support for the running game version.
///
/// `tfm2_item_tactics` is pinned to one game version (`base >=0.5.3,<0.5.4`)
/// and its own description says it disables itself elsewhere. Being listed in
/// `enabled_mods` therefore is not the same as being active: after a game
/// update it is still "enabled" while doing nothing, and a fourth slot offered
/// on that basis would write an item the game never places.
///
/// The range is read from that mod's own `mod.mod_info` rather than hardcoded,
/// so the day it widens support this follows without an edit here. Anything
/// unreadable or unparseable is treated as supported: the enabled list and the
/// slot count have already been checked by then, and refusing on a range this
/// code merely failed to understand would disable the feature for no reason.
fn supports_this_game_version(mod_id: &str) -> bool {
    let Some(&version) = GAME_VERSION.get() else {
        // Not recorded yet; `init` sets it before anything can ask.
        return false;
    };
    let Some(path) = mods_dir().map(|dir| dir.join(mod_id).join("mod.mod_info")) else {
        return true;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return true;
    };
    let Ok(info) = serde_json::from_str::<ModInfo>(&text) else {
        return true;
    };
    info.dependencies
        .iter()
        .find(|dependency| dependency.mod_id == "base")
        .map(|dependency| range_allows(&dependency.version, version))
        .unwrap_or(true)
}

/// Whether `version` satisfies a comma-separated requirement like
/// `">=0.5.3,<0.5.4"`. Every term must hold. A term this does not understand is
/// skipped rather than failing the whole check.
fn range_allows(range: &str, version: (u32, u32, u32)) -> bool {
    range.split(',').all(|term| {
        let term = term.trim();
        let (op, rest) = match term {
            _ if term.starts_with(">=") => (">=", &term[2..]),
            _ if term.starts_with("<=") => ("<=", &term[2..]),
            _ if term.starts_with('>') => (">", &term[1..]),
            _ if term.starts_with('<') => ("<", &term[1..]),
            _ if term.starts_with('=') => ("=", &term[1..]),
            _ => return true,
        };
        let Some(bound) = parse_version(rest) else {
            return true;
        };
        match op {
            ">=" => version >= bound,
            "<=" => version <= bound,
            ">" => version > bound,
            "<" => version < bound,
            _ => version == bound,
        }
    })
}

/// Parses `"0.5.3"`. A missing patch counts as zero, so `"0.5"` is `0.5.0`.
fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let mut parts = text.trim().split('.');
    let major = parts.next()?.trim().parse().ok()?;
    let minor = parts.next().unwrap_or("0").trim().parse().ok()?;
    let patch = parts.next().unwrap_or("0").trim().parse().ok()?;
    Some((major, minor, patch))
}

/// The slot count `tfm2_item_tactics` is configured for, from its `4items.cfg`.
///
/// That file is `# comment` lines plus one `slots = N`, and the mod itself
/// parses it by finding the first digits after the key, so this reads it the
/// same lenient way: comments are dropped, the first `slots` assignment wins,
/// and anything unparseable yields `None`.
fn item_tactics_slot_count() -> Option<usize> {
    let path = mods_dir()?.join(ITEM_TACTICS_ID).join("4items.cfg");
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim().eq_ignore_ascii_case("slots"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
}

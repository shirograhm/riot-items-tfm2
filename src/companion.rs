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
//!
//! The one subtlety is *which* failures are remembered. These files have other
//! writers — the game rewrites `config/game/mods.json` mid-session — so a read
//! can fail for reasons that say nothing about whether the companion mod is
//! there. Those reads are retried, and their fallback is never cached; only an
//! answer actually read off disk settles the question for the session.

use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
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

/// Reads and parses a file, retrying a few times before giving up.
///
/// Both files this module reads are owned by other writers. `config/game/mods.json`
/// in particular is rewritten *while the game runs* — accepting a save/mod
/// mismatch warning appends to it, which happens during save loading, moments
/// before the strategy screen this feature is for. A read landing inside that
/// rewrite either fails outright (Windows denies the open with a sharing
/// violation) or returns a truncated document that will not parse.
///
/// Retrying rather than reading once is what turns that from a coin flip into a
/// near-certainty, since the window is a single small write. Parsing happens
/// inside the loop so a torn read is retried too, not just a failed open.
/// `None` means "could not read it", which callers must not confuse with "read
/// it and the answer was no".
/// `attempts` is how many times to try. Only the eager resolution at init asks
/// for more than one: it owns a thread that is doing nothing else, whereas the
/// lazy path runs on the frame the UI is drawing and must not sleep on it.
fn read_parsed<T>(path: &Path, attempts: u32, parse: impl Fn(&str) -> Option<T>) -> Option<T> {
    const RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(20);

    for attempt in 0..attempts.max(1) {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Some(value) = parse(&text) {
                return Some(value);
            }
        }
        if attempt + 1 < attempts {
            std::thread::sleep(RETRY_DELAY);
        }
    }
    None
}

/// Schema of `config/game/mods.json`, of which only the enabled list matters.
/// Every other key in that file (`known_workshop_mods`, `known_workshop_items`,
/// …) is ignored by omission.
#[derive(Deserialize)]
struct ModsFile {
    #[serde(default)]
    enabled_mods: Vec<String>,
}

/// Cache for [`item_slots`]. Zero means "not resolved yet"; no real answer is
/// zero, so the sentinel costs nothing. An `AtomicUsize` rather than a
/// `OnceLock` because a `OnceLock` cannot distinguish a real answer from a
/// guess, and this must never memoize a guess — see [`item_slots`].
static SLOTS: AtomicUsize = AtomicUsize::new(0);

/// Lazy detections run so far, so an input that never becomes readable cannot
/// cost a file read on every frame forever. See [`item_slots`].
static LAZY_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);

/// How many lazy detections may come back inconclusive before the vanilla three
/// are accepted as the answer. Each is a single failed read — no sleeping — so
/// this is roughly a second of frames: long enough to outlast any write to
/// `mods.json`, short enough that a genuinely missing file settles quickly.
const MAX_LAZY_ATTEMPTS: usize = 64;

/// Reads used per input when resolving eagerly, spaced by a short sleep. Only
/// the init path pays this, and only when a read fails.
const EAGER_READ_ATTEMPTS: u32 = 5;

/// Resolves the slot count now, so the answer is settled before the files it
/// depends on can be written under us. Called from `init`.
///
/// This is not just an optimisation. Left to resolve lazily, the first caller is
/// the hook or the build editor — both of which run just after a save is loaded,
/// which is exactly when the game rewrites `config/game/mods.json`. Resolving at
/// init instead puts the read in a quiet window, during mod loading, when the
/// game has finished reading that file and has no reason to touch it — and it is
/// the one call site that can afford to retry a read properly, since no frame is
/// waiting on it.
pub fn resolve_item_slots() {
    let (slots, conclusive) = detect_item_slots(EAGER_READ_ATTEMPTS);
    if conclusive {
        SLOTS.store(slots, Ordering::Relaxed);
    }
}

/// How many item slots a build has: [`VANILLA_SLOTS`], or [`EXTENDED_SLOTS`]
/// when `tfm2_item_tactics` is installed, enabled, and configured for four.
///
/// Resolved once, but only once *conclusively*. Every input is read at startup
/// by the mod that owns it — `4items.cfg` documents that changing it needs a
/// game restart, and the enabled list cannot change while the game runs — so a
/// re-read per call would buy nothing but disk hits on a UI path.
///
/// What must not be cached is an answer this could not actually determine. A
/// read that loses a race with the game rewriting `mods.json` looks identical to
/// the companion mod being absent, and memoizing that verdict turns a lost race
/// into a session with no fourth item in it at all. So an unreadable input
/// falls back to the vanilla three for *this* call, and leaves the question open
/// for the next one.
///
/// "Leaves it open" is bounded, because this is a per-frame UI call and a file
/// that is missing rather than busy would never become readable: after
/// [`MAX_LAZY_ATTEMPTS`] inconclusive tries the fallback is accepted. Retrying
/// here never sleeps, for the same reason.
pub fn item_slots() -> usize {
    match SLOTS.load(Ordering::Relaxed) {
        0 => {
            let (slots, conclusive) = detect_item_slots(1);
            let give_up = LAZY_ATTEMPTS.fetch_add(1, Ordering::Relaxed) + 1 >= MAX_LAZY_ATTEMPTS;
            if conclusive || give_up {
                SLOTS.store(slots, Ordering::Relaxed);
            }
            slots
        }
        cached => cached,
    }
}

/// The slot count, and whether it was actually determined (as opposed to fallen
/// back to). Only a determined answer is worth remembering.
fn detect_item_slots(attempts: u32) -> (usize, bool) {
    match is_mod_enabled(ITEM_TACTICS_ID, attempts) {
        Some(true) => {}
        // Read and answered: the companion mod is not enabled.
        Some(false) => return (VANILLA_SLOTS, true),
        None => return (VANILLA_SLOTS, false),
    }
    match supports_this_game_version(ITEM_TACTICS_ID, attempts) {
        Some(true) => {}
        Some(false) => return (VANILLA_SLOTS, true),
        None => return (VANILLA_SLOTS, false),
    }
    match item_tactics_slot_count(attempts) {
        Some(EXTENDED_SLOTS) => (EXTENDED_SLOTS, true),
        // Present and enabled but toggled back to three slots: the fourth slot
        // does not exist in the game, so offering it would write builds whose
        // last item goes nowhere.
        Some(_) => (VANILLA_SLOTS, true),
        None => (VANILLA_SLOTS, false),
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
/// `None` when the file could not be read or parsed at all — which is a
/// different thing from reading it and not finding `mod_id`, because the game
/// rewrites this file while it runs and a read can lose that race. Caching
/// "not enabled" on that basis is the bug this distinction exists to prevent.
fn is_mod_enabled(mod_id: &str, attempts: u32) -> Option<bool> {
    let game_root = mods_dir().and_then(|dir| dir.parent().map(PathBuf::from))?;
    let path = game_root.join("config").join("game").join("mods.json");
    let mods = read_parsed(&path, attempts, |text| {
        serde_json::from_str::<ModsFile>(text).ok()
    })?;
    Some(mods.enabled_mods.iter().any(|id| id == mod_id))
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
///
/// `None` only when the game version has not been recorded yet — an answer this
/// cannot give rather than one it gives as no, so the caller retries instead of
/// caching a three-slot verdict reached before `init` finished.
fn supports_this_game_version(mod_id: &str, attempts: u32) -> Option<bool> {
    let &version = GAME_VERSION.get()?;
    let Some(path) = mods_dir().map(|dir| dir.join(mod_id).join("mod.mod_info")) else {
        return Some(true);
    };
    let info = read_parsed(&path, attempts, |text| {
        serde_json::from_str::<ModInfo>(text).ok()
    });
    let Some(info) = info else {
        return Some(true);
    };
    Some(
        info.dependencies
            .iter()
            .find(|dependency| dependency.mod_id == "base")
            .map(|dependency| range_allows(&dependency.version, version))
            .unwrap_or(true),
    )
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
/// same lenient way: comments are dropped and the first `slots` assignment wins.
///
/// `None` means the file could not be read, which the caller must not cache. A
/// file that *was* read but carries no usable `slots` value reports
/// [`VANILLA_SLOTS`]: unconfigured and configured-for-three lead to the same
/// place, and both are real answers rather than failures to look.
fn item_tactics_slot_count(attempts: u32) -> Option<usize> {
    let path = mods_dir()?.join(ITEM_TACTICS_ID).join("4items.cfg");
    let text = read_parsed(&path, attempts, |text| {
        (!text.trim().is_empty()).then(|| text.to_string())
    })?;
    let slots = text
        .lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim().eq_ignore_ascii_case("slots"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok());
    Some(slots.unwrap_or(VANILLA_SLOTS))
}

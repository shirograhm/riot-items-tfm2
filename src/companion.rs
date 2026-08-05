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
///
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
    // `tfm2_item_tactics` now ships *inside* this mod (see `src/tactics`), so
    // the slot count it reports is a first-hand answer, not an observation of
    // somebody else's files. It settles the question outright: it is the code
    // that installs the byte patches making a fourth slot exist, so if it says
    // four there are four, and if it says three there are three no matter what
    // a separately installed copy claims.
    //
    // A separate copy may still be installed and enabled alongside this one.
    // That is why the detection below is kept rather than deleted — it is the
    // answer for a user who has not enabled our merged half but has that mod.
    // `checked_sub(1)` is what removes the bias — `None` means "not active".
    // Do not re-add it here: storing `slots + 1` put the *encoded* value in
    // `SLOTS`, so the editor drew four slots for a three-slot game and five for
    // a four-slot one.
    if let Some(builtin_slots) = BUILTIN_SLOTS.load(Ordering::Relaxed).checked_sub(1) {
        SLOTS.store(builtin_slots, Ordering::Relaxed);
        return;
    }

    let (slots, conclusive) = detect_item_slots(EAGER_READ_ATTEMPTS);
    if conclusive {
        SLOTS.store(slots, Ordering::Relaxed);
    }
}

/// Slot count reported by the merged `tfm2_item_tactics` half, biased by one so
/// that 0 means "it is not active" (its version gate failed, so it installed no
/// patches at all and the game has whatever slots it shipped with).
static BUILTIN_SLOTS: AtomicUsize = AtomicUsize::new(0);

/// Records what the merged tactics half decided, before [`resolve_item_slots`]
/// runs. `None` means it is inactive and the question falls back to detecting a
/// separately installed copy.
pub fn record_builtin_item_slots(slots: Option<usize>) {
    BUILTIN_SLOTS.store(slots.map_or(0, |n| n + 1), Ordering::Relaxed);
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
///
/// For a hand-installed mod that is `<game>/mods`. For one installed from the
/// Workshop it is the Steam workshop content directory, which is somewhere else
/// entirely and is *not* under the game folder. Nothing here may assume which.
fn mods_dir() -> Option<PathBuf> {
    crate::config::dll_dir()?.parent().map(PathBuf::from)
}

/// The game's own directory, where `config/game/mods.json` lives.
///
/// Resolved from the running executable, not from this DLL's position. The two
/// agree only for a hand-installed mod: Steam puts a subscribed Workshop mod
/// under `steamapps/workshop/content/<appid>/<published_file_id>/`, which shares
/// no useful ancestor with `steamapps/common/<game>/`. Walking up from the DLL
/// therefore lands outside the game entirely, and every read of a game config
/// file fails with "not found" — permanently, for everyone who installed this
/// mod the ordinary way.
///
/// The executable is the game, wherever the game is, so it is the reliable
/// anchor. The old DLL-relative guess is kept as a fallback for the case where
/// the exe path cannot be read, and both are checked for the file that is
/// actually wanted rather than trusted blind.
fn game_root() -> Option<PathBuf> {
    let has_config = |dir: &Path| dir.join("config").join("game").join("mods.json").is_file();

    let from_exe = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from));
    if let Some(dir) = from_exe.filter(|dir| has_config(dir)) {
        return Some(dir);
    }

    mods_dir()
        .and_then(|dir| dir.parent().map(PathBuf::from))
        .filter(|dir| has_config(dir))
}

/// The directory `mod_id` is installed in, or `None` if it cannot be found.
///
/// The conventional `<game>/mods/<mod_id>` is tried first, then every root in
/// [`mod_search_roots`] is scanned. The scan exists because a Workshop install
/// has none of the properties the conventional path assumes: it lives outside
/// the game folder, and its directory is named for its published file id rather
/// than its mod id. So the folder is identified by reading `mod.mod_info` and
/// comparing the id it declares, which is the same thing the game does.
///
/// Either mod may be installed either way, independently — this one from the
/// Workshop and the companion by hand, or the reverse — so all four
/// combinations have to resolve, which is why the roots include both this mod's
/// neighbours and Steam's workshop tree.
fn companion_dir(mod_id: &str) -> Option<PathBuf> {
    if let Some(dir) = game_root().map(|root| root.join("mods").join(mod_id)) {
        if dir.is_dir() {
            return Some(dir);
        }
    }
    mod_search_roots()
        .iter()
        .find_map(|root| find_mod_dir(root, mod_id))
}

/// Directories that may contain mod folders, in the order worth trying.
///
/// `<game>/mods` covers a hand-installed mod. This mod's own parent covers a
/// Workshop neighbour. Its grandparent covers a Workshop item whose payload sits
/// one folder deep. The workshop tree is derived from the game root rather than
/// searched for: Steam lays out `steamapps/common/<game>` beside
/// `steamapps/workshop/content/<appid>`, so two steps up from the game and back
/// down finds it without guessing at drive letters or library folders.
///
/// Duplicates are dropped so a hand-installed pair does not read the same
/// directory three times.
fn mod_search_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut add = |dir: PathBuf| {
        if !roots.contains(&dir) {
            roots.push(dir);
        }
    };

    if let Some(root) = game_root() {
        add(root.join("mods"));
    }
    if let Some(dir) = mods_dir() {
        if let Some(parent) = dir.parent() {
            add(parent.to_path_buf());
        }
        add(dir);
    }
    if let Some(steamapps) =
        game_root().and_then(|root| root.parent().and_then(Path::parent).map(PathBuf::from))
    {
        let content = steamapps.join("workshop").join("content");
        if let Ok(entries) = std::fs::read_dir(&content) {
            for entry in entries.flatten().filter(|entry| entry.path().is_dir()) {
                add(entry.path());
            }
        }
    }
    roots
}

/// Finds the directory under `dir` whose `mod.mod_info` declares `mod_id`,
/// looking at `dir`'s children and then one level below them.
///
/// Two levels because that is the rule the game itself applies — its log says a
/// Workshop item is rejected when it has "no mod.mod_info at the install root or
/// in one-level child folders" — so a mod packaged with its payload in a
/// subfolder is loadable by the game and must be findable here too.
///
/// Unreadable entries are skipped: this is a search, and a directory that cannot
/// be read simply is not the answer.
fn find_mod_dir(dir: &Path, mod_id: &str) -> Option<PathBuf> {
    let declares = |candidate: &Path| {
        std::fs::read_to_string(candidate.join("mod.mod_info"))
            .ok()
            .and_then(|text| serde_json::from_str::<ModInfo>(&text).ok())
            .is_some_and(|info| info.mod_id == mod_id)
    };

    let entries = std::fs::read_dir(dir).ok()?;
    let children: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();

    if let Some(hit) = children.iter().find(|child| declares(child)) {
        return Some(hit.clone());
    }
    children.iter().find_map(|child| {
        std::fs::read_dir(child).ok()?.flatten().find_map(|entry| {
            let path = entry.path();
            (path.is_dir() && declares(&path)).then_some(path)
        })
    })
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
    let path = game_root()?.join("config").join("game").join("mods.json");
    let mods = read_parsed(&path, attempts, |text| {
        serde_json::from_str::<ModsFile>(text).ok()
    })?;
    Some(mods.enabled_mods.iter().any(|id| id == mod_id))
}

/// Schema of another mod's `mod.mod_info`, of which only the dependency list
/// matters here.
#[derive(Deserialize)]
struct ModInfo {
    /// Read as well as the dependencies because a Workshop mod's folder is named
    /// for its published file id, so this is the only place its mod id appears.
    #[serde(default)]
    mod_id: String,
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
    let Some(path) = companion_dir(mod_id).map(|dir| dir.join("mod.mod_info")) else {
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
    let path = companion_dir(ITEM_TACTICS_ID)?.join("4items.cfg");
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

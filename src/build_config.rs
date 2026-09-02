//! Item-build setter: the fixed build paths, keyed by champion id, that
//! `crate::item_build_hook` hands the engine through `StableItemBuildHook`.
//!
//! # `item-builds.json` is storage, not configuration
//!
//! The builds live in `item-builds.json` next to the mod DLL, but that file is
//! the in-game editor's backing store and nothing else. Editing it by hand is
//! not supported: the schema below documents what the editor writes so the code
//! that reads it can be understood, not a format for players to author.
//!
//! Two things follow, and both are load-bearing:
//!
//! - The file is read from disk **once**, and again only when the editor writes
//!   it. Nothing polls it for changes — see the cache below for what re-reading
//!   it per route call did to the ban/pick screen.
//! - A match's builds are therefore fixed when it starts. Nothing between "click
//!   play" and the final whistle can reload them.
//!
//! Extension seams (intentional TODOs):
//! - [`build_for_champion`] keys builds by champion id and [`Role`]. Lineup-aware
//!   selection — varying a champion's build by the enemy comp — would slot in
//!   there, using the `ally_champions`/`enemy_champions` the item-build hook
//!   already has in hand.

use serde::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, RwLock};

/// Schema of `item-builds.json`: the whole file is a map of champion id -> build
/// (there is no wrapper object).
///
/// Key = champion id, optionally suffixed with `@role` (`"lancer"`,
/// `"lancer@jungle"`); value = an ordered list of build slots.
///
/// A bare champion id is the [`Role`]-agnostic build, which is what every file
/// written before the Role column contains — so an old file loads unchanged, and
/// a file this version writes still loads in an older build of the mod, which
/// sees the suffixed keys as champions it does not have. A suffixed key wins over
/// the bare one when the champion is played in that role; see [`build_entry`]. Each slot is
/// either an item key (a *pinned* item) or JSON `null` (a *blank* slot the game's
/// AI fills). Pinned keys are resolved to their `radiant_` (tier 5) variant
/// (`radiant_key`), then to the game's internal key for renamed items
/// (`alias_key`). A route whose champion has no entry here is left as the game
/// generated it.
///
/// A build with no `null`s behaves exactly as before: the champion builds only
/// the listed items. Each `null` slot is filled with the next item the AI would
/// have built that the player did not already pin, so `["kraken_slayer", null,
/// null]` = Kraken Slayer plus the AI's two best complementary picks.
#[derive(Deserialize, Default)]
#[serde(transparent)]
pub struct BuildConfig {
    pub by_champion: HashMap<String, Vec<Option<String>>>,
}

impl BuildConfig {
    pub fn is_empty(&self) -> bool {
        self.by_champion.is_empty()
    }
}

/// Loads `item-builds.json` from next to the mod DLL.
///
/// Returns `Ok(None)` when the file is absent — the common, non-error case: the
/// mod ships inert and the file appears the first time the editor saves a build.
///
/// Callers want [`load_cached`]; this is the one place that touches the disk.
fn load() -> Result<Option<BuildConfig>, String> {
    let path = config_path()?;
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|error| format!("{}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

fn config_path() -> Result<PathBuf, String> {
    crate::config::dll_dir()
        .map(|dir| dir.join("item-builds.json"))
        .ok_or_else(|| "could not resolve mod directory".to_string())
}

// ---------------------------------------------------------------------------
//  File caches for the route hook
// ---------------------------------------------------------------------------
//
// `load` and `unique_items_enabled` were called straight off `hook::detour`, on
// the assumption noted at `PINS` that the route hook "fires a couple of times a
// match". It does not: the game builds routes as *every* match starts, and a
// league day's other fixtures sim on parallel rayon workers — so on a save with
// a developed world the detour runs many times over, concurrently, while the
// player is still in the ban/pick screen. Two file reads, two JSON parses and a
// full `HashMap` clone per call, all funnelled through global locks, is what
// made that phase stall.
//
// Both files are therefore read from disk exactly once — on the first read, and
// again only when the *editor* writes them, which is the only supported way to
// change either. There is deliberately no revalidation: the files are this mod's
// private storage for what the editor holds, not a config surface, so an edit
// made behind the mod's back is not a case to detect. Nothing re-reads them
// mid-match either, so the builds a match starts with are the builds it
// finishes with.

static CONFIG_CACHE: RwLock<Option<Arc<BuildConfig>>> = RwLock::new(None);

// `mod-settings.json` is cached in two atomics rather than behind the config
// lock, because one of its readers is the buy detour: `own_team_only_enabled`
// is asked once per buy decision, on every rayon worker at once, and a
// `RwLock` read there is contention for a value that changes only when the
// editor writes. `SETTING_UNSET` means "not read yet"; the first reader parses
// the file and fills both.
const SETTING_UNSET: u8 = 0;
const SETTING_OFF: u8 = 1;
const SETTING_ON: u8 = 2;

static UNIQUE_ITEMS: AtomicU8 = AtomicU8::new(SETTING_UNSET);
static OWN_TEAM_ONLY: AtomicU8 = AtomicU8::new(SETTING_UNSET);

/// Drops both file caches, so the next read reloads from disk.
///
/// The editor calls this after it writes — the one event that can change either
/// file — and nothing else has cause to. Do not call it from anywhere a match
/// could be running: re-reading mid-match is exactly what "the build is fixed
/// once you click play" rules out.
pub fn invalidate_caches() {
    if let Ok(mut cache) = CONFIG_CACHE.write() {
        *cache = None;
    }
    UNIQUE_ITEMS.store(SETTING_UNSET, Ordering::Relaxed);
    OWN_TEAM_ONLY.store(SETTING_UNSET, Ordering::Relaxed);
}

/// `item-builds.json`, parsed once and held.
///
/// This is what the route hook calls. A malformed file yields an empty config
/// rather than an error: the hook's only response to one was to ignore it and
/// leave the game's routes alone, which is what an empty config does.
///
/// Loading also publishes [`PINS`], which is why nothing else needs to: the
/// snapshot the buy detour reads and the config the hook applies come from the
/// same parse.
pub fn load_cached() -> Arc<BuildConfig> {
    if let Ok(cache) = CONFIG_CACHE.read() {
        if let Some(config) = cache.as_ref() {
            return config.clone();
        }
    }

    let Ok(mut cache) = CONFIG_CACHE.write() else {
        return Arc::new(BuildConfig::default());
    };
    // Another worker may have loaded it while this one waited for the lock.
    if let Some(config) = cache.as_ref() {
        return config.clone();
    }

    let value = Arc::new(match load() {
        Ok(config) => {
            let config = config.unwrap_or_default();
            // Published here rather than from the hook: this is the only place
            // the file is read now, so it is the only place that can keep the
            // buy detour's snapshot in step with it. A config the editor emptied
            // publishes an empty set, which is what stops it applying.
            publish_pins(&config);
            config
        }
        // Malformed — only reachable if the file was corrupted, since the editor
        // is the only supported writer. The game's routes are left alone and the
        // buy detour keeps whatever it last had, rather than a bad file silently
        // wiping every build.
        Err(_) => BuildConfig::default(),
    });
    *cache = Some(value.clone());
    value
}

/// Item slots the editor exposes per champion, matching the columns the
/// strategy screen shows: the vanilla three, or four when the tactics half is
/// active and `4items.cfg` asks for four.
///
/// This is a first-hand answer, not a detection: the tactics half is the code
/// that installs the byte patches making a fourth slot exist, so if it says
/// four there are four.
///
/// A build written with four slots and later opened in 3-slot mode keeps its
/// fourth item in `item-builds.json` — [`load_champion_rows`] only pads short
/// builds, never truncates long ones, so switching back restores the build
/// intact — but the editor stops showing that item and [`apply`] stops sending
/// it, because the game has nowhere to put it.
pub fn picker_slots() -> usize {
    crate::tactics::driver::picker_slots()
}

/// The role a build is written for.
///
/// `Any` is the default and the one every build had before this column existed:
/// it applies wherever the champion is played. A role-specific build wins over
/// `Any` when the champion is actually played in that role, which is the whole
/// point of the column.
///
/// The order matches the game's own `Position` (and the stable API's `LaneV1`)
/// from `Top` on, so a lane code indexes straight into [`Role::LANES`].
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    #[default]
    Any,
    Top,
    Jungle,
    Mid,
    Bottom,
    Support,
}

impl Role {
    /// Every role, in the order the editor's dropdown offers them.
    pub const ALL: [Role; 6] = [
        Role::Any,
        Role::Top,
        Role::Jungle,
        Role::Mid,
        Role::Bottom,
        Role::Support,
    ];

    /// The five real lanes, indexed by lane code (`Top` = 0), which is the order
    /// both `LaneV1` and the route hook's `team1` use.
    pub const LANES: [Role; 5] = [
        Role::Top,
        Role::Jungle,
        Role::Mid,
        Role::Bottom,
        Role::Support,
    ];

    /// Label for the editor's Role column.
    pub fn label(self) -> &'static str {
        match self {
            Role::Any => "Any",
            Role::Top => "Top",
            Role::Jungle => "Jungle",
            Role::Mid => "Mid",
            Role::Bottom => "Bot",
            Role::Support => "Support",
        }
    }

    /// The role a lane code names, or [`Role::Any`] for a code the host does not
    /// map — an unknown lane must not silently answer to a real role's build.
    pub fn from_lane_code(code: usize) -> Role {
        Role::LANES.get(code).copied().unwrap_or(Role::Any)
    }

    /// Suffix this role carries in an `item-builds.json` key.
    ///
    /// `Any` has none: its build stays under the bare champion id, which is
    /// exactly what every file written before roles existed contains. That is
    /// what makes the format change invisible to an existing file — and what
    /// makes a file this version writes still readable by one that predates it,
    /// which sees the suffixed keys as champions it does not have and ignores
    /// them.
    fn suffix(self) -> Option<&'static str> {
        match self {
            Role::Any => None,
            Role::Top => Some("top"),
            Role::Jungle => Some("jungle"),
            Role::Mid => Some("mid"),
            Role::Bottom => Some("bot"),
            Role::Support => Some("support"),
        }
    }

    fn from_suffix(suffix: &str) -> Option<Role> {
        Role::ALL
            .iter()
            .copied()
            .find(|role| role.suffix() == Some(suffix))
    }
}

/// Splits a stored key into the champion it names and the role it is written
/// for. A key with no `@` — or with a suffix this version does not know — is an
/// [`Role::Any`] build for the whole key, so an unrecognised suffix degrades to
/// "some champion we have no build for" rather than to the wrong role.
pub fn split_build_key(key: &str) -> (&str, Role) {
    match key.rsplit_once(ROLE_SEPARATOR) {
        Some((champion, suffix)) => match Role::from_suffix(suffix) {
            Some(role) => (champion, role),
            None => (key, Role::Any),
        },
        None => (key, Role::Any),
    }
}

/// The key a row is stored under: the champion id for [`Role::Any`], and
/// `champion@role` for anything else.
pub fn build_key(champion: &str, role: Role) -> String {
    match role.suffix() {
        Some(suffix) => format!("{champion}{ROLE_SEPARATOR}{suffix}"),
        None => champion.to_string(),
    }
}

/// Separator between champion and role in a stored key. `@` cannot occur in a
/// champion id, which are all snake_case, so the split is unambiguous.
const ROLE_SEPARATOR: char = '@';

/// One editable row of the in-game editor: a champion, the role it is for, and
/// its three slots.
///
/// The champion is optional because a freshly added row has not been assigned
/// one yet. Such a row is kept in the editor but never written, since a build
/// with nothing to key it by is not a build.
#[derive(Default, Clone, Debug, PartialEq)]
pub struct ChampionRow {
    pub champion: Option<String>,
    /// Which role this build is for. Defaults to [`Role::Any`], so a row added
    /// without touching the column behaves as every row did before.
    pub role: Role,
    pub slots: Vec<Option<String>>,
}

impl ChampionRow {
    /// Whether the row contributes a build: a champion, and at least one pinned
    /// item. A row of nothing but AI slots is a no-op, identical to not listing
    /// the champion at all.
    pub fn is_complete(&self) -> bool {
        self.champion.is_some() && self.slots.iter().any(Option::is_some)
    }
}

/// Reads `item-builds.json` as an ordered row list for the editor.
///
/// File order is preserved (serde_json is built with `preserve_order`), so rows
/// do not shuffle between visits. This reads the file directly rather than going
/// through [`load_cached`] for that reason: the cached form is a `HashMap` and
/// has already lost the order the editor wrote.
///
/// A missing or malformed file yields no rows rather than an error: the editor
/// is additive, and a bad file must never cost the player the routes the game
/// already produced.
pub fn load_champion_rows() -> Vec<ChampionRow> {
    let Ok(path) = config_path() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&text) else {
        return Vec::new();
    };

    map.into_iter()
        .map(|(key, value)| {
            let (champion, role) = split_build_key(&key);
            let mut slots: Vec<Option<String>> = value
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .map(|item| item.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            // Pad to the editable width, but never cut a longer build down:
            // a four-item build read in 3-slot mode has to survive the round
            // trip so it is still there when four slots come back. `apply` is
            // where the unusable tail is dropped.
            if slots.len() < picker_slots() {
                slots.resize(picker_slots(), None);
            }
            ChampionRow {
                champion: Some(champion.to_string()),
                role,
                slots,
            }
        })
        .collect()
}

/// Writes the complete rows back to `item-builds.json`, in row order.
///
/// Incomplete rows are skipped and a later row with the same champion *and
/// role* wins, so one champion can hold an `Any` build alongside a build for
/// each role it is played in. Returns false when it could not be written.
pub fn save_champion_rows(rows: &[ChampionRow]) -> bool {
    let Ok(path) = config_path() else {
        return false;
    };
    let mut map = serde_json::Map::new();
    for row in rows.iter().filter(|row| row.is_complete()) {
        let Some(champion) = &row.champion else {
            continue;
        };
        let slots: Vec<serde_json::Value> = row
            .slots
            .iter()
            .map(|slot| match slot {
                Some(key) => serde_json::Value::String(key.clone()),
                None => serde_json::Value::Null,
            })
            .collect();
        map.insert(
            build_key(champion, row.role),
            serde_json::Value::Array(slots),
        );
    }

    let written = serde_json::to_string_pretty(&map)
        .ok()
        .and_then(|text| std::fs::write(path, text + "\n").ok())
        .is_some();

    // Nothing else re-reads the file — not the route hook, not the buy detour,
    // which works from the `PINS` snapshot — so this write is the only event
    // that can make either notice a change, and it has to say so. Reloading
    // eagerly rather than leaving the cache empty keeps the parse on the
    // editor's thread, where a few hundred microseconds are free, instead of on
    // whichever sim worker calls the route hook first.
    if written {
        invalidate_caches();
        let _ = load_cached();
    }
    written
}

/// Champion roster the hook was handed, for the editor to offer.
///
/// A static, not a file: the detour and the client extension run in the *same
/// process*, so handing a list from one to the other never needed to touch the
/// disk. It is only a fallback — the editor asks the client for
/// `champion_names()` first — and it exists because that call returns nothing
/// when made from inside a UI event handler, the same restriction that makes
/// `setting_get_json` return None there.
///
/// The hook receives the whole roster as `champion_ids`, and those are exactly
/// the strings a build is keyed by, so it is the right list by construction
/// rather than by coincidence. It includes champions added by *other* mods,
/// which is why the editor must not filter it against the base game's champion
/// text (see `strategy_ui::load_champions`).
static CHAMPION_ROSTER: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Records the roster from inside the detour.
///
/// The roster cannot change without a restart, so an unchanged list returns
/// without allocating: the detour runs once per team per match *including the
/// league's background fixtures*, on parallel workers, and copying 60-odd
/// `String`s under a global lock each time is contention for no new information.
pub fn record_champion_roster(ids: &[String]) {
    if let Ok(mut roster) = CHAMPION_ROSTER.lock() {
        if roster.as_slice() == ids {
            return;
        }
        roster.clear();
        roster.extend_from_slice(ids);
    }
}

/// The recorded roster. Empty until the first match simulates, which happens
/// before the player can reach their own strategy screen.
pub fn champion_roster() -> Vec<String> {
    CHAMPION_ROSTER
        .lock()
        .map(|roster| roster.clone())
        .unwrap_or_default()
}

/// `item-builds.json`, as the *simulation* side reads it.
///
/// The buy detour that applies these builds per athlete fires per buy decision,
/// hundreds of thousands of times a match, and must never touch the disk. So the
/// file is published here when it loads, and that side takes a snapshot.
///
/// This used to say the route hook could read the file itself because it "fires
/// a couple of times a match". It does not — see the cache above — so the load
/// and the publication are now one event, in [`load_cached`], and the snapshot
/// changes only when the editor saves.
static PINS: Mutex<Option<Arc<HashMap<String, Vec<Option<String>>>>>> = Mutex::new(None);

/// Publishes the current builds for the buy detour. An empty config publishes an
/// empty set — builds the editor removed must stop applying, not keep the last
/// ones alive.
fn publish_pins(config: &BuildConfig) {
    let builds = config.by_champion.clone();
    if let Ok(mut pins) = PINS.lock() {
        *pins = Some(Arc::new(builds));
    }
}

fn pins() -> Option<Arc<HashMap<String, Vec<Option<String>>>>> {
    PINS.lock().ok()?.clone()
}

/// Whether this champion has any pinned item in the role it is being played,
/// which is what makes it worth looking up slot by slot.
///
/// The role comes from [`role_for_champion`] rather than from the caller: the
/// buy detour reads this from inside an athlete hook that has the champion key
/// and nothing else, so the lineup snapshot is what supplies the missing half.
pub fn has_pins(champion: &str) -> bool {
    let role = role_for_champion(champion);
    pins().is_some_and(|pins| {
        pin_entry(&pins, champion, role).is_some_and(|build| build.iter().any(Option::is_some))
    })
}

/// [`build_entry`] over the pin snapshot: the role's build first, `Any` second.
fn pin_entry<'a>(
    pins: &'a HashMap<String, Vec<Option<String>>>,
    champion: &str,
    role: Role,
) -> Option<&'a Vec<Option<String>>> {
    if role != Role::Any {
        if let Some(build) = pins.get(&build_key(champion, role)) {
            return Some(build);
        }
    }
    pins.get(champion)
}

// ---------------------------------------------------------------------------
//  Lineup roles, for the half that only knows the champion
// ---------------------------------------------------------------------------
//
// The stable item-build hook is told the lane outright. The buy detour is not:
// it resolves pins from inside an athlete spawn hook whose only handle on
// identity is the champion key, and finding the position on the athlete struct
// would mean a new offset in the part of this mod that breaks on every game
// update.
//
// It does not need one. The route detour is handed `team1` position-ordered
// (Top = 0), so recording champion -> role there hands the buy detour the same
// answer with no memory work at all. The map is best effort by construction:
// routes are built for every fixture on a league day, on parallel workers, so a
// champion played in two simultaneous matches at two positions keeps whichever
// was recorded last. The cost of losing that race is one build applied at the
// wrong role, in a match the player is not watching.

static LINEUP_ROLES: Mutex<Option<Arc<HashMap<String, Role>>>> = Mutex::new(None);

/// Records a lineup's champion -> role mapping from a position-ordered roster.
///
/// Entries beyond the five lanes are ignored rather than wrapped: a longer
/// roster than the map has positions for is not something to guess about.
pub fn record_lineup_roles(champions: &[String]) {
    let mut roles: HashMap<String, Role> = match lineup_roles() {
        Some(existing) => (*existing).clone(),
        None => HashMap::new(),
    };
    let mut changed = false;
    for (index, champion) in champions.iter().enumerate() {
        let role = Role::from_lane_code(index);
        if role == Role::Any {
            continue;
        }
        if roles.get(champion) != Some(&role) {
            roles.insert(champion.clone(), role);
            changed = true;
        }
    }
    if !changed {
        return;
    }
    if let Ok(mut cache) = LINEUP_ROLES.lock() {
        *cache = Some(Arc::new(roles));
    }
}

fn lineup_roles() -> Option<Arc<HashMap<String, Role>>> {
    LINEUP_ROLES.lock().ok()?.clone()
}

/// The role a champion was last seen playing, or [`Role::Any`] when no lineup
/// has named it — which is also what makes the `Any` build the answer.
pub fn role_for_champion(champion: &str) -> Role {
    lineup_roles()
        .and_then(|roles| roles.get(champion).copied())
        .unwrap_or(Role::Any)
}

/// The pinned item for one slot, exactly as written in the file.
///
/// The verbatim key matters on its own: it is what tells a vanilla tier 5
/// (`"warlords_final_judgement"`, whose id *is* its catalog index) from a mod
/// item that has to be found by name.
pub fn pinned_key_raw(champion: &str, slot: usize) -> Option<String> {
    let pins = pins()?;
    let role = role_for_champion(champion);
    pin_entry(&pins, champion, role)?.get(slot)?.clone()
}

/// The pinned item for one slot, normalized the way [`resolve_key`] normalizes
/// it — `radiant_` variant first, then through [`alias_key`] — so a build
/// authored with a plain LoL name resolves the same here as it does on the
/// route hook.
pub fn pinned_key(champion: &str, slot: usize) -> Option<String> {
    let raw = pinned_key_raw(champion, slot)?;
    Some(alias_key(radiant_key(&raw).as_ref()).to_string())
}

/// Writes `mod-settings.json` from the toggles as they stand, with `apply`
/// changing the one the player just clicked.
///
/// The whole file is rewritten every time, so every key has to be written out —
/// a partial write would silently reset the other toggle to its default.
fn save_settings(apply: impl FnOnce(&mut ModSettings)) -> bool {
    let mut settings = ModSettings {
        unique_items: unique_items_enabled(),
        own_team_only: own_team_only_enabled(),
    };
    apply(&mut settings);
    let path = crate::config::mod_dir().join("mod-settings.json");
    let text = format!(
        "{{\n  \"unique_items\": {},\n  \"own_team_only\": {}\n}}\n",
        settings.unique_items, settings.own_team_only
    );
    let written = std::fs::write(path, text).is_ok();
    if written {
        invalidate_caches();
    }
    written
}

/// Writes `unique_items` to `mod-settings.json`, the toggle
/// [`unique_items_enabled`] reads back on every hook call.
pub fn set_unique_items(enabled: bool) -> bool {
    save_settings(|settings| settings.unique_items = enabled)
}

/// Writes `own_team_only` to `mod-settings.json` — see
/// [`own_team_only_enabled`] for what the two sides of it mean.
pub fn set_own_team_only(enabled: bool) -> bool {
    save_settings(|settings| settings.own_team_only = enabled)
}

/// Schema of `mod-settings.json`: behavior toggles managed by the item build
/// editor. An absent file (the common case) means every toggle takes its
/// default.
#[derive(Deserialize)]
struct ModSettings {
    #[serde(default = "default_true")]
    unique_items: bool,
    #[serde(default)]
    own_team_only: bool,
}

fn default_true() -> bool {
    true
}

/// Reads one cached toggle out of `mod-settings.json`, parsing the file on the
/// first read of either.
///
/// Cached in an atomic rather than a lock because [`own_team_only_enabled`] is
/// on the buy detour's path — see the cache declarations. Flipping a toggle in
/// the editor still applies to the next match: the editor invalidates the cache
/// when it writes.
fn setting(cache: &AtomicU8, read: impl Fn(&ModSettings) -> bool, default: bool) -> bool {
    match cache.load(Ordering::Relaxed) {
        SETTING_ON => return true,
        SETTING_OFF => return false,
        _ => {}
    }

    let value = crate::config::dll_dir()
        .map(|dir| dir.join("mod-settings.json"))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<ModSettings>(&text).ok())
        .map(|settings| read(&settings))
        .unwrap_or(default);
    cache.store(
        if value { SETTING_ON } else { SETTING_OFF },
        Ordering::Relaxed,
    );
    value
}

/// Whether unique-build enforcement is enabled: `unique_items` in
/// `mod-settings.json` next to the mod DLL. Defaults to enforced when the file
/// is absent or malformed, so players opt *out* via the editor toggle.
pub fn unique_items_enabled() -> bool {
    setting(&UNIQUE_ITEMS, |settings| settings.unique_items, true)
}

/// Whether configured builds are restricted to the player's own team:
/// `own_team_only` in `mod-settings.json`. Defaults to off — a build applies to
/// whoever plays the champion, both teams — which is the behaviour every version
/// up to now had and the only one the stable item-build hook can express.
///
/// Turning it on moves the work to the other half of the mod. The hook is told
/// the champion but not whose team it is playing for, so it stops setting builds
/// entirely and the native buy detour pins the same items per athlete instead,
/// under the athlete-id team gate that already scopes the 4th item
/// (`tactics::is_my_athlete`). The two must never both be applying builds — see
/// `item_build_hook::decide_build` and the injection in `tactics`.
pub fn own_team_only_enabled() -> bool {
    setting(&OWN_TEAM_ONLY, |settings| settings.own_team_only, false)
}

/// The configured build for one champion, as item indices.
///
/// `resolve` turns an item key into an index in whatever list the caller is
/// working against — `StableItemBuildContext::item_index` for the item-build
/// hook. Keeping it a closure is what makes this independent of *how* the caller
/// sees the item pool, which is the whole difference between the stable hook and
/// the detour this used to serve.
///
/// `ai_build` is the build the engine picked for the same champion; it fills the
/// blank (`null`) slots. Returns `None` when the champion has no entry, which
/// callers must read as "leave the engine's build alone" — distinct from
/// `Some(vec![])`, a configured build whose every key failed to resolve.
///
/// Unknown item keys are skipped rather than aborting, so one typo does not
/// discard the rest of a build.
pub fn build_for_champion(
    config: &BuildConfig,
    champion: &str,
    role: Role,
    resolve: impl Fn(&str) -> Option<usize>,
    ai_build: &[usize],
) -> Option<Vec<usize>> {
    let build = build_entry(config, champion, role)?;
    // A build may be longer than the game has slots for — the file keeps a
    // fourth item while 3-slot mode is on, so that switching back restores the
    // build. Sending that item anyway would hand the game a slot it cannot put
    // anywhere.
    let usable = build.len().min(picker_slots());
    Some(merge_build(&build[..usable], ai_build, &resolve))
}

/// The build a champion uses in `role`: the role's own if one is written, and
/// the [`Role::Any`] build otherwise.
///
/// This ordering is the whole contract of the Role column — a build written for
/// Jungle beats the catch-all when the champion is actually jungling, and the
/// catch-all still covers every role nobody wrote a build for.
fn build_entry<'a>(
    config: &'a BuildConfig,
    champion: &str,
    role: Role,
) -> Option<&'a Vec<Option<String>>> {
    if role != Role::Any {
        if let Some(build) = config.by_champion.get(&build_key(champion, role)) {
            return Some(build);
        }
    }
    config.by_champion.get(champion)
}

/// Resolves one configured item key to a pool index. The key is normalized to
/// its `radiant_` variant and run through `alias_key` first, which is what lets
/// builds be authored with plain LoL names (`"collector"`); only if that misses
/// is it tried verbatim, so a game-internal key (`"warlords_final_judgement"`,
/// or any vanilla tier 5) still resolves as written. Unknown keys return `None`
/// (skipped rather than aborting the build).
///
/// The `radiant_` attempt comes FIRST and the verbatim one is the fallback.
/// Order matters: `"liandrys_torment"` in an existing `item-builds.json` means
/// the radiant item, but a base item of that exact key also exists, so trying
/// verbatim first would silently downgrade every build in that file to its base
/// tier. The fallback exists only for keys with no radiant variant — the vanilla
/// tier 5s the in-game picker offers, like `"warlords_final_judgement"`.
fn resolve_key(key: &str, resolve: &impl Fn(&str) -> Option<usize>) -> Option<usize> {
    let radiant = radiant_key(key);
    if let Some(index) = resolve(alias_key(radiant.as_ref())) {
        return Some(index);
    }
    resolve(key)
}

/// Builds the final route from a configured build and the route the AI generated
/// for the same champion (`ai_route`). Pinned slots (`Some`) use the player's
/// item; blank slots (`None`) are filled, in order, with the AI's own picks that
/// the player did not already pin. Unresolvable pinned keys and exhausted AI
/// picks simply drop their slot, so one typo or an over-long build never aborts
/// the rest.
fn merge_build(
    build: &[Option<String>],
    ai_route: &[usize],
    resolve: &impl Fn(&str) -> Option<usize>,
) -> Vec<usize> {
    let pinned: std::collections::HashSet<usize> = build
        .iter()
        .flatten()
        .filter_map(|key| resolve_key(key, resolve))
        .collect();
    let mut ai_fill = ai_route.iter().copied().filter(|i| !pinned.contains(i));

    let mut route = Vec::with_capacity(build.len());
    for slot in build {
        match slot {
            Some(key) => {
                if let Some(index) = resolve_key(key, resolve) {
                    route.push(index);
                }
            }
            None => {
                if let Some(index) = ai_fill.next() {
                    route.push(index);
                }
            }
        }
    }
    route
}

/// Resolves a configured item key to its `radiant_` (tier 5) variant: keys that
/// do not already start with `radiant_` are prefixed with it. This lets builds
/// be written with base item names (`"collector"`) and always resolve to the
/// radiant item the mod registers (`"radiant_collector"`). Keys already starting
/// with `radiant_` are passed through unchanged.
fn radiant_key(key: &str) -> Cow<'_, str> {
    if key.starts_with("radiant_") {
        Cow::Borrowed(key)
    } else {
        Cow::Owned(format!("radiant_{key}"))
    }
}

/// Maps a `radiant_`-normalized item slug to the game's internal item key, for
/// renamed items whose registered key differs from the LoL name shown in
/// `text/item.i18n`. The input is the output of `radiant_key`, so both
/// `"bloodthirster"` and `"radiant_bloodthirster"` arrive here as
/// `"radiant_bloodthirster"` and resolve to the same internal key. Slugs with no
/// alias pass through unchanged (covers items whose registered key already
/// matches their slug, like `radiant_collector`).
///
/// Add a new arm per renamed item in `text/item.i18n` (lines 58-177): key the arm
/// on the `radiant_` form of the LoL name, value is the i18n object key.
/// Normalizes any spelling of an item back to the plain LoL slug the item
/// catalog is keyed by: the inverse of [`alias_key`] followed by dropping the
/// `radiant_` prefix. `"warlords_final_judgement"`, `"radiant_bloodthirster"`
/// and `"bloodthirster"` all yield `"bloodthirster"`.
///
/// The in-game editor needs this because it reads item keys back out of the
/// game (where they are internal keys) but groups them with
/// [`crate::item_catalog`], which is keyed the way a player writes a build.
pub fn base_slug(key: &str) -> &str {
    unalias_key(key)
        .strip_prefix("radiant_")
        .unwrap_or_else(|| unalias_key(key))
}

/// Inverse of [`alias_key`]: the game's internal key for a renamed item back to
/// the `radiant_` form of its LoL name. Keys with no alias pass through.
fn unalias_key(key: &str) -> &str {
    match key {
        "warlords_final_judgement" => "radiant_bloodthirster",
        "storm_sovereign" => "radiant_phantom_dancer",
        "impregnable_fortress" => "radiant_thornmail",
        "veil_of_annihilation" => "radiant_dragons_claw",
        "prophet_of_the_abyss" => "radiant_ludens_tempest",
        "giants_horn_shard" => "radiant_sunfire_cape",
        other => other,
    }
}

fn alias_key(key: &str) -> &str {
    match key {
        // Radiant Bloodthirster
        "radiant_bloodthirster" => "warlords_final_judgement",
        // Radiant Phantom Dancer
        "radiant_phantom_dancer" => "storm_sovereign",
        // Radiant Thornmail
        "radiant_thornmail" => "impregnable_fortress",
        // Radiant Dragon's Claw
        "radiant_dragons_claw" => "veil_of_annihilation",
        // Radiant Luden's Tempest
        "radiant_ludens_tempest" => "prophet_of_the_abyss",
        // Radiant Sunfire Cape
        "radiant_sunfire_cape" => "giants_horn_shard",
        other => other,
    }
}

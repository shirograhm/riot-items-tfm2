//! Item-build setter: the builds keyed by champion (and role) that
//! `item_build_hook` hands the engine. `item-builds.json` beside the DLL is the
//! in-game editor's private storage, read once and again only when the editor
//! writes it, so a match's builds are fixed when it starts.

use serde::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

// `item-builds.json`: champion id, optionally `@role`, -> ordered slots, each
// an item key (a pin) or `null` (a blank the AI fills with its next unpinned
// pick). A bare id is the role-agnostic build, which a role-specific key beats
// in that role; pinned keys resolve to their `radiant_` variant, then to the
// game's key for renamed items. A row with a component focus is written as
// `{ "slots": [...], "focus": "offense" }` instead ([`StoredBuild`]).
#[derive(Deserialize, Default)]
#[serde(from = "HashMap<String, StoredBuild>")]
pub struct BuildConfig {
    pub by_champion: HashMap<String, Vec<Option<String>>>,
    // Build key -> its row's component focus, for the rows that have one.
    pub focus: HashMap<String, Focus>,
}

impl BuildConfig {
    pub fn is_empty(&self) -> bool {
        self.by_champion.is_empty()
    }
}

// One entry of `item-builds.json`: the slots alone, as every row was written
// before the component focus, or the slots with a focus. Only a row with a
// focus takes the second shape, so a file without one stays readable by older
// versions of the mod.
#[derive(Deserialize)]
#[serde(untagged)]
enum StoredBuild {
    Slots(Vec<Option<String>>),
    Row {
        slots: Vec<Option<String>>,
        #[serde(default)]
        focus: Option<String>,
    },
}

impl From<HashMap<String, StoredBuild>> for BuildConfig {
    fn from(stored: HashMap<String, StoredBuild>) -> Self {
        let mut config = BuildConfig::default();
        for (key, build) in stored {
            let slots = match build {
                StoredBuild::Slots(slots) => slots,
                StoredBuild::Row { slots, focus } => {
                    let focus = focus.as_deref().map_or(Focus::Neither, Focus::from_stored);
                    if focus != Focus::Neither {
                        config.focus.insert(key.clone(), focus);
                    }
                    slots
                }
            };
            config.by_champion.insert(key, slots);
        }
        config
    }
}

// Which components a row's champion builds its items from where an item can be
// built more than one way (Smart Builds rule 18, `route_choice`): the editor's
// Offense and Defense cells. `Neither` leaves it to the champion's own stats.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Focus {
    #[default]
    Neither,
    Offense,
    Defense,
}

impl Focus {
    // How `item-builds.json` names it; `Neither` is not written.
    fn stored(self) -> Option<&'static str> {
        match self {
            Focus::Neither => None,
            Focus::Offense => Some("offense"),
            Focus::Defense => Some("defense"),
        }
    }

    // The focus a stored name says; anything else is `Neither`.
    fn from_stored(name: &str) -> Focus {
        [Focus::Offense, Focus::Defense]
            .into_iter()
            .find(|focus| focus.stored() == Some(name))
            .unwrap_or_default()
    }
}

// Loads `item-builds.json`; `Ok(None)` when it is absent, which is normal until
// the editor first saves. Callers want [`load_cached`].
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

// -- file caches for the route hook ------------------------------------------
//
// The route hook runs for every match of a league day, concurrently, so
// `item-builds.json` is read from disk once and again only when the editor
// writes it. Nothing re-reads it mid-match, so a match finishes with the builds
// it started with.

static CONFIG_CACHE: RwLock<Option<Arc<BuildConfig>>> = RwLock::new(None);

// The build editor's two toggles, Smart Builds and own team only, kept in the
// save ([`sync_settings`]) and held in atomics because the buy detour reads one
// on every worker. Both default to on.
static SMART_BUILDS: AtomicBool = AtomicBool::new(true);
static OWN_TEAM_ONLY: AtomicBool = AtomicBool::new(true);
// The loaded save's toggles have been read.
static SETTINGS_LOADED: AtomicBool = AtomicBool::new(false);
// A toggle has changed since the save was last written to.
static SETTINGS_DIRTY: AtomicBool = AtomicBool::new(false);

// The keys the toggles are kept under, in the mod's namespace of the save.
const SMART_BUILDS_KEY: &str = "smart_builds";
const OWN_TEAM_ONLY_KEY: &str = "own_team_only";

// Drops the cached builds so the next read reloads the file. Only the editor
// calls this, after it writes; never call it while a match could be running.
pub fn invalidate_caches() {
    if let Ok(mut cache) = CONFIG_CACHE.write() {
        *cache = None;
    }
}

// `item-builds.json`, parsed once and held, for the route hook. A malformed
// file yields an empty config, which leaves the game's builds alone; loading
// also publishes [`PINS`] from the same parse.
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
            // Published here, the only place the file is read, so the buy
            // detour's pins match it; an emptied config publishes no pins.
            publish_pins(&config);
            config
        }
        // Malformed (the file was corrupted): leave the game's builds alone and
        // keep the buy detour's last pins.
        Err(_) => BuildConfig::default(),
    });
    *cache = Some(value.clone());
    value
}

// Item slots the editor shows per champion: the game's four plus a fifth and
// sixth that are this mod's, added by the native buy detour and byte patches
// (`tactics::buy_replace_ctx`, `patch_final_gate`); see [`game_slots`].
//
// A longer build in the file keeps its tail; [`load_champion_rows`] only pads
// short builds.
pub fn picker_slots() -> usize {
    6
}

// Item slots the game allocates per build, which is all the stable hook can
// fill. Slots past it exist only once the buy detour grows the build to
// [`picker_slots`].
pub fn game_slots() -> usize {
    4
}

// The role a build is written for; a role's own build beats `Any` when the
// champion plays that role. Ordered like the game's `Position` from `Top`, so a
// lane code indexes [`Role::LANES`].
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
    // Every role, in the order the editor's dropdown offers them.
    pub const ALL: [Role; 6] = [
        Role::Any,
        Role::Top,
        Role::Jungle,
        Role::Mid,
        Role::Bottom,
        Role::Support,
    ];

    // The five real lanes, indexed by lane code (`Top` = 0), which is the order
    // both `LaneV1` and the route hook's `team1` use.
    pub const LANES: [Role; 5] = [
        Role::Top,
        Role::Jungle,
        Role::Mid,
        Role::Bottom,
        Role::Support,
    ];

    // Label for the editor's Role column.
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

    // The role a lane code names, or [`Role::Any`] for a code the host does not
    // map — an unknown lane must not silently answer to a real role's build.
    pub fn from_lane_code(code: usize) -> Role {
        Role::LANES.get(code).copied().unwrap_or(Role::Any)
    }

    // Suffix this role carries in a build key. `Any` has none, so pre-role
    // files load unchanged and older mod versions just ignore suffixed keys.
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

// Splits a stored key into champion and role. No `@`, or an unknown suffix, is
// an `Any` build for the whole key.
pub fn split_build_key(key: &str) -> (&str, Role) {
    match key.rsplit_once(ROLE_SEPARATOR) {
        Some((champion, suffix)) => match Role::from_suffix(suffix) {
            Some(role) => (champion, role),
            None => (key, Role::Any),
        },
        None => (key, Role::Any),
    }
}

// The key a row is stored under: the champion id for [`Role::Any`], and
// `champion@role` for anything else.
pub fn build_key(champion: &str, role: Role) -> String {
    match role.suffix() {
        Some(suffix) => format!("{champion}{ROLE_SEPARATOR}{suffix}"),
        None => champion.to_string(),
    }
}

// Separator between champion and role in a stored key. `@` cannot occur in a
// champion id, which are all snake_case, so the split is unambiguous.
const ROLE_SEPARATOR: char = '@';

// One row of the in-game editor. The champion is `None` for a fresh row, which
// is kept in the editor but never written.
#[derive(Default, Clone, Debug, PartialEq)]
pub struct ChampionRow {
    pub champion: Option<String>,
    // Defaults to [`Role::Any`].
    pub role: Role,
    pub slots: Vec<Option<String>>,
    // Defaults to [`Focus::Neither`].
    pub focus: Focus,
}

impl ChampionRow {
    // Whether the row contributes a build: a champion, and at least one pin or
    // a component focus.
    pub fn is_complete(&self) -> bool {
        self.champion.is_some()
            && (self.slots.iter().any(Option::is_some) || self.focus != Focus::Neither)
    }
}

// Reads `item-builds.json` as ordered rows for the editor, straight from the
// file since the cached `HashMap` has lost the order. A missing or malformed
// file yields no rows.
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
            // A row with a focus is an object ([`StoredBuild`]).
            let (value, focus) = match value {
                serde_json::Value::Object(mut row) => {
                    let focus = row
                        .get("focus")
                        .and_then(serde_json::Value::as_str)
                        .map_or(Focus::Neither, Focus::from_stored);
                    (row.remove("slots").unwrap_or_default(), focus)
                }
                slots => (slots, Focus::Neither),
            };
            let mut slots: Vec<Option<String>> = value
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .map(|item| item.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            // Pad to the editable width, but never cut a longer build down.
            if slots.len() < picker_slots() {
                slots.resize(picker_slots(), None);
            }
            ChampionRow {
                champion: Some(champion.to_string()),
                role,
                slots,
                focus,
            }
        })
        .collect()
}

// Writes the complete rows back to `item-builds.json`, in row order.
//
// Incomplete rows are skipped and a later row with the same champion *and
// role* wins, so one champion can hold an `Any` build alongside a build for
// each role it is played in. Returns false when it could not be written.
pub fn save_champion_rows(rows: &[ChampionRow]) -> bool {
    let Ok(path) = config_path() else {
        return false;
    };
    let mut map = serde_json::Map::new();
    for row in rows.iter().filter(|row| row.is_complete()) {
        let Some(champion) = &row.champion else {
            continue;
        };
        let slots = serde_json::Value::Array(
            row.slots
                .iter()
                .map(|slot| match slot {
                    Some(key) => serde_json::Value::String(key.clone()),
                    None => serde_json::Value::Null,
                })
                .collect(),
        );
        // The slots alone unless the row has a focus ([`StoredBuild`]).
        let entry = match row.focus.stored() {
            Some(focus) => serde_json::json!({ "slots": slots, "focus": focus }),
            None => slots,
        };
        map.insert(build_key(champion, row.role), entry);
    }

    let written = serde_json::to_string_pretty(&map)
        .ok()
        .and_then(|text| std::fs::write(path, text + "\n").ok())
        .is_some();

    // This write is the only change the route hook and buy detour can see, so
    // reload right away, on the editor's thread rather than a sim worker.
    if written {
        invalidate_caches();
        let _ = load_cached();
    }
    written
}

// The champion roster the route detour was handed (every champion, other mods'
// included), for the editor, since `champion_names()` returns nothing inside a
// UI handler. A static, because the detour and the client run in one process.
static CHAMPION_ROSTER: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

// Records the roster from the detour; an unchanged list returns early, since
// the detour runs for every fixture on parallel workers.
pub fn record_champion_roster(ids: &[String]) {
    if let Ok(mut roster) = CHAMPION_ROSTER.lock() {
        if roster.as_slice() == ids {
            return;
        }
        roster.clear();
        roster.extend_from_slice(ids);
    }
}

// The recorded roster. Empty until the first match simulates, which happens
// before the player can reach their own strategy screen.
pub fn champion_roster() -> Vec<String> {
    CHAMPION_ROSTER
        .lock()
        .map(|roster| roster.clone())
        .unwrap_or_default()
}

// Where `champion` stands in the recorded roster, which is the id the game's
// item network knows it by: `get_item_builds_list` turns each lineup entry
// into its index in this same list before it scores anything. `None` until
// the first match simulates.
pub fn champion_roster_index(champion: &str) -> Option<usize> {
    CHAMPION_ROSTER
        .lock()
        .ok()?
        .iter()
        .position(|key| key == champion)
}

// Size of the recorded roster, for a per-frame caller that only needs to
// know whether it grew and should not copy it to find out.
pub fn champion_roster_len() -> usize {
    CHAMPION_ROSTER
        .lock()
        .map(|roster| roster.len())
        .unwrap_or(0)
}

// The buy detour's view of `item-builds.json`: the file's keys exactly as
// written, plus an index that survives the detour not knowing the lane. In
// multiplayer, the other players' files too ([`pin_snapshot`]).
pub(crate) struct PinSnapshot {
    // `champion` or `champion@role`, as the file has it.
    by_key: HashMap<String, Vec<Option<String>>>,
    // Champion -> its only key, for champions with exactly one build.
    // Precomputed because the buy detour looks pins up hundreds of thousands of
    // times a match.
    sole_key: HashMap<String, String>,
    // Key -> its row's component focus, for the rows that have one.
    focus: HashMap<String, Focus>,
}

impl PinSnapshot {
    fn of(config: &BuildConfig) -> PinSnapshot {
        let by_key = config.by_champion.clone();

        // Champions with exactly one build, by id alone; with two roles there
        // is no single answer.
        let sole_key = {
            let mut counts: HashMap<&str, usize> = HashMap::new();
            for key in by_key.keys() {
                *counts
                    .entry(key.split(ROLE_SEPARATOR).next().unwrap_or(key.as_str()))
                    .or_insert(0) += 1;
            }
            by_key
                .keys()
                .filter_map(|key| {
                    let champion = key.split(ROLE_SEPARATOR).next().unwrap_or(key.as_str());
                    (counts.get(champion) == Some(&1)).then(|| (champion.to_string(), key.clone()))
                })
                .collect()
        };

        PinSnapshot {
            by_key,
            sole_key,
            focus: config.focus.clone(),
        }
    }

    // How many rows the builds have, for the multiplayer log.
    pub(crate) fn rows(&self) -> usize {
        self.by_key.len()
    }

    // Whether any row has a component focus.
    pub(crate) fn any_focus(&self) -> bool {
        !self.focus.is_empty()
    }
}

// `item-builds.json` as the buy detour reads it: published when the file loads
// ([`load_cached`]), since that detour runs per buy decision and must never
// touch the disk.
static PINS: Mutex<Option<Arc<PinSnapshot>>> = Mutex::new(None);

// Publishes the builds for the buy detour; an empty config publishes nothing,
// so removed builds stop applying.
fn publish_pins(config: &BuildConfig) {
    let snapshot = PinSnapshot::of(config);
    ANY_FOCUS.store(snapshot.any_focus(), Ordering::Relaxed);
    if let Ok(mut pins) = PINS.lock() {
        *pins = Some(Arc::new(snapshot));
    }
    PINS_GENERATION.fetch_add(1, Ordering::Relaxed);
}

// Another player's builds, as `crate::multiplayer` hears them: their
// `item-builds.json` ([`own_builds_json`]). `None` when it does not read.
pub(crate) fn pin_snapshot(builds: &serde_json::Value) -> Option<Arc<PinSnapshot>> {
    let config: BuildConfig = serde_json::from_value(builds.clone()).ok()?;
    Some(Arc::new(PinSnapshot::of(&config)))
}

// This player's builds as `item-builds.json` has them, for `crate::multiplayer`
// to send the other players. In key order, so the same builds always read the
// same.
pub(crate) fn own_builds_json() -> serde_json::Value {
    let config = load_cached();
    let mut keys: Vec<&String> = config.by_champion.keys().collect();
    keys.sort();
    let map = keys
        .into_iter()
        .map(|key| {
            let slots = serde_json::json!(config.by_champion[key]);
            // The slots alone unless the row has a focus ([`StoredBuild`]).
            let entry = match config.focus.get(key).and_then(|focus| focus.stored()) {
                Some(focus) => serde_json::json!({ "slots": slots, "focus": focus }),
                None => slots,
            };
            (key.clone(), entry)
        })
        .collect();
    serde_json::Value::Object(map)
}

// The pins changed without this player's file changing: another player's
// builds came in or went. The memos keyed on [`pins_generation`] ask again.
pub(crate) fn pins_changed() {
    PINS_GENERATION.fetch_add(1, Ordering::Relaxed);
}

// Whether any row has a component focus, so the route picker can skip asking
// while none does.
static ANY_FOCUS: AtomicBool = AtomicBool::new(false);

pub fn any_focus() -> bool {
    ANY_FOCUS.load(Ordering::Relaxed) || crate::multiplayer::any_focus()
}

// The component focus of the row `champion` plays by in `role` (`Any` where the
// lane is not known), the row being found as its pins are ([`pin_key`]): a
// role's own row decides it, focus and all, even with none set.
pub fn focus_for(champion: &str, role: Role) -> Focus {
    if !any_focus() {
        return Focus::Neither;
    }
    pins()
        .and_then(|pins| {
            let key = pin_key(&pins, champion, role, role != Role::Any)?;
            pins.focus.get(key).copied()
        })
        .unwrap_or_default()
}

// Counts publications of [`PINS`], so the buy detour's memo
// (`tactics::BuyMemo`) picks up a build saved mid-match.
static PINS_GENERATION: AtomicU64 = AtomicU64::new(0);

// How many times the pins have been published. One atomic load.
pub fn pins_generation() -> u64 {
    PINS_GENERATION.load(Ordering::Relaxed)
}

// Whose builds a pin lookup reads: this player's `item-builds.json`, or in
// multiplayer another player's, by the team they manage (`crate::multiplayer`).
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum PinOwner {
    #[default]
    Own,
    Player(u64),
}

thread_local! {
    // Whose builds this thread's pin lookups read ([`pin_owner_scope`]).
    static PIN_OWNER: std::cell::Cell<PinOwner> = const { std::cell::Cell::new(PinOwner::Own) };
}

// Has this thread's pin lookups read `owner`'s builds until the guard drops,
// for the athlete a detour is handling. Every other caller reads its own.
pub fn pin_owner_scope(owner: PinOwner) -> PinOwnerScope {
    let previous = PIN_OWNER
        .try_with(|cell| cell.replace(owner))
        .unwrap_or_default();
    PinOwnerScope(previous)
}

pub struct PinOwnerScope(PinOwner);

impl Drop for PinOwnerScope {
    fn drop(&mut self) {
        let _ = PIN_OWNER.try_with(|cell| cell.set(self.0));
    }
}

fn pins() -> Option<Arc<PinSnapshot>> {
    match PIN_OWNER.try_with(std::cell::Cell::get).unwrap_or_default() {
        PinOwner::Own => PINS.lock().ok()?.clone(),
        PinOwner::Player(team) => crate::multiplayer::pins_of(team),
    }
}

// Whether this champion has any pin in the role it is played (from
// [`lane_for_champion`]).
pub fn has_pins(champion: &str) -> bool {
    let (role, known) = lane_for_champion(champion);
    pins().is_some_and(|pins| {
        pin_entry(&pins, champion, role, known)
            .is_some_and(|build| build.iter().any(Option::is_some))
    })
}

// The pin row the detours would apply to `champion` in `role`, for the stable
// hook under `own_team_only` (see [`remember_pinned_build`]). `role` is `Any`
// when the host did not say; pins are returned as written, since the mod never
// overwrites a pin.
pub fn pin_row(champion: &str, role: Role) -> Vec<Option<String>> {
    pins()
        .and_then(|pins| pin_entry(&pins, champion, role, role != Role::Any).cloned())
        .unwrap_or_default()
}

// [`build_entry`] over the pin snapshot, with a third arm: while the role is
// only a guess (`known` false), a champion's only build applies whatever role
// it names. The guess comes from a last-writer-wins map that background
// fixtures overwrite, which would otherwise drop role-specific pins under
// `own_team_only`.
fn pin_entry<'a>(
    pins: &'a PinSnapshot,
    champion: &str,
    role: Role,
    known: bool,
) -> Option<&'a Vec<Option<String>>> {
    pins.by_key.get(pin_key(pins, champion, role, known)?)
}

// The key of the row [`pin_entry`] finds.
fn pin_key<'a>(pins: &'a PinSnapshot, champion: &str, role: Role, known: bool) -> Option<&'a str> {
    if role != Role::Any {
        if let Some((key, _)) = pins.by_key.get_key_value(&build_key(champion, role)) {
            return Some(key.as_str());
        }
    }
    if let Some((key, _)) = pins.by_key.get_key_value(champion) {
        return Some(key.as_str());
    }
    if known {
        return None;
    }
    pins.sole_key.get(champion).map(String::as_str)
}

// -- lineup roles, for the half that only knows the champion -----------------
//
// The detours read the athlete's lane off the athlete ([`set_athlete_lane`]);
// this champion -> role map from the route detour's position-ordered `team1` is
// only the fallback. It is best effort, since fixtures on parallel workers
// overwrite each other.

static LINEUP_ROLES: Mutex<Option<Arc<HashMap<String, Role>>>> = Mutex::new(None);

// Whether the last item-build route call set up a lane or 5v5 test. Read by
// `tactics` for its team-id bookkeeping; the test's own athletes are found
// through [`TEST_WINDOW`] instead, which a league fixture does not close.
static TRAINING_MATCH: AtomicBool = AtomicBool::new(false);

// The last test's route call: when it came and both sides' champions. It opens
// a window in which the spawn hook finds the test's own match
// ([`note_test_spawn`]); league fixtures keep simming on the same champions, so
// the match, not the champion, marks the player's athletes.
static TEST_WINDOW: Mutex<Option<(Instant, Vec<String>)>> = Mutex::new(None);

// How long after a test's route call its athletes can still spawn; they come
// within a fraction of a second.
const TEST_WINDOW_SECS: u64 = 10;

// Records whether an item-build route call is for a lane or 5v5 test
// (`training`), with the champions of both sides. A test opens a new window
// and drops the tallies of the last one; a league fixture changes neither.
pub fn record_route_call<'a>(training: bool, champions: impl Iterator<Item = &'a str>) {
    if training {
        if let Ok(mut window) = TEST_WINDOW.lock() {
            *window = Some((Instant::now(), champions.map(str::to_string).collect()));
        }
        if let Ok(mut spawns) = TEST_SPAWNS.lock() {
            spawns.clear();
        }
    }
    TRAINING_MATCH.store(training, Ordering::Relaxed);
}

// Whether a lane or 5v5 test has been set up since the last league fixture.
// One atomic load.
pub fn training_match() -> bool {
    TRAINING_MATCH.load(Ordering::Relaxed)
}

// A match that spawned athletes while a test was on, by seed: the game plays a
// match in several provider copies sharing the seed, and the spawn hook sees
// only stack copies of athletes.
struct TestSpawns {
    seed: u64,
    // How many of its athletes play a champion of the test. Counted per
    // spawn, so a match spawned twice counts each athlete twice.
    matching: usize,
    // Whether one plays a champion the test does not have: a league fixture.
    foreign: bool,
}

static TEST_SPAWNS: Mutex<Vec<TestSpawns>> = Mutex::new(Vec::new());

// Seeds of the matches [`TEST_SPAWNS`] showed to be a test's. Several slots,
// because a league fixture can look like the test until its other champions
// spawn; lock-free for the buy detour's hot path, with `0` an empty slot.
static TEST_SEEDS: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
static NEXT_TEST_SEED: AtomicUsize = AtomicUsize::new(0);

// Records an athlete spawning in match `seed` while a test is on, and publishes
// the match as the test's once its athletes add up. A league fixture always has
// champions the test lacks, so it may be published and withdrawn before anyone
// buys.
pub fn note_test_spawn(seed: u64, champion: &str) {
    if seed == 0 {
        return;
    }
    let (on_test_champion, size, window_open) = {
        let Ok(window) = TEST_WINDOW.lock() else {
            return;
        };
        let Some((started, champions)) = window.as_ref() else {
            return;
        };
        (
            champions.iter().any(|known| known == champion),
            champions.len(),
            started.elapsed().as_secs() < TEST_WINDOW_SECS,
        )
    };
    if size == 0 {
        return;
    }
    let Ok(mut spawns) = TEST_SPAWNS.lock() else {
        return;
    };
    let index = match spawns.iter().position(|spawned| spawned.seed == seed) {
        Some(index) => index,
        // Past the window, only matches already tracked keep counting, so a
        // fixture that began with test champions can still be ruled out.
        None if !window_open => return,
        None => {
            spawns.push(TestSpawns {
                seed,
                matching: 0,
                foreign: false,
            });
            spawns.len() - 1
        }
    };
    let spawned = &mut spawns[index];
    if on_test_champion {
        spawned.matching += 1;
    } else {
        spawned.foreign = true;
    }
    let is_test = !spawned.foreign && spawned.matching >= size;
    let matching = spawned.matching;
    let published = is_test_match(seed);
    if is_test && !published {
        let slot = NEXT_TEST_SEED.fetch_add(1, Ordering::Relaxed) % TEST_SEEDS.len();
        TEST_SEEDS[slot].store(seed, Ordering::Relaxed);
        crate::own_team_log::line(|| {
            format!("test match: seed=0x{seed:x} found ({matching} athletes)")
        });
    } else if !is_test && published {
        for slot in &TEST_SEEDS {
            let _ = slot.compare_exchange(seed, 0, Ordering::Relaxed, Ordering::Relaxed);
        }
        crate::own_team_log::line(|| {
            format!("test match: seed=0x{seed:x} withdrawn ({champion} is not in the test)")
        });
    }
}

// Whether the match seeded `seed` is one [`note_test_spawn`] found to be a
// lane or 5v5 test: both of its sides are the player's. Lock-free: the buy
// detour's hot-path exit reads it.
pub fn is_test_match(seed: u64) -> bool {
    seed != 0
        && TEST_SEEDS
            .iter()
            .any(|slot| slot.load(Ordering::Relaxed) == seed)
}

// Records a lineup's champion -> role, as the route hook reads each one's
// position (`hook::lineup_roles`); a champion of no known role is left out.
pub fn record_lineup_roles(champions: &[(String, Role)]) {
    let mut roles: HashMap<String, Role> = match lineup_roles() {
        Some(existing) => (*existing).clone(),
        None => HashMap::new(),
    };
    let mut changed = false;
    for &(ref champion, role) in champions {
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

thread_local! {
    // The lane of the athlete this thread's spawn or buy detour is handling,
    // read off the athlete; each detour sets it first ([`set_athlete_lane`]).
    static ATHLETE_LANE: std::cell::Cell<Option<Role>> = const { std::cell::Cell::new(None) };
}

// Tells [`role_for_champion`] the lane of the athlete this thread is about to
// resolve pins for, or `None` when it could not be read (the guess then
// stands). Set on every detour call, never left from the last athlete.
pub fn set_athlete_lane(lane: Option<Role>) {
    ATHLETE_LANE.with(|cell| cell.set(lane));
}

fn lineup_roles() -> Option<Arc<HashMap<String, Role>>> {
    LINEUP_ROLES.lock().ok()?.clone()
}

// The lane pins for `champion` resolve in, and whether it is known: the
// athlete's own (see [`set_athlete_lane`]), else the lane the champion was
// last seen playing, else [`Role::Any`] — which is also what makes the `Any`
// build the answer.
fn lane_for_champion(champion: &str) -> (Role, bool) {
    if let Some(lane) = ATHLETE_LANE.with(std::cell::Cell::get) {
        return (lane, true);
    }
    let guess = lineup_roles()
        .and_then(|roles| roles.get(champion).copied())
        .unwrap_or(Role::Any);
    (guess, false)
}

// The role `champion` is played in, as [`lane_for_champion`] finds it.
pub fn role_for_champion(champion: &str) -> Role {
    lane_for_champion(champion).0
}

// The pin row the detours apply to `champion`, one entry per picker slot, in
// the lane [`pinned_key_raw`] resolves: the row [`pin_row`] gives the stable
// hook, padded the same way, when both find the same lane.
pub fn athlete_pin_row(champion: &str) -> Vec<Option<String>> {
    (0..picker_slots())
        .map(|slot| pinned_key_raw(champion, slot))
        .collect()
}

// Whether a build slot past the game's four — the 5th or 6th — is open for
// Smart Builds' boots: one that no pin in `row` holds, in a build the buy
// detour will grow that far. `row` is a pin row as [`pin_row`] gives it.
pub fn later_slot_open(row: &[Option<String>]) -> bool {
    (game_slots()..picker_slots()).any(|slot| row.get(slot).map_or(true, Option::is_none))
        && crate::tactics::builds_grow_past_four()
}

// The boots Smart Builds last picked per champion. Only the stable hook and the
// training detour see the enemy lineup, so the buy detour takes its
// 5th/6th-slot boots from here.
static RULE_BOOTS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

// Records `boots` as the pair Smart Builds picked for `champion`.
pub fn remember_rule_boots(champion: &str, boots: &str) {
    let Ok(mut picked) = RULE_BOOTS.lock() else {
        return;
    };
    match picked.iter_mut().find(|(known, _)| known == champion) {
        Some((_, known)) => *known = boots.to_string(),
        None => picked.push((champion.to_string(), boots.to_string())),
    }
}

// The pair of boots Smart Builds last picked for `champion`, when it has. Not in
// multiplayer, where only the host's server picks them: every game then takes
// the pair picked without the enemy lineup.
pub fn rule_boots(champion: &str) -> Option<String> {
    if crate::multiplayer::active() {
        return None;
    }
    let picked = RULE_BOOTS.lock().ok()?;
    picked
        .iter()
        .find(|(known, _)| known == champion)
        .map(|(_, boots)| boots.clone())
}

// A pin-aware build waiting for the spawn injector: under `own_team_only`,
// the build the stable hook would have given `champion` had it known it was
// the player's (`to`), next to the one it gave instead (`from`).
struct PinnedBuild {
    champion: String,
    row: Vec<Option<String>>,
    from: Vec<String>,
    to: Vec<String>,
}

// Newest last, capped: every fixture's athletes land here, and a spawn follows
// its own match's decisions closely.
static PINNED_BUILDS: Mutex<Vec<PinnedBuild>> = Mutex::new(Vec::new());
const PINNED_BUILDS_CAP: usize = 64;

// Records the build `champion` with pins `row` gets on the player's team when
// the stable hook hands it `from`. Replaces an older entry for the same three,
// so a match whose pins change nothing clears a stale one.
pub fn remember_pinned_build(
    champion: &str,
    row: Vec<Option<String>>,
    from: Vec<String>,
    to: Vec<String>,
) {
    let Ok(mut builds) = PINNED_BUILDS.lock() else {
        return;
    };
    builds.retain(|build| !(build.champion == champion && build.row == row && build.from == from));
    if builds.len() >= PINNED_BUILDS_CAP {
        builds.remove(0);
    }
    builds.push(PinnedBuild {
        champion: champion.to_string(),
        row,
        from,
        to,
    });
}

// The pin-aware build for a player's athlete on `champion` with pins `row`
// holding `from`, when the stable hook recorded one that differs from it.
pub fn pinned_build(
    champion: &str,
    row: &[Option<String>],
    from: &[String],
) -> Option<Vec<String>> {
    let builds = PINNED_BUILDS.lock().ok()?;
    builds
        .iter()
        .rev()
        .find(|build| build.champion == champion && build.row == row && build.from == from)
        .filter(|build| build.to != build.from)
        .map(|build| build.to.clone())
}

// The pinned item for one slot, exactly as written. The verbatim key tells a
// vanilla tier 5 (whose id is its catalog index) from a mod item found by name.
pub fn pinned_key_raw(champion: &str, slot: usize) -> Option<String> {
    let pins = pins()?;
    let (role, known) = lane_for_champion(champion);
    pin_entry(&pins, champion, role, known)?.get(slot)?.clone()
}

// The pinned item for one slot, normalized as [`resolve_key`] does: `radiant_`
// variant first, then [`alias_key`].
pub fn pinned_key(champion: &str, slot: usize) -> Option<String> {
    let raw = pinned_key_raw(champion, slot)?;
    Some(alias_key(radiant_key(&raw).as_ref()).to_string())
}

// Sets the Smart Builds toggle, for the save that is loaded: in force at
// once, and written to the save on the next client frame ([`sync_settings`]).
pub fn set_smart_builds(enabled: bool) -> bool {
    SMART_BUILDS.store(enabled, Ordering::Relaxed);
    SETTINGS_DIRTY.store(true, Ordering::Relaxed);
    true
}

// Sets `own_team_only` the same way — see [`own_team_only_enabled`] for what
// the two sides of it mean.
pub fn set_own_team_only(enabled: bool) -> bool {
    OWN_TEAM_ONLY.store(enabled, Ordering::Relaxed);
    SETTINGS_DIRTY.store(true, Ordering::Relaxed);
    true
}

// Every client frame: the two toggles follow the loaded save, read once it has
// loaded and written back after a click. Without a save, or toggles in it, both
// are on.
pub fn sync_settings(ctx: &mut mod_api_stable::StableClient<'_>) {
    if !ctx.save_can_write() {
        if SETTINGS_LOADED.swap(false, Ordering::Relaxed) {
            SMART_BUILDS.store(true, Ordering::Relaxed);
            OWN_TEAM_ONLY.store(true, Ordering::Relaxed);
            SETTINGS_DIRTY.store(false, Ordering::Relaxed);
        }
        return;
    }
    if !SETTINGS_LOADED.load(Ordering::Relaxed) {
        // The save's namespace reads empty on some frames while it is still
        // loading. `item_stats` waits that out, and its word is taken here.
        if !crate::item_stats::loaded() {
            return;
        }
        let saved = |key: &str| {
            ctx.save_get_string(key)
                .and_then(|text| text.trim().parse::<bool>().ok())
        };
        let (smart, own) = (saved(SMART_BUILDS_KEY), saved(OWN_TEAM_ONLY_KEY));
        SMART_BUILDS.store(smart.unwrap_or(true), Ordering::Relaxed);
        OWN_TEAM_ONLY.store(own.unwrap_or(true), Ordering::Relaxed);
        SETTINGS_LOADED.store(true, Ordering::Relaxed);
    }
    if SETTINGS_DIRTY.load(Ordering::Relaxed) {
        // This save's own toggles, not what multiplayer has in force.
        let smart = own_smart_builds().to_string();
        let own = OWN_TEAM_ONLY.load(Ordering::Relaxed).to_string();
        let written = ctx.save_set_string(SMART_BUILDS_KEY, &smart)
            & ctx.save_set_string(OWN_TEAM_ONLY_KEY, &own);
        if written {
            SETTINGS_DIRTY.store(false, Ordering::Relaxed);
        }
    }
}

// Whether the Smart Builds pass is on for the loaded save (default on); see
// [`crate::smart_builds`]. A multiplayer guest plays by the host's
// (`crate::multiplayer::host_smart_builds`).
pub fn smart_builds_enabled() -> bool {
    crate::multiplayer::host_smart_builds().unwrap_or_else(own_smart_builds)
}

// This game's own Smart Builds toggle, whatever multiplayer has in force: what
// the host's server sends the other players.
pub fn own_smart_builds() -> bool {
    SMART_BUILDS.load(Ordering::Relaxed)
}

// Whether builds apply only to the player's own team, for the loaded save
// (default on). Off, a build applies to whoever plays the champion, which is
// all the stable hook can express.
//
// On, the stable hook sets no builds and the native side applies pins per
// athlete under the `tactics::is_my_athlete` gate: the spawn-time injector for
// slot 0, the buy detour for the rest. The stable hook cannot gate itself,
// since `ctx.team()` is only a 0/1 side index.
//
// Always on in multiplayer, where each player's builds go to their own team
// (`crate::multiplayer`).
pub fn own_team_only_enabled() -> bool {
    OWN_TEAM_ONLY.load(Ordering::Relaxed) || crate::multiplayer::active()
}

// A configured build merged with the AI's, as item indices, with a record of
// which slots the player pinned. Smart Builds may only rewrite the others.
pub struct MergedBuild {
    pub items: Vec<usize>,
    // Parallel to `items`: `true` where the slot holds the player's pin.
    pub pinned: Vec<bool>,
    // The player's pins for slots past `items` (the 5th and 6th, filled later
    // by the buy detour), which the AI's picks must not duplicate.
    pub reserved: Vec<usize>,
}

// The configured build for one champion as item indices, merged with the
// engine's `ai_build`; `resolve` maps keys into the caller's item list. `None`
// when the champion has no entry (leave the engine's build alone); unknown keys
// are skipped.
pub fn build_for_champion(
    config: &BuildConfig,
    champion: &str,
    role: Role,
    resolve: impl Fn(&str) -> Option<usize>,
    ai_build: &[usize],
) -> Option<MergedBuild> {
    let build = build_entry(config, champion, role)?;
    Some(merge_pin_row(build, resolve, ai_build))
}

// A pin row merged into the engine's build: each pin in its slot, the engine's
// picks in between, and pins past the game's slots as `reserved`.
pub fn merge_pin_row(
    row: &[Option<String>],
    resolve: impl Fn(&str) -> Option<usize>,
    ai_build: &[usize],
) -> MergedBuild {
    // Only the game's slots go to the engine; the 5th and 6th do not exist yet,
    // and the buy detour fills them from the same pins.
    let usable = row.len().min(game_slots());
    let (items, pinned) = merge_build(&row[..usable], ai_build, &resolve);
    let reserved = row[usable..]
        .iter()
        .flatten()
        .filter_map(|key| resolve_key(key, &resolve))
        .collect();
    MergedBuild {
        items,
        pinned,
        reserved,
    }
}

// The build a champion uses in `role`: the role's own, else the [`Role::Any`]
// build.
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

// Resolves a configured key to a pool index: its `radiant_` variant through
// [`alias_key`] first, so plain LoL names work, then verbatim for keys with no
// radiant (like `warlords_final_judgement`). Radiant first matters:
// `liandrys_torment` means the radiant, and verbatim-first would downgrade it.
pub(crate) fn resolve_key<T>(key: &str, resolve: &impl Fn(&str) -> Option<T>) -> Option<T> {
    let radiant = radiant_key(key);
    if let Some(index) = resolve(alias_key(radiant.as_ref())) {
        return Some(index);
    }
    resolve(key)
}

// Builds the final route: pins in their slots, blanks filled in order with the
// AI's unpinned picks. An unresolvable pin counts as blank; the second vector
// marks the pins.
fn merge_build(
    build: &[Option<String>],
    ai_route: &[usize],
    resolve: &impl Fn(&str) -> Option<usize>,
) -> (Vec<usize>, Vec<bool>) {
    let pinned: std::collections::HashSet<usize> = build
        .iter()
        .flatten()
        .filter_map(|key| resolve_key(key, resolve))
        .collect();
    let mut ai_fill = ai_route.iter().copied().filter(|i| !pinned.contains(i));

    let mut route = Vec::with_capacity(build.len());
    let mut is_pin = Vec::with_capacity(build.len());
    for (position, slot) in build.iter().enumerate() {
        if let Some(index) = slot.as_deref().and_then(|key| resolve_key(key, resolve)) {
            route.push(index);
            is_pin.push(true);
            continue;
        }
        // A blank or unresolvable slot still takes its place, or later pins
        // would land early: the AI's next unpinned pick, else the engine's own
        // for this position.
        let Some(index) = ai_fill.next().or_else(|| ai_route.get(position).copied()) else {
            break;
        };
        route.push(index);
        is_pin.push(false);
    }
    (route, is_pin)
}

// A key's `radiant_` variant, so builds can name base items (`collector` ->
// `radiant_collector`).
fn radiant_key(key: &str) -> Cow<'_, str> {
    if key.starts_with("radiant_") {
        Cow::Borrowed(key)
    } else {
        Cow::Owned(format!("radiant_{key}"))
    }
}

// Any spelling of an item back to its plain LoL slug
// (`warlords_final_judgement`, `radiant_bloodthirster` and `bloodthirster` all
// give `bloodthirster`), for grouping with [`crate::item_catalog`].
pub fn base_slug(key: &str) -> &str {
    unalias_key(key)
        .strip_prefix("radiant_")
        .unwrap_or_else(|| unalias_key(key))
}

// Inverse of [`alias_key`]: the game's key for one of its own radiants back to
// the `radiant_` form of its LoL name. Keys with no alias pass through.
fn unalias_key(key: &str) -> &str {
    crate::item_catalog::GAME_RADIANTS
        .iter()
        .find(|&&(_, game, _)| game == key)
        .map_or(key, |&(name, _, _)| name)
}

// A `radiant_` LoL slug to the game's key for its own radiants
// (`radiant_bloodthirster` -> `warlords_final_judgement`); other slugs pass
// through.
fn alias_key(key: &str) -> &str {
    crate::item_catalog::GAME_RADIANTS
        .iter()
        .find(|&&(name, _, _)| name == key)
        .map_or(key, |&(_, game, _)| game)
}

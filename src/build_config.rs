//! Minimal config-driven item-build setter (scaffold).
//!
//! Reads `item-builds.json` from next to the mod DLL and overwrites the routes
//! that the game's `get_item_builds_list` returns. This is the starting point
//! for the `feature/item_build_config` work: it sets fixed build paths, keyed by
//! champion id, and no lineup analysis yet.
//!
//! Extension seams (intentional TODOs):
//! - `apply` keys builds by champion id against the route's lineup (`team1`),
//!   assuming `routes[i]` is the build for `lineup[i]`. Lineup-aware selection —
//!   varying a champion's build by the enemy comp — would slot in there, using
//!   the same `team1`/`team2` data the hook already passes through.

use serde::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;

/// Schema of `item-builds.json`: the whole file is a map of champion id -> build
/// (there is no wrapper object).
///
/// Key = champion id (the champions the hook receives as `team1`), value = an
/// ordered list of build slots. Each slot is
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
/// Returns `Ok(None)` when the file is absent (the common, non-error case so
/// the mod ships inert until a user opts in by creating the file).
pub fn load() -> Result<Option<BuildConfig>, String> {
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

/// Item slots the editor exposes per champion, matching the columns the
/// strategy screen shows: the vanilla three, or four when `tfm2_item_tactics`
/// is installed, enabled and set to four slots.
///
/// A build written with four slots and later opened with that mod disabled
/// keeps its fourth item in `item-builds.json` — [`load_champion_rows`] only
/// pads short builds, never truncates long ones, so turning the companion mod
/// back on restores the build intact — but the editor stops showing that item
/// and [`apply`] stops sending it, because the game has nowhere to put it.
pub fn picker_slots() -> usize {
    crate::companion::item_slots()
}

/// One editable row of the in-game editor: a champion and its three slots.
///
/// The champion is optional because a freshly added row has not been assigned
/// one yet. Such a row is kept in the editor but never written, since a build
/// with nothing to key it by is not a build.
#[derive(Default, Clone, Debug, PartialEq)]
pub struct ChampionRow {
    pub champion: Option<String>,
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
/// do not shuffle between visits, and a file edited by hand keeps the order it
/// was written in.
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
        .map(|(champion, value)| {
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
            // a four-item build read while the fourth slot is unavailable has
            // to survive the round trip so it is still there when it comes
            // back. `apply` is where the unusable tail is dropped.
            if slots.len() < picker_slots() {
                slots.resize(picker_slots(), None);
            }
            ChampionRow {
                champion: Some(champion),
                slots,
            }
        })
        .collect()
}

/// Writes the complete rows back to `item-builds.json`, in row order.
///
/// Incomplete rows are skipped and a later row with the same champion wins, so
/// the file only ever holds builds that mean something. Returns false when it
/// could not be written.
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
        map.insert(champion.clone(), serde_json::Value::Array(slots));
    }

    serde_json::to_string_pretty(&map)
        .ok()
        .and_then(|text| std::fs::write(path, text + "\n").ok())
        .is_some()
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

/// Records the roster from inside the detour. Cheap enough to call per match;
/// the roster cannot change without a restart.
pub fn record_champion_roster(ids: &[String]) {
    if let Ok(mut roster) = CHAMPION_ROSTER.lock() {
        roster.clear();
        roster.extend_from_slice(ids);
    }
}

/// The recorded roster. Empty until the first match simulates, which happens
/// before the player can reach their own strategy screen.
pub fn champion_roster() -> Vec<String> {
    CHAMPION_ROSTER.lock().map(|roster| roster.clone()).unwrap_or_default()
}

/// Writes `unique_items` to `mod-settings.json`, the toggle
/// [`unique_items_enabled`] reads back on every hook call.
///
/// The whole file is rewritten from this one field: the schema has exactly one
/// key, so there is nothing else in it to preserve.
pub fn set_unique_items(enabled: bool) -> bool {
    let path = crate::config::mod_dir().join("mod-settings.json");
    std::fs::write(path, format!("{{\n  \"unique_items\": {enabled}\n}}\n")).is_ok()
}

/// Schema of `mod-settings.json`: behavior toggles managed by the item build
/// editor. An absent file (the common case) means every toggle takes its
/// default.
#[derive(Deserialize)]
struct ModSettings {
    #[serde(default = "default_true")]
    unique_items: bool,
}

fn default_true() -> bool {
    true
}

/// Whether unique-build enforcement is enabled: `unique_items` in
/// `mod-settings.json` next to the mod DLL. Defaults to enforced when the file
/// is absent or malformed, so players opt *out* via the editor toggle. Read on
/// every hook call, so flipping it applies to the next match without restarting
/// the game.
pub fn unique_items_enabled() -> bool {
    let Some(path) = crate::config::dll_dir().map(|dir| dir.join("mod-settings.json")) else {
        return true;
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<ModSettings>(&text).ok())
        .map(|settings| settings.unique_items)
        .unwrap_or(true)
}

/// Overwrites the game's route list with the configured builds.
///
/// `item_keys` is the parallel list of item keys for the `items` slice the game
/// passed to the hook; each build's item keys are resolved to indices into it.
/// `lineup` is the champion id for each route in route order — the team the game
/// generates these builds for (`team1`), so `routes[i]` is the build for
/// `lineup[i]`. This is the lineup, *not* the game's 60-entry `champion_ids`
/// roster: `route_count` tracks `team1` size, far smaller than
/// `champion_id_count`.
///
/// Each route slot whose `lineup` champion has a `by_champion` entry is
/// overwritten with that build; every other slot is left exactly as the game
/// generated it. Unknown item keys are skipped rather than aborting, so one typo
/// does not discard the rest of a build. The route count is never changed.
pub fn apply(
    config: &BuildConfig,
    item_keys: &[String],
    lineup: &[String],
    routes: &mut [Vec<usize>],
) {
    let index_by_key: HashMap<&str, usize> = item_keys
        .iter()
        .enumerate()
        .map(|(index, key)| (key.as_str(), index))
        .collect();

    for (index, slot) in routes.iter_mut().enumerate() {
        let champion_build = lineup
            .get(index)
            .and_then(|champion_id| config.by_champion.get(champion_id));
        if let Some(build) = champion_build {
            // A build may be longer than the game has slots for — the file
            // keeps a fourth item while `tfm2_item_tactics` is off, so that
            // turning it back on restores the build. Sending that item anyway
            // would hand the game a route it has nowhere to put.
            let usable = build.len().min(picker_slots());
            let ai_route = slot.clone();
            *slot = merge_build(&build[..usable], &ai_route, &index_by_key);
        }
    }
}

/// Resolves one configured item key to a pool index. The key is tried verbatim
/// first, so a game-internal key (`"warlords_final_judgement"`, or any vanilla
/// tier 5) resolves as written; only if that misses is it normalized to its
/// `radiant_` variant and run through `alias_key`, which is what lets builds be
/// authored with plain LoL names (`"collector"`). Unknown keys return `None`
/// (skipped rather than aborting the build).
///
/// The `radiant_` attempt comes FIRST and the verbatim one is the fallback.
/// Order matters: `"liandrys_torment"` in an existing `item-builds.json` means
/// the radiant item, but a base item of that exact key also exists, so trying
/// verbatim first would silently downgrade every build in that file to its base
/// tier. The fallback exists only for keys with no radiant variant — the vanilla
/// tier 5s the in-game picker offers, like `"warlords_final_judgement"`.
fn resolve_key(key: &str, index_by_key: &HashMap<&str, usize>) -> Option<usize> {
    let radiant = radiant_key(key);
    if let Some(index) = index_by_key.get(alias_key(radiant.as_ref())) {
        return Some(*index);
    }
    index_by_key.get(key).copied()
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
    index_by_key: &HashMap<&str, usize>,
) -> Vec<usize> {
    let pinned: std::collections::HashSet<usize> = build
        .iter()
        .flatten()
        .filter_map(|key| resolve_key(key, index_by_key))
        .collect();
    let mut ai_fill = ai_route.iter().copied().filter(|i| !pinned.contains(i));

    let mut route = Vec::with_capacity(build.len());
    for slot in build {
        match slot {
            Some(key) => {
                if let Some(index) = resolve_key(key, index_by_key) {
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

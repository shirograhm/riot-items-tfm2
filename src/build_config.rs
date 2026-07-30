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

/// Number of route slots the in-game picker exposes, matching the three item
/// columns the strategy screen already shows per player.
pub const PICKER_SLOTS: usize = 3;

/// Number of rows on the strategy screen's personal tab — one per position, in
/// position order (Top, Jungle, Mid, Bottom, Support).
pub const PICKER_ROWS: usize = 5;

/// Builds chosen from the strategy screen, keyed by route index as a decimal
/// string (`"0"`..`"4"`).
///
/// Kept in `item-builds-strategy.json`, separate from `item-builds.json`, for
/// two reasons: the two are keyed differently (position vs. champion), and the
/// external item build editor owns the champion file — writing both from two
/// places would make them fight over it.
///
/// Route index is the position order the hook already relies on (Top = 0), which
/// is exactly the strategy screen's row order, so a row index needs no mapping.
/// Values follow the same slot convention as [`BuildConfig`]: an item key pins
/// the slot, `null` leaves it to the AI.
pub type PositionBuilds = HashMap<String, Vec<Option<String>>>;

fn position_config_path() -> PathBuf {
    crate::config::mod_dir().join("item-builds-strategy.json")
}

/// Loads `item-builds-strategy.json`. An absent or malformed file yields an
/// empty map: the picker is additive, so a bad file must never cost the player
/// the routes the game (or `item-builds.json`) already produced.
pub fn load_position_builds() -> PositionBuilds {
    std::fs::read_to_string(position_config_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Writes one slot of one row's build and persists the whole file.
///
/// `item_key` of `None` clears the slot back to AI choice. Rows are padded to
/// [`PICKER_SLOTS`] so a write to slot 2 does not depend on slots 0 and 1 having
/// been set first. Returns false when the file could not be written.
pub fn set_position_slot(row: usize, slot: usize, item_key: Option<&str>) -> bool {
    if row >= PICKER_ROWS || slot >= PICKER_SLOTS {
        return false;
    }
    let path = position_config_path();

    let mut builds = load_position_builds();
    let build = builds.entry(row.to_string()).or_default();
    build.resize(PICKER_SLOTS, None);
    build[slot] = item_key.map(str::to_string);

    // A row that is back to all-AI is dropped rather than stored as a row of
    // nulls, so the file stays a record of actual player choices.
    if builds
        .get(&row.to_string())
        .is_some_and(|build| build.iter().all(Option::is_none))
    {
        builds.remove(&row.to_string());
    }

    serde_json::to_string_pretty(&builds)
        .ok()
        .and_then(|text| std::fs::write(path, text).ok())
        .is_some()
}

/// Exchanges two slots of one row's build, for the editor's swap buttons.
///
/// Done as one read-modify-write rather than two [`set_position_slot`] calls
/// because a swap through an intermediate state can leave the row all-`None`,
/// which that function deletes — losing the build it was asked to reorder.
pub fn swap_position_slots(row: usize, a: usize, b: usize) -> bool {
    if row >= PICKER_ROWS || a >= PICKER_SLOTS || b >= PICKER_SLOTS || a == b {
        return false;
    }

    let mut builds = load_position_builds();
    let Some(build) = builds.get_mut(&row.to_string()) else {
        return true; // an all-AI row has nothing to reorder
    };
    build.resize(PICKER_SLOTS, None);
    build.swap(a, b);

    serde_json::to_string_pretty(&builds)
        .ok()
        .and_then(|text| std::fs::write(position_config_path(), text).ok())
        .is_some()
}

/// Writes `unique_items` to `mod-settings.json`, the toggle
/// [`unique_items_enabled`] reads back on every hook call.
///
/// The whole file is rewritten from this one field, matching the desktop
/// editor's `Save-Settings`: the schema has exactly one key, so there is
/// nothing else in it to preserve.
pub fn set_unique_items(enabled: bool) -> bool {
    let path = crate::config::mod_dir().join("mod-settings.json");
    std::fs::write(path, format!("{{\n  \"unique_items\": {enabled}\n}}\n")).is_ok()
}

/// Applies the strategy screen's per-row builds on top of `routes`.
///
/// Runs after [`apply`] so an explicit in-match pick from the strategy screen
/// wins over the champion-keyed file, and shares its merge semantics: pinned
/// slots take the player's item, `null` slots fall back to the AI's own picks.
/// Rows with no entry are left exactly as they were.
pub fn apply_positions(builds: &PositionBuilds, item_keys: &[String], routes: &mut [Vec<usize>]) {
    if builds.is_empty() {
        return;
    }
    let index_by_key: HashMap<&str, usize> = item_keys
        .iter()
        .enumerate()
        .map(|(index, key)| (key.as_str(), index))
        .collect();

    for (index, slot) in routes.iter_mut().enumerate() {
        if let Some(build) = builds.get(&index.to_string()) {
            let ai_route = slot.clone();
            *slot = merge_build(build, &ai_route, &index_by_key);
        }
    }
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
/// is absent or malformed, so players opt *out* via the editor checkbox. Read on
/// every hook call, so toggling the checkbox takes effect on the next match
/// without restarting the game.
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
            let ai_route = slot.clone();
            *slot = merge_build(build, &ai_route, &index_by_key);
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

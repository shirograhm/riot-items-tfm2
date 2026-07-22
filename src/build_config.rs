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

/// Resolves one configured item key to a pool index: normalize to its `radiant_`
/// variant, map any renamed item through `alias_key`, then look it up. Unknown
/// keys return `None` (skipped rather than aborting the build).
fn resolve_key(key: &str, index_by_key: &HashMap<&str, usize>) -> Option<usize> {
    let radiant = radiant_key(key);
    index_by_key.get(alias_key(radiant.as_ref())).copied()
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

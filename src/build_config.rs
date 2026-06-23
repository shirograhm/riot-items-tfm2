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
//!   the same `team1`/`team2` data the `item_build_probe` log already captures.

use serde::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;

/// Schema of `item-builds.json`: the whole file is a map of champion id -> build
/// (there is no wrapper object).
///
/// Key = champion id (e.g. the champions logged as `team1=[..]` in the
/// `item_build_probe` line), value = an ordered list of item keys forming that
/// champion's build. Each item key is resolved to its `radiant_` (tier 5) variant
/// (`radiant_key`), then to the game's internal key for renamed items
/// (`alias_key`). A route whose champion has no entry here is left as the game
/// generated it.
#[derive(Deserialize, Default)]
#[serde(transparent)]
pub struct BuildConfig {
    pub by_champion: HashMap<String, Vec<String>>,
}

impl BuildConfig {
    pub fn is_empty(&self) -> bool {
        self.by_champion.is_empty()
    }
}

/// Outcome of applying a config to a route list, for logging.
pub struct ApplyReport {
    /// Total number of route slots overwritten (each one matched a `by_champion`
    /// entry; routes with no match are left untouched).
    pub routes_set: usize,
    /// Resolved item keys (after `radiant_` normalization) that were not present
    /// in the game's item list (deduplicated). A non-empty list usually means a
    /// typo or an item this mod/version does not register.
    pub unknown_keys: Vec<String>,
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

/// Overwrites the game's route list with the configured builds.
///
/// `item_keys` is the parallel list of item keys for the `items` slice the game
/// passed to the hook; each build's item keys are resolved to indices into it.
/// `lineup` is the champion id for each route in route order — the team the game
/// generates these builds for (`team1`), so `routes[i]` is the build for
/// `lineup[i]`. This is the lineup, *not* the game's 60-entry `champion_ids`
/// roster: the `item_build_probe` log shows `route_count` tracks `team1` size,
/// far smaller than `champion_id_count`.
///
/// Each route slot whose `lineup` champion has a `by_champion` entry is
/// overwritten with that build; every other slot is left exactly as the game
/// generated it. Unknown item keys are skipped and reported rather than aborting,
/// so one typo does not discard the rest of a build. The route count is never
/// changed.
pub fn apply(
    config: &BuildConfig,
    item_keys: &[String],
    lineup: &[String],
    routes: &mut [Vec<usize>],
) -> ApplyReport {
    let index_by_key: HashMap<&str, usize> = item_keys
        .iter()
        .enumerate()
        .map(|(index, key)| (key.as_str(), index))
        .collect();

    let mut unknown = Vec::new();
    let mut routes_set = 0;

    for (index, slot) in routes.iter_mut().enumerate() {
        let champion_build = lineup
            .get(index)
            .and_then(|champion_id| config.by_champion.get(champion_id));
        if let Some(build) = champion_build {
            *slot = resolve_build(build, &index_by_key, &mut unknown);
            routes_set += 1;
        }
    }

    unknown.sort();
    unknown.dedup();
    ApplyReport {
        routes_set,
        unknown_keys: unknown,
    }
}

fn resolve_build(
    build: &[String],
    index_by_key: &HashMap<&str, usize>,
    unknown: &mut Vec<String>,
) -> Vec<usize> {
    build
        .iter()
        .filter_map(|key| {
            let radiant = radiant_key(key);
            let resolved = alias_key(radiant.as_ref());
            match index_by_key.get(resolved) {
                Some(&index) => Some(index),
                None => {
                    unknown.push(resolved.to_string());
                    None
                }
            }
        })
        .collect()
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
        // Radiant Warmog's Armor
        "radiant_warmogs_armor" => "giants_horn_shard",
        other => other,
    }
}

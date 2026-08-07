use mod_api_stable::{StableDraftDecision, StableItemBuildContext, StableItemBuildHook};

use crate::build_config;

const MOD_ITEM_SCORE_BONUS: f32 = 0.5;

// ---------------------------------------------------------------------------
//  Attribution diagnostic (temporary)
// ---------------------------------------------------------------------------
//
// Answers one question: does the host call this hook for BOTH teams, only the
// player's, or never? Nothing else in the mod can tell us — `ctx.team()` is the
// only first-hand statement of which side a build belongs to, and it exists
// only inside this call.
//
// Read `item_build_hook_diag.txt` next to the DLL after a match:
//   * no file at all      -> the hook is never dispatched; the export wiring or
//                            the host is the problem, not the team gate
//   * only `team=N` lines
//     for one value of N  -> the host team-gates the hook (the hypothesis)
//   * team=0 and team=1   -> the hook sees both sides; a build that still does
//                            not land is failing later, in `configured_build`
//                            (look at `applied=`) or in the engine
//
// `applied=` distinguishes "the hook ran for this champion" from "the hook
// changed anything": `no_entry` means the champion is not in item-builds.json,
// `same_as_base` means the configured build equalled the engine's own pick.
//
// IO discipline: this runs on parallel sim workers, once per player per match
// INCLUDING background league fixtures, so it must never touch the disk here —
// that is what made the old per-call file writes a runaway crash risk. Lines
// accumulate into a bounded, deduplicated buffer and are flushed from
// `flush_diag` on the management tick, which is the main thread.
const DECIDE_DIAG: bool = true;

const DIAG_MAX_ENTRIES: usize = 400;

static DIAG_SEEN: std::sync::Mutex<Option<std::collections::HashSet<String>>> =
    std::sync::Mutex::new(None);
static DIAG_BUF: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
static DIAG_DIRTY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn diag_record(ctx: &StableItemBuildContext<'_>, applied: &str) {
    if !DECIDE_DIAG {
        return;
    }
    // Deduplicated per (team, champion, outcome): the same fixture simulating
    // every league day would otherwise write the same line thousands of times
    // and tell us nothing new after the first.
    let key = format!("{}#{}#{}", ctx.team(), ctx.champion_key(), applied);
    {
        let mut seen = DIAG_SEEN.lock().unwrap_or_else(|e| e.into_inner());
        let set = seen.get_or_insert_with(std::collections::HashSet::new);
        if set.len() >= DIAG_MAX_ENTRIES || !set.insert(key) {
            return;
        }
    }
    let line = format!(
        "team={} lane={:?} champion={} base_len={} applied={}\n",
        ctx.team(),
        ctx.lane(),
        ctx.champion_key(),
        ctx.base_build().len(),
        applied,
    );
    if let Ok(mut buf) = DIAG_BUF.lock() {
        buf.push_str(&line);
    }
    DIAG_DIRTY.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Writes the accumulated diagnostic to `item_build_hook_diag.txt`. Called from
/// the management tick (main thread) — never from `decide_build` itself.
pub fn flush_diag() {
    use std::sync::atomic::Ordering;
    if !DECIDE_DIAG || !DIAG_DIRTY.swap(false, Ordering::Relaxed) {
        return;
    }
    let Ok(buf) = DIAG_BUF.lock() else {
        return;
    };
    let path = crate::config::mod_dir().join("item_build_hook_diag.txt");
    let _ = std::fs::write(path, buf.as_str());
}

pub struct ConfiguredBuilds;

impl StableItemBuildHook for ConfiguredBuilds {
    fn id(&self) -> String {
        "riot_items_tfm2.configured_builds".to_string()
    }

    fn priority(&self) -> i32 {
        100
    }

    fn score_item(
        &self,
        ctx: &StableItemBuildContext<'_>,
        candidate: usize,
        base_score: f32,
    ) -> StableDraftDecision {
        // Already wanted, or not ours: leave the engine's ranking alone.
        if base_score > 0.0 {
            return StableDraftDecision::Pass;
        }
        let Some(key) = ctx.item_key(candidate) else {
            return StableDraftDecision::Pass;
        };
        if !crate::strategy_ui::is_mod_final_item(key) {
            return StableDraftDecision::Pass;
        }
        StableDraftDecision::Add(MOD_ITEM_SCORE_BONUS)
    }

    fn decide_build(&self, ctx: &StableItemBuildContext<'_>) -> Vec<usize> {
        let base = ctx.base_build();
        let configured = self.configured_build(ctx);
        let had_entry = configured.is_some();
        let mut build = configured.unwrap_or_else(|| base.to_vec());

        if build_config::unique_items_enabled() {
            enforce_unique_items(ctx, &mut build);
        }

        if build.is_empty() || build == base {
            // Both are no-ops to the engine but mean very different things to
            // us: `no_entry` is this champion having no configured build at
            // all, `same_as_base` is a build that resolved to what the engine
            // already wanted.
            diag_record(ctx, if had_entry { "same_as_base" } else { "no_entry" });
            return Vec::new();
        }
        diag_record(ctx, "override");
        build
    }
}

impl ConfiguredBuilds {
    /// No team gate: a build is keyed by champion and applies to whoever plays
    /// it, the enemy included. The tactics half still pins the same items per
    /// athlete for the player's own side, which is now redundant rather than
    /// conflicting — both read the same `item-builds.json`, so they agree.
    fn configured_build(&self, ctx: &StableItemBuildContext<'_>) -> Option<Vec<usize>> {
        let config = build_config::load_cached();
        if config.is_empty() {
            return None;
        }

        build_config::build_for_champion(
            &config,
            ctx.champion_key(),
            |key| ctx.item_index(key),
            ctx.base_build(),
        )
    }
}

const SELECTABLE_FINAL_TIER: usize = 4;

fn is_selectable_final(ctx: &StableItemBuildContext<'_>, index: usize) -> bool {
    ctx.item_tier(index)
        .is_some_and(|tier| tier >= SELECTABLE_FINAL_TIER)
}

fn enforce_unique_items(ctx: &StableItemBuildContext<'_>, build: &mut [usize]) {
    let count = ctx.item_count();
    if count == 0 {
        return;
    }
    let mut seen = std::collections::HashSet::new();
    for slot in build.iter_mut() {
        if seen.insert(*slot) {
            continue;
        }
        // Must be known: matching `None` against `None` would swap a duplicate
        // for any item the host could not classify.
        let Some(category) = ctx.item_category(*slot) else {
            continue;
        };
        let duplicate = *slot;
        let replacement = (1..count).map(|step| (duplicate + step) % count).find(|c| {
            !seen.contains(c)
                && ctx.item_category(*c) == Some(category)
                && is_selectable_final(ctx, *c)
        });
        if let Some(index) = replacement {
            *slot = index;
            seen.insert(index);
        }
    }
}

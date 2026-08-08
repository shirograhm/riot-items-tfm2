use mod_api_stable::{StableDraftDecision, StableItemBuildContext, StableItemBuildHook};

use crate::build_config;

const MOD_ITEM_SCORE_BONUS: f32 = 0.5;

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
        // `own_team_only` hands the configured builds to the native buy detour,
        // which is the only half of the mod that can tell the player's athletes
        // from the enemy's. This context names the champion and the two lineups
        // but never says which side it belongs to, so applying a build here
        // would apply it to both — exactly what the toggle is off for. Unique
        // enforcement below still runs: it is about the shape of a build, not
        // about whose it is, and it applies to the engine's own picks too.
        let mut build = if build_config::own_team_only_enabled() {
            base.to_vec()
        } else {
            self.configured_build(ctx).unwrap_or_else(|| base.to_vec())
        };

        if build_config::unique_items_enabled() {
            enforce_unique_items(ctx, &mut build);
        }

        if build.is_empty() || build == base {
            return Vec::new();
        }
        build
    }
}

impl ConfiguredBuilds {
    // No team gate, and there is none to write: a build is keyed by champion and
    // applies to whoever plays it, enemy included. A player who does not want
    // that turns on `own_team_only`, which stops `decide_build` calling this at
    // all — see there.
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

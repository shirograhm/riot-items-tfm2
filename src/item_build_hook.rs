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
        let mut build = self.configured_build(ctx).unwrap_or_else(|| base.to_vec());

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
    // No team gate: a build is keyed by champion and applies to whoever plays it, enemy included.
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

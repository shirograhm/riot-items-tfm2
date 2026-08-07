use mod_api_stable::{StableItemBuildContext, StableItemBuildHook};

use crate::build_config;

pub struct ConfiguredBuilds;

impl StableItemBuildHook for ConfiguredBuilds {
    fn id(&self) -> String {
        "riot_items_tfm2.configured_builds".to_string()
    }

    fn priority(&self) -> i32 {
        100
    }

    fn decide_build(&self, ctx: &StableItemBuildContext<'_>) -> Vec<usize> {
        let allies = ctx.ally_champions();
        let enemies = ctx.enemy_champions();

        crate::my_team::note_lineups(&allies, &enemies);

        let base = ctx.base_build();
        let mut build = self
            .configured_build(ctx, &allies, &enemies)
            .unwrap_or_else(|| base.to_vec());

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
    fn configured_build(
        &self,
        ctx: &StableItemBuildContext<'_>,
        allies: &[&str],
        enemies: &[&str],
    ) -> Option<Vec<usize>> {
        if crate::tactics::driver::injects_builds() {
            return None;
        }
        if !crate::my_team::owns_lineup(allies, enemies) {
            return None;
        }

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

fn enforce_unique_items(ctx: &StableItemBuildContext<'_>, build: &mut [usize]) {
    let mut seen = std::collections::HashSet::new();
    for slot in build.iter_mut() {
        if seen.insert(*slot) {
            continue;
        }

        let (Some(category), Some(tier)) = (ctx.item_category(*slot), ctx.item_tier(*slot)) else {
            continue;
        };
        let replacement = (0..ctx.item_count()).find(|candidate| {
            !seen.contains(candidate)
                && ctx.item_category(*candidate) == Some(category)
                && ctx.item_tier(*candidate) == Some(tier)
        });
        if let Some(index) = replacement {
            *slot = index;
            seen.insert(index);
        }
    }
}

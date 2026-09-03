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
            self.configured_build(ctx)
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
    fn configured_build(&self, ctx: &StableItemBuildContext<'_>) -> Vec<usize> {
        let config = build_config::load_cached();
        if config.is_empty() {
            return ctx.base_build().to_vec();
        }

        let champion = ctx.champion_key();
        // The lane is the half the buy detour has to infer; here the host
        // states it outright, so a role build is picked without guessing.
        //
        // Unless it does not. A host that leaves `lane` at a code `LaneV1` does
        // not map yields `None`, and taking that as `Role::Any` makes every
        // `champion@role` line in the file dead — the whole configured build
        // silently becomes the engine's. The route detour's position-ordered
        // `team1` is the mod's other source for the same fact, so fall back to
        // it rather than to a role that matches nothing. It is best effort (see
        // `build_config::role_for_champion`), which still beats certain failure.
        let lane = ctx.lane().map(|lane| lane.code() as usize);
        let role = match lane {
            Some(code) => build_config::Role::from_lane_code(code),
            None => build_config::role_for_champion(champion),
        };

        match build_config::build_for_champion(
            &config,
            champion,
            role,
            |key| ctx.item_index(key),
            ctx.base_build(),
        ) {
            Ok(build) => build,
            Err(miss) => {
                build_config::record_hook_miss(champion, role, describe_miss(ctx, lane, &miss));
                ctx.base_build().to_vec()
            }
        }
    }
}

/// One human-readable block explaining a [`build_config::BuildMiss`], for the
/// report `build_config::take_hook_miss_report` writes.
///
/// Built only on the failing path, so the formatting never costs anything in a
/// match whose builds all apply.
fn describe_miss(
    ctx: &StableItemBuildContext<'_>,
    lane: Option<usize>,
    miss: &build_config::BuildMiss,
) -> String {
    let lane = match lane {
        Some(code) => format!("host lane code {code}"),
        None => "host named NO lane (fell back to the lineup map)".to_string(),
    };
    match miss {
        build_config::BuildMiss::NoEntry { tried, available } => format!(
            "  cause : no entry for this champion+role\n  \
             lane  : {lane}\n  \
             wanted: {tried}\n  \
             file  : {}\n",
            if available.is_empty() {
                "(nothing for this champion - is the champion id right?)".to_string()
            } else {
                available.join(", ")
            }
        ),
        build_config::BuildMiss::Unresolved { key, pins } => {
            let mut out = format!(
                "  cause : entry found, but no pinned item is in the engine's pool\n  \
                 lane  : {lane}\n  \
                 entry : {key}\n  \
                 pool  : {} selectable items\n",
                ctx.item_count()
            );
            for (pin, hit) in pins {
                match hit {
                    Some(index) => out.push_str(&format!("  pin   : {pin} -> index {index}\n")),
                    None => out.push_str(&format!("  pin   : {pin} -> NOT IN POOL\n")),
                }
            }
            // A handful of pool keys sharing a word with a failed pin. This is
            // what tells a key that is absent from one that is merely spelled
            // differently than the file has it.
            let stem = pins
                .iter()
                .find(|(_, hit)| hit.is_none())
                .map(|(pin, _)| build_config::base_slug(pin).to_string())
                .unwrap_or_default();
            let needle = stem.split('_').next().unwrap_or("");
            if !needle.is_empty() {
                let near: Vec<&str> = ctx
                    .item_keys()
                    .into_iter()
                    .filter(|key| key.contains(needle))
                    .take(6)
                    .collect();
                out.push_str(&format!(
                    "  near  : {}\n",
                    if near.is_empty() {
                        format!("no pool key contains \"{needle}\"")
                    } else {
                        near.join(", ")
                    }
                ));
            }
            out
        }
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

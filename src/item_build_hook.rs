use mod_api_stable::{StableDraftDecision, StableItemBuildContext, StableItemBuildHook};

use crate::{build_config, smart_builds};

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
        // Boots are listed with the finals for the editor's picker, but they
        // reach a build through Smart Builds' boots rule, never the ranking.
        if !crate::strategy_ui::is_mod_final_item(key) || smart_builds::is_boots(key) {
            return StableDraftDecision::Pass;
        }
        // The bonus exists because the engine's scoring model does not know
        // the mod's items, not because every one suits every champion: pushing
        // Death's Dance on an Ice Mage as hard as on a Swordsman is how mages
        // ended up building it. An item the champion could not keep under the
        // Smart Builds rules gets no push, toggle or not — declining to promote
        // an item is not overriding a pick.
        if smart_builds::Budget::empty(champion_fit(ctx)).rejects(key).is_some() {
            return StableDraftDecision::Pass;
        }
        StableDraftDecision::Add(MOD_ITEM_SCORE_BONUS)
    }

    fn decide_build(&self, ctx: &StableItemBuildContext<'_>) -> Vec<usize> {
        let base = ctx.base_build();
        // `own_team_only` hands the configured builds to the native buy detour,
        // which is the only half of the mod that can tell the player's athletes
        // from the enemy's.
        //
        // This used to say the context "never says which side it belongs to",
        // which was wrong in letter — `ctx.team()` exists — but right in
        // substance, and a logged match (2026-09-08) settled why. `team` is a
        // **0/1 side index within the match**, not a team id: 40 decisions
        // across 4 fixtures came back as exactly five `team=0` lines then five
        // `team=1` lines per match, uniform per lineup. It says which of the two
        // lineups a build belongs to and nothing else.
        //
        // That is not enough to gate on, for two separate reasons. It cannot
        // say *which* side is the player's — that alternates per match — and it
        // cannot say whether the player is in this match at all: every one of
        // those 40 decisions was a background league fixture between two AI
        // teams, where `team=0` means only "the first lineup". Applying a build
        // here would still reach both sides, which is exactly what the toggle
        // is off for.
        //
        // The Smart Builds pass below still runs: it is about the shape of a
        // build, not about whose it is, and it applies to the engine's own
        // picks too.
        let own_team_only = build_config::own_team_only_enabled();
        let configured = if own_team_only {
            None
        } else {
            self.configured_build(ctx)
        };
        // With no configured build every slot is the engine's, so none is pinned.
        let merged = configured.unwrap_or_else(|| build_config::MergedBuild {
            items: base.to_vec(),
            pinned: Vec::new(),
            reserved: Vec::new(),
        });
        let mut build = merged.items;

        if build_config::smart_builds_enabled() {
            let (boots_avoid, pinned_boots) = if own_team_only {
                pending_pins(ctx)
            } else {
                (Vec::new(), None)
            };
            enforce_smart_build(
                ctx,
                &mut build,
                &merged.pinned,
                &merged.reserved,
                &boots_avoid,
                pinned_boots,
            );
        }

        if build.is_empty() || build == base {
            Vec::new()
        } else {
            build
        }
    }
}

impl ConfiguredBuilds {
    // No team gate, and none that can be written from this context alone: a
    // build is keyed by champion and applies to whoever plays it, enemy
    // included. A player who does not want that turns on `own_team_only`, which
    // stops `decide_build` calling this at all — see there for what `ctx.team()`
    // turned out to be and why it does not close the gap.
    fn configured_build(
        &self,
        ctx: &StableItemBuildContext<'_>,
    ) -> Option<build_config::MergedBuild> {
        let config = build_config::load_cached();
        if config.is_empty() {
            return None;
        }

        // The lane is the half the buy detour has to infer; here the host
        // states it outright, so a role build is picked without guessing.
        let role = ctx
            .lane()
            .map(|lane| build_config::Role::from_lane_code(lane.code() as usize))
            .unwrap_or(build_config::Role::Any);

        build_config::build_for_champion(
            &config,
            ctx.champion_key(),
            role,
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

/// The lane the host states, as a build role.
fn champion_role(ctx: &StableItemBuildContext<'_>) -> build_config::Role {
    ctx.lane()
        .map(|lane| build_config::Role::from_lane_code(lane.code() as usize))
        .unwrap_or(build_config::Role::Any)
}

/// The Smart Builds [`smart_builds::Fit`] of the champion this build is for, in
/// the lane the host states.
fn champion_fit(ctx: &StableItemBuildContext<'_>) -> smart_builds::Fit {
    smart_builds::fit(ctx.champion_key(), champion_role(ctx))
}

/// What Smart Builds' boots rule must know about the pins under
/// `own_team_only`, where the detours paste them over this build later: the
/// game slots the champion's row pins in this lane (the boots stay out of
/// them, or the engine can finish the pair before the buy detour writes the
/// pin), and the pair the player pinned, if any.
///
/// The pinned pair goes in as the rule's choice so that the one pair the
/// engine plans is the player's. Any other pair clashes with the pin, and when
/// the spawn injector misses the athlete the buy path settles that clash by
/// keeping the engine's pair: the pin lands too late to replace it.
fn pending_pins(ctx: &StableItemBuildContext<'_>) -> (Vec<bool>, Option<usize>) {
    // Publishes the pin snapshot `pin_row` reads.
    build_config::load_cached();
    let row = build_config::pin_row(ctx.champion_key(), champion_role(ctx));
    let avoid = row
        .iter()
        .take(build_config::game_slots())
        .map(Option::is_some)
        .collect();
    let pinned_boots = row.iter().flatten().find_map(|key| {
        build_config::resolve_key(key, &|key: &str| ctx.item_index(key))
            .filter(|&index| ctx.item_key(index).is_some_and(smart_builds::is_boots))
    });
    (avoid, pinned_boots)
}

/// The Smart Builds pass over a build the host handed us, with the catalog seen
/// through `StableItemBuildContext`. The rules themselves live in
/// [`crate::smart_builds`], which the training-screen detour in `crate::hook`
/// drives over the same build with its own accessors. `pinned_boots` replaces
/// the boots rule's own choice (see [`pending_pins`]).
fn enforce_smart_build(
    ctx: &StableItemBuildContext<'_>,
    build: &mut [usize],
    pinned: &[bool],
    reserved: &[usize],
    boots_avoid: &[bool],
    pinned_boots: Option<usize>,
) {
    // This is the one path that sees the enemy lineup, which is what picks a
    // tank's boots.
    let boots = pinned_boots.or_else(|| {
        let enemies = ctx.enemy_champions();
        ctx.item_index(smart_builds::boots_for(ctx.champion_key(), champion_role(ctx), &enemies))
    });
    smart_builds::enforce(
        ctx.item_count(),
        build,
        pinned,
        reserved,
        boots_avoid,
        champion_fit(ctx),
        boots,
        |index| ctx.item_key(index).map(str::to_string),
        |index| ctx.item_category(index),
        |index| is_selectable_final(ctx, index),
    );
}

use mod_api_stable::{StableDraftDecision, StableItemBuildContext, StableItemBuildHook};

use crate::{build_config, smart_builds};

const MOD_ITEM_SCORE_BONUS: f32 = 0.5;

/// What a support item gets in place of [`MOD_ITEM_SCORE_BONUS`] from a
/// support that prefers them (Smart Builds rule 9): enough to rank it over the
/// mod's other items, not over what the engine itself wants for the
/// champion's class. Double the plain bonus, a guess (2026-10-04) on a score
/// scale nothing documents.
const SUPPORT_ITEM_SCORE_BONUS: f32 = 1.0;

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
        let _probe = crate::perf::Probe::start(crate::perf::Section::ScoreItem);
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
        let fit = champion_fit(ctx);
        if smart_builds::Budget::empty(fit).rejects(key).is_some() {
            return StableDraftDecision::Pass;
        }
        // A support's World Atlas item is its one dedicated support item, and
        // past that it prefers support items without being held to one.
        if fit.is_preferred_support_item(key) {
            return StableDraftDecision::Add(SUPPORT_ITEM_SCORE_BONUS);
        }
        StableDraftDecision::Add(MOD_ITEM_SCORE_BONUS)
    }

    fn decide_build(&self, ctx: &StableItemBuildContext<'_>) -> Vec<usize> {
        let _probe = crate::perf::Probe::start(crate::perf::Section::StableBuildHook);
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
        // What the player's athlete on this champion holds, where that is not
        // `build`: the pin-aware twin `own_team_only` keeps back from the enemy.
        let mut own = None;

        if build_config::smart_builds_enabled() {
            // Under `own_team_only` this build reaches both teams, so it must
            // not know the pins: an enemy on a pinned champion would lose the
            // boots slots, the pinned items and gain the player's pair. The
            // pin-aware build goes to the spawn injector instead, which gives
            // it to the player's athletes only. See `remember_pinned_build`.
            // (Nor, then, whether a pin holds the 5th or 6th slot.)
            let later_open = !own_team_only
                && build_config::later_slot_open(&build_config::pin_row(
                    ctx.champion_key(),
                    champion_role(ctx),
                ));
            enforce_smart_build(ctx, &mut build, &merged.pinned, &merged.reserved, later_open);
            if own_team_only {
                own = remember_pinned_build(ctx, &build);
            }
        } else {
            // The one rule the toggle does not switch off.
            keep_jungle_items_in_jungle(ctx, &mut build, &merged.pinned, &merged.reserved);
            if own_team_only {
                own = pins_over(ctx, &build);
            }
        }
        note_for_match_panel(ctx, &build, own.as_deref());

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

/// The build the player's athlete gets under `own_team_only`, handed to the
/// spawn injector rather than returned.
///
/// This hook cannot tell the teams apart (see [`build_config::own_team_only_enabled`]),
/// so what it returns reaches the enemy too, and that is the build the Smart
/// Builds pass makes without the pins. The player's athletes get the one the
/// toggle-off path gives: the pins merged into the engine's build
/// ([`build_config::merge_pin_row`]), which slides the engine's picks into the
/// slots between them rather than under them, and the Smart Builds pass over
/// that with the pins held in place. (Running the pass first and pasting the
/// pins over its result, as this did until 2026-09-25, threw away whatever the
/// pass had put in a pinned slot: a pin in the first slot cost the engine's
/// first pick.)
///
/// Recorded next to `unpinned`, the build the athlete will hold, for
/// `tactics::spawn_paste_pinned_build` to swap in — which it does only for the
/// player's own athletes — and keyed by the pin row it was made from, the one
/// the detours read ([`build_config::pin_row`]).
///
/// Returns the pin-aware build, for the in-match tactics panel to show; nothing
/// when no pin applies and the athlete holds `unpinned` like anyone else.
fn remember_pinned_build(
    ctx: &StableItemBuildContext<'_>,
    unpinned: &[usize],
) -> Option<Vec<usize>> {
    // Publishes the pin snapshot `pin_row` reads.
    build_config::load_cached();
    let mut row = build_config::pin_row(ctx.champion_key(), champion_role(ctx));
    row.resize(build_config::picker_slots(), None);
    let merged = build_config::merge_pin_row(&row, |key| ctx.item_index(key), ctx.base_build());
    if !merged.pinned.contains(&true) && merged.reserved.is_empty() {
        return None;
    }
    let later_open = build_config::later_slot_open(&row);
    let mut pinned = merged.items;
    enforce_smart_build(ctx, &mut pinned, &merged.pinned, &merged.reserved, later_open);
    let keys = |build: &[usize]| {
        build
            .iter()
            .map(|&index| ctx.item_key(index).map(str::to_string))
            .collect::<Option<Vec<String>>>()
    };
    if let (Some(from), Some(to)) = (keys(unpinned), keys(&pinned)) {
        crate::own_team_log::line(|| {
            format!(
                "stable hook: {} lane={:?} side={:?} row={:?} shared={:?} pinned={:?}",
                ctx.champion_key(),
                champion_role(ctx),
                ctx.team(),
                row,
                from,
                to
            )
        });
        build_config::remember_pinned_build(ctx.champion_key(), row, from, to);
    }
    Some(pinned)
}

/// The build the player's athlete holds under `own_team_only` with Smart
/// Builds off, where nothing is recorded for the spawn injector to swap in:
/// the detours lay each pin over its own slot of the build they find. Nothing
/// when no pin applies.
fn pins_over(ctx: &StableItemBuildContext<'_>, build: &[usize]) -> Option<Vec<usize>> {
    // Publishes the pin snapshot `pin_row` reads.
    build_config::load_cached();
    let row = build_config::pin_row(ctx.champion_key(), champion_role(ctx));
    let mut own = build.to_vec();
    let mut pinned = false;
    for (slot, item) in own.iter_mut().enumerate() {
        let pin = row
            .get(slot)
            .and_then(Option::as_ref)
            .and_then(|key| ctx.item_index(key));
        if let Some(index) = pin {
            *item = index;
            pinned = true;
        }
    }
    pinned.then_some(own)
}

/// Hands the build this champion ends up with to the in-match tactics panel
/// ([`crate::match_builds`]), which shows both teams. Every match's builds
/// pass through here and this cannot tell whose a build is, so all of them
/// are noted and the panel picks its match out by lineup: `build` is what
/// anyone playing the champion is handed, and `own` what the player's athlete
/// is, where `own_team_only` makes that another build.
fn note_for_match_panel(ctx: &StableItemBuildContext<'_>, build: &[usize], own: Option<&[usize]>) {
    let keys = |build: &[usize]| {
        build
            .iter()
            .map(|&index| ctx.item_key(index).map(str::to_string))
            .collect::<Option<Vec<String>>>()
    };
    let Some(shared) = keys(build) else {
        return;
    };
    crate::match_builds::note_decision(
        ctx.champion_key(),
        ctx.lane().map(|lane| lane.code() as usize),
        &ctx.ally_champions(),
        &ctx.enemy_champions(),
        shared,
        own.and_then(keys),
    );
}

/// Smart Builds rule 8's role half, which holds with the toggle off: an AI
/// pick that is a jungle item makes way on a champion that is not jungling.
/// See [`smart_builds::keep_jungle_items_in_jungle`].
fn keep_jungle_items_in_jungle(
    ctx: &StableItemBuildContext<'_>,
    build: &mut [usize],
    pinned: &[bool],
    reserved: &[usize],
) {
    smart_builds::keep_jungle_items_in_jungle(
        ctx.item_count(),
        build,
        pinned,
        reserved,
        champion_fit(ctx),
        |index| ctx.item_key(index).map(str::to_string),
        |index| ctx.item_category(index),
        |index| is_selectable_final(ctx, index),
    );
}

/// The Smart Builds pass over a build the host handed us, with the catalog seen
/// through `StableItemBuildContext`. The rules themselves live in
/// [`crate::smart_builds`], which the training-screen detour in `crate::hook`
/// drives over the same build with its own accessors.
fn enforce_smart_build(
    ctx: &StableItemBuildContext<'_>,
    build: &mut [usize],
    pinned: &[bool],
    reserved: &[usize],
    later_open: bool,
) {
    // This is the one path that sees the enemy lineup, which is what picks a
    // tank's boots.
    let enemies = ctx.enemy_champions();
    let boots = smart_builds::boots_for(ctx.champion_key(), champion_role(ctx), &enemies);
    build_config::remember_rule_boots(ctx.champion_key(), boots);
    smart_builds::enforce(
        ctx.item_count(),
        build,
        pinned,
        reserved,
        later_open,
        champion_fit(ctx),
        ctx.item_index(boots),
        |index| ctx.item_key(index).map(str::to_string),
        |index| ctx.item_category(index),
        |index| is_selectable_final(ctx, index),
    );
}

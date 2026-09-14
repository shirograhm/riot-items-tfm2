//! Immolate on the vanilla Sunfire Cape.
//!
//! Sunfire Cape is the base game's `hourglass_of_eternity`, renamed in
//! `text/item.i18n`. It is an engine item, so there is no `StableItem` to hang
//! the passive on: its Mending regen is the data field `flat_regen`, and
//! registering a mod item under the same key is not a documented override for
//! items. Instead the match hook looks for holders every tick and burns their
//! surroundings once a second, which leaves the vanilla item itself untouched.
//!
//! Radiant Sunfire Cape (`giants_horn_shard`) already has this aura built into
//! the engine (`flat_aoe_damage` / `max_hp_aoe_ratio` / `aoe_range`), so it is
//! deliberately not handled here. The numbers below mirror its vanilla values,
//! matching the tooltip.

use mod_api_stable::*;

use crate::{percent_of, DISTANCE_UNITS_PER_RANGE, TICKS_PER_SECOND};

const SUNFIRE_KEY: &str = "hourglass_of_eternity";

const IMMOLATE_FLAT_DAMAGE: usize = 10;
const IMMOLATE_MAX_HP_PERCENT: f64 = 1.0;
const IMMOLATE_RANGE: usize = 30;

/// Deals one second of Immolate for every living Sunfire Cape holder.
pub(crate) fn immolate(sim: &mut StableSim<'_>) {
    if sim.tick() % TICKS_PER_SECOND as usize != 0 {
        return;
    }

    let mut burns = Vec::new();
    for index in 0..sim.player_count() {
        let Some(player) = sim.player_at(index) else {
            continue;
        };
        if !player.item_keys().iter().any(|key| key == SUNFIRE_KEY) {
            continue;
        }
        let Some(champion) = player.champion() else {
            continue;
        };
        if !champion.is_alive() {
            continue;
        }
        let damage = IMMOLATE_FLAT_DAMAGE + percent_of(champion.hp().1, IMMOLATE_MAX_HP_PERCENT);
        burns.push((champion.id(), champion.team(), damage));
    }

    let range = (IMMOLATE_RANGE * DISTANCE_UNITS_PER_RANGE) as u64;
    let range_sq = range * range;
    for (caster, caster_team, damage) in burns {
        let targets: Vec<usize> = (0..sim.entity_count())
            .filter_map(|index| sim.entity_at(index))
            .filter(|entity| {
                entity.is_alive() && !entity.is_tower() && entity.team() != caster_team
            })
            .map(|entity| entity.id())
            .filter(|&id| sim.distance_sq(caster, id) <= range_sq)
            .collect();

        for target in targets {
            sim.deal_damage(caster, target, 0, damage, AttackTypeV1::Item);
        }
    }
}

/// The mod's one match hook: Immolate while the match runs, then the
/// end-of-match item capture.
pub(crate) struct MatchHooks;

impl StableMatchHook for MatchHooks {
    fn on_match_start(&self, sim: &mut StableSim<'_>) {
        crate::item_stats_sim::EndOfMatchItems.on_match_start(sim);
    }

    fn on_match_tick(&self, sim: &mut StableSim<'_>, rng_seed: u64) {
        if !sim.is_end() {
            immolate(sim);
        }
        crate::item_stats_sim::EndOfMatchItems.on_match_tick(sim, rng_seed);
    }
}

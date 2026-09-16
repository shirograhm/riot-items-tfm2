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
//! deliberately not handled here. Its numbers, like the rest of the HP line's
//! stats, are written into `setting/item_setting.item_setting` by
//! `apply_config.ps1`; only this scripted aura is read from config here, under
//! the `sunfire_cape` entry.

use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, DISTANCE_UNITS_PER_RANGE, TICKS_PER_SECOND};

const SUNFIRE_KEY: &str = "hourglass_of_eternity";

/// Sunfire Cape's Immolate numbers. Defaults mirror Radiant Sunfire Cape's
/// vanilla aura, matching the tooltip.
#[derive(Clone, Debug)]
pub(crate) struct Immolate {
    effect_bonus_flat_damage: usize,
    effect_caster_hp_percent_damage: f64,
    effect_max_distance: usize,
}

impl Default for Immolate {
    fn default() -> Self {
        Self {
            effect_bonus_flat_damage: 10,
            effect_caster_hp_percent_damage: 1.0,
            effect_max_distance: 30,
        }
    }
}

impl Immolate {
    pub(crate) fn with_config(cfg: &ItemConfig) -> Self {
        let mut immolate = Self::default();
        apply_config!(
            immolate,
            cfg,
            [
                effect_bonus_flat_damage,
                effect_caster_hp_percent_damage,
                effect_max_distance
            ]
        );
        immolate
    }
}

/// Deals one second of Immolate for every living Sunfire Cape holder.
fn immolate(sim: &mut StableSim<'_>, numbers: &Immolate) {
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
        let damage = numbers.effect_bonus_flat_damage
            + percent_of(champion.hp().1, numbers.effect_caster_hp_percent_damage);
        burns.push((champion.id(), champion.team(), damage));
    }

    let range = (numbers.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
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
pub(crate) struct MatchHooks {
    pub(crate) immolate: Immolate,
}

impl StableMatchHook for MatchHooks {
    fn on_match_start(&self, sim: &mut StableSim<'_>) {
        crate::item_stats_sim::EndOfMatchItems.on_match_start(sim);
    }

    fn on_match_tick(&self, sim: &mut StableSim<'_>, rng_seed: u64) {
        if !sim.is_end() {
            immolate(sim, &self.immolate);
        }
        crate::item_stats_sim::EndOfMatchItems.on_match_tick(sim, rng_seed);
    }
}

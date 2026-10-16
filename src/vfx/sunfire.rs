//! Immolate on the vanilla Sunfire Cape (`hourglass_of_eternity`), which has no
//! `StableItem` to hang a passive on, so the match hook burns around its
//! holders once a second.
//!
//! Radiant Sunfire Cape (`giants_horn_shard`) burns through its built-in engine
//! aura; the hook only adds the flames, the bonus against minions and monsters,
//! and the reach past the engine's fixed range for a bigger holder. Both read
//! the `sunfire_cape` and `radiant_sunfire_cape` config entries.

use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, immolate_burn, mark_immolate, percent_of, sized_range, DISTANCE_UNITS_PER_RANGE,
    TICKS_PER_SECOND,
};

use super::immolate_burn_between;

const SUNFIRE_KEY: &str = "hourglass_of_eternity";
// Radiant Sunfire Cape. The engine burns for it; the hook only adds the flames.
const RADIANT_SUNFIRE_KEY: &str = "giants_horn_shard";

// Immolate numbers for one of the two capes. Defaults mirror Radiant Sunfire
// Cape's vanilla aura, matching the tooltip.
#[derive(Clone, Debug)]
pub(crate) struct Immolate {
    effect_bonus_flat_damage: usize,
    effect_caster_hp_percent_damage: f64,
    effect_max_distance: usize,
    effect_minion_bonus_percent: f64,
}

impl Default for Immolate {
    fn default() -> Self {
        Self {
            effect_bonus_flat_damage: 10,
            effect_caster_hp_percent_damage: 1.0,
            effect_max_distance: 30,
            effect_minion_bonus_percent: 50.0,
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
                effect_max_distance,
                effect_minion_bonus_percent
            ]
        );
        immolate
    }

    // One second of the burn from a champion with `max_hp`.
    fn damage(&self, max_hp: usize) -> usize {
        self.effect_bonus_flat_damage + percent_of(max_hp, self.effect_caster_hp_percent_damage)
    }

    // What minions and monsters take on top of `damage`.
    fn minion_bonus(&self, damage: usize) -> usize {
        percent_of(damage, self.effect_minion_bonus_percent)
    }
}

// Deals one second of Immolate for every living Sunfire Cape holder, adds
// the minion and monster bonus to the engine's burn for every Radiant Sunfire
// Cape holder, and keeps the Immolate flames up on both.
fn immolate(sim: &mut StableSim<'_>, sunfire: &Immolate, radiant: &Immolate) {
    if sim.tick() % TICKS_PER_SECOND as usize != 0 {
        return;
    }

    let mut flames = Vec::new();
    // (caster, team, champion damage, minion and monster damage)
    let mut burns = Vec::new();
    // (caster, team, champion damage, minion and monster bonus)
    let mut radiant_burns = Vec::new();
    for index in 0..sim.player_count() {
        let Some(player) = sim.player_at(index) else {
            continue;
        };
        let keys = player.item_keys();
        let sunfire_here = keys.iter().any(|key| key == SUNFIRE_KEY);
        let radiant_here = keys.iter().any(|key| key == RADIANT_SUNFIRE_KEY);
        if !sunfire_here && !radiant_here {
            continue;
        }
        let Some(champion) = player.champion() else {
            continue;
        };
        if !champion.is_alive() {
            continue;
        }
        let (id, team, max_hp) = (champion.id(), champion.team(), champion.hp().1);
        flames.push(id);
        if sunfire_here {
            let damage = sunfire.damage(max_hp);
            let minion_damage = damage + sunfire.minion_bonus(damage);
            burns.push((id, team, damage, minion_damage));
        }
        if radiant_here {
            let damage = radiant.damage(max_hp);
            radiant_burns.push((id, team, damage, radiant.minion_bonus(damage)));
        }
    }

    for champion in flames {
        mark_immolate(sim, champion);
    }

    for (caster, caster_team, damage, minion_damage) in burns {
        let range = sunfire.effect_max_distance;
        immolate_burn(sim, caster, caster_team, range, damage, minion_damage);
    }

    for (caster, caster_team, damage, minion_bonus) in radiant_burns {
        // The engine burns everyone within its own range for the base damage
        // itself, so there the hook only adds the bonus.
        let engine_reach = (radiant.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        immolate_burn_between(sim, caster, caster_team, 0, engine_reach, 0, minion_bonus);
        // The engine's range does not grow with the holder, so the band a
        // bigger holder adds past it ([`sized_range`]) is burnt here in full.
        let reach = sized_range(sim, caster, radiant.effect_max_distance);
        if reach > engine_reach {
            let minion_damage = damage + minion_bonus;
            immolate_burn_between(
                sim,
                caster,
                caster_team,
                engine_reach,
                reach,
                damage,
                minion_damage,
            );
        }
    }
}

// The mod's one match hook: Immolate and Shared Riches while the match runs,
// then the end-of-match item capture.
pub(crate) struct MatchHooks {
    pub(crate) immolate: Immolate,
    pub(crate) radiant_immolate: Immolate,
    // The World Atlas line's gold, which keeps coming while its carrier is dead.
    pub(crate) riches: crate::SharedRiches,
}

impl StableMatchHook for MatchHooks {
    fn on_match_start(&self, sim: &mut StableSim<'_>) {
        crate::item_stats::sim::EndOfMatchItems.on_match_start(sim);
    }

    fn on_match_tick(&self, sim: &mut StableSim<'_>, rng_seed: u64) {
        use crate::perf::{self, Section};
        // Which match is on screen, for the Check Tactics panel's builds; other
        // simulations return at once.
        crate::match_builds::on_match_tick(sim);
        // What its athletes buy, for the item tooltips' cost.
        crate::item_spend::on_match_tick(sim);
        // What they hold, for the counts drawn on their item icons.
        crate::item_counts::on_match_tick(sim);
        if perf::ENABLED {
            perf::sim_tick(sim.seed(), sim.tick(), sim.is_end());
        }
        let _probe = perf::Probe::sim(Section::MatchTick);
        // What a balance patch has made of the flat stats of the items each
        // champion holds, kept on the champion.
        crate::patches::on_match_tick(sim);
        if !sim.is_end() {
            perf::time(Section::MatchImmolate, || {
                immolate(sim, &self.immolate, &self.radiant_immolate)
            });
            perf::time(Section::MatchRiches, || self.riches.pay(sim));
        }
        perf::time(Section::MatchCapture, || {
            crate::item_stats::sim::EndOfMatchItems.on_match_tick(sim, rng_seed)
        });
    }
}

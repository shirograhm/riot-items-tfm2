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
//! the engine (`flat_aoe_damage` / `max_hp_aoe_ratio` / `aoe_range`), so its
//! burn is deliberately not handled here. The hook gives its holders the same
//! Immolate flames, and adds the part the engine lacks: the bonus against
//! minions and monsters, as a second hit on them alone. That assumes the
//! engine burns once a second like every other Immolate. Its numbers, like the
//! rest of the HP line's stats, are written into
//! `setting/item_setting.item_setting` by `apply_config.ps1`. The hook reads
//! the same config entries, `sunfire_cape` and `radiant_sunfire_cape`.

use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, immolate_burn, mark_immolate, percent_of, TICKS_PER_SECOND};

const SUNFIRE_KEY: &str = "hourglass_of_eternity";
/// Radiant Sunfire Cape. The engine burns for it; the hook only adds the flames.
const RADIANT_SUNFIRE_KEY: &str = "giants_horn_shard";

/// Immolate numbers for one of the two capes. Defaults mirror Radiant Sunfire
/// Cape's vanilla aura, matching the tooltip.
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

    /// One second of the burn from a champion with `max_hp`.
    fn damage(&self, max_hp: usize) -> usize {
        self.effect_bonus_flat_damage + percent_of(max_hp, self.effect_caster_hp_percent_damage)
    }

    /// What minions and monsters take on top of `damage`.
    fn minion_bonus(&self, damage: usize) -> usize {
        percent_of(damage, self.effect_minion_bonus_percent)
    }
}

/// Deals one second of Immolate for every living Sunfire Cape holder, adds
/// the minion and monster bonus to the engine's burn for every Radiant Sunfire
/// Cape holder, and keeps the Immolate flames up on both.
fn immolate(sim: &mut StableSim<'_>, sunfire: &Immolate, radiant: &Immolate) {
    if sim.tick() % TICKS_PER_SECOND as usize != 0 {
        return;
    }

    let mut flames = Vec::new();
    // (caster, team, range, champion damage, minion and monster damage)
    let mut burns = Vec::new();
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
            burns.push((id, team, sunfire.effect_max_distance, damage, minion_damage));
        }
        if radiant_here {
            // The engine burns everyone for the base damage itself.
            let minion_bonus = radiant.minion_bonus(radiant.damage(max_hp));
            burns.push((id, team, radiant.effect_max_distance, 0, minion_bonus));
        }
    }

    for champion in flames {
        mark_immolate(sim, champion);
    }

    for (caster, caster_team, range, damage, minion_damage) in burns {
        immolate_burn(sim, caster, caster_team, range, damage, minion_damage);
    }
}

/// The mod's one match hook: Immolate while the match runs, then the
/// end-of-match item capture.
pub(crate) struct MatchHooks {
    pub(crate) immolate: Immolate,
    pub(crate) radiant_immolate: Immolate,
}

impl StableMatchHook for MatchHooks {
    fn on_match_start(&self, sim: &mut StableSim<'_>) {
        crate::item_stats::sim::EndOfMatchItems.on_match_start(sim);
    }

    fn on_match_tick(&self, sim: &mut StableSim<'_>, rng_seed: u64) {
        if !sim.is_end() {
            immolate(sim, &self.immolate, &self.radiant_immolate);
        }
        crate::item_stats::sim::EndOfMatchItems.on_match_tick(sim, rng_seed);
    }
}

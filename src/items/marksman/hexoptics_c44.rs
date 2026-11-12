use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::vfx::reach_ring::{self, RING_REACHES};
use crate::{apply_config, ItemMeta, DISTANCE_UNITS_PER_RANGE};

// Magnification: basic attacks deal up to `effect_max_percent_bonus` more
// damage, growing with the distance to the target up to `effect_max_distance`.
// Turrets are not magnified.
//
// Arcane Aim: a champion takedown (kill or assist) grants `effect_bonus_range`
// attack range for `effect_duration_seconds`; another takedown renews it. One
// timed buff carries the range and draws a ring at the new reach
// ([`reach_ring`]; `effects/arcane_aim_ring_<reach>`, bound in
// `view/effects.view_effects`).

// The ring buff for each of [`RING_REACHES`].
const RINGS: [&str; RING_REACHES.len()] = [
    "riot_arcane_aim_40",
    "riot_arcane_aim_50",
    "riot_arcane_aim_60",
    "riot_arcane_aim_70",
    "riot_arcane_aim_80",
    "riot_arcane_aim_90",
    "riot_arcane_aim_100",
    "riot_arcane_aim_110",
];

#[derive(Clone, Debug)]
pub struct HexopticsC44 {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    crit_chance: i32,
    effect_max_percent_bonus: f64,
    effect_max_distance: usize,
    effect_bonus_range: usize,
    effect_duration_seconds: f64,
}

impl HexopticsC44 {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("hexoptics_c44", &["noonquiver", "bf_sword"], &["radiant_hexoptics_c44"]),
            price: 700,
            attack: 50,
            crit_chance: 20,
            effect_max_percent_bonus: 10.0,
            effect_max_distance: 55,
            effect_bonus_range: 10,
            effect_duration_seconds: 8.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_hexoptics_c44", &["hexoptics_c44"]),
            price: 1000,
            attack: 85,
            crit_chance: 25,
            effect_max_percent_bonus: 15.0,
            effect_max_distance: 55,
            effect_bonus_range: 15,
            effect_duration_seconds: 8.0,
        }
    }

    pub fn with_config(cfg: &ItemConfig) -> Self {
        Self::base().configured(cfg)
    }

    pub fn radiant_with_config(cfg: &ItemConfig) -> Self {
        Self::radiant().configured(cfg)
    }

    fn configured(mut self, cfg: &ItemConfig) -> Self {
        apply_config!(
            self,
            cfg,
            [
                price,
                attack,
                crit_chance,
                effect_max_percent_bonus,
                effect_max_distance,
                effect_bonus_range,
                effect_duration_seconds
            ]
        );
        self
    }

    // Arcane Aim on `champion`, replacing any ring already up.
    fn arcane_aim(&self, ctx: &mut StableSim<'_>, champion: usize) {
        if !ctx.get_entity(champion).is_some_and(|c| c.is_alive()) {
            return;
        }
        let ticks = (self.effect_duration_seconds * 60.0).round() as usize;
        reach_ring::clear_rings(ctx, champion, &RINGS);
        let name = reach_ring::ring_for(ctx, champion, self.effect_bonus_range, &RINGS);
        ctx.add_buff(
            champion,
            &reach_ring::range_buff(name, self.effect_bonus_range, ticks),
        );
    }
}

impl Default for HexopticsC44 {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for HexopticsC44 {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        self.meta.key.to_string()
    }

    fn icon(&self) -> String {
        self.meta.key.to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        self.meta.tier
    }

    fn previous_tier(&self) -> Vec<String> {
        self.meta.previous_tier()
    }

    fn next_tier(&self) -> Vec<String> {
        self.meta.next_tier()
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack: self.attack,
            crit_chance: self.crit_chance,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        if ctx.get_entity(target).map_or(true, |t| t.is_tower()) {
            return;
        }
        let distance = (ctx.distance_sq(caster, target) as f64).sqrt();
        let ratio =
            (distance / (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as f64).min(1.0);
        let scaling = 1.0 + (self.effect_max_percent_bonus / 100.0) * ratio;
        *damage = (*damage as f64 * scaling).round() as usize;
    }

    // A takedown on a champion. Minions, monsters and turrets grant nothing.
    fn on_kill(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _player: usize,
        entity: usize,
        victim: usize,
    ) {
        if ctx.get_entity(victim).is_some_and(|victim| victim.is_champion()) {
            self.arcane_aim(ctx, entity);
        }
    }

    fn on_assist(&mut self, ctx: &mut StableSim<'_>, _player: usize, entity: usize) {
        self.arcane_aim(ctx, entity);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::AttackRange]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

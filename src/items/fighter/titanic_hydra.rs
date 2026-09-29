use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ItemMeta, ProcQueue, DISTANCE_UNITS_PER_RANGE};

// Cleave, scaled on the carrier's maximum health instead of Tiamat's Attack
// Damage: basic attacks hit their target for a share of it and every other
// enemy near the target for a larger share. The splash radius, the tower
// exclusion, the ranged falloff and the swing effect are Tiamat's and Ravenous
// Hydra's, so the three read as one Cleave.

#[derive(Clone, Debug)]
pub struct TitanicHydra {
    meta: ItemMeta,
    cleave_effect: &'static str,
    price: usize,
    attack: i32,
    hp: i32,
    effect_caster_hp_percent_damage: f64,
    effect_splash_caster_hp_percent: f64,
    effect_max_distance: usize,
    effect_melee_distance: usize,
    effect_ranged_percent: f64,
    // Non-vital stats (internals)
    procs: ProcQueue,
}

impl TitanicHydra {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("titanic_hydra", &["tiamat"], &["radiant_titanic_hydra"]),
            cleave_effect: "riot_ravenous_hydra_cleave",
            price: 750,
            attack: 25,
            hp: 200,
            effect_caster_hp_percent_damage: 1.0,
            effect_splash_caster_hp_percent: 3.0,
            effect_max_distance: 35,
            effect_melee_distance: 35,
            effect_ranged_percent: 50.0,
            // Non-vital stats (internals)
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_titanic_hydra", &["titanic_hydra"]),
            price: 1000,
            attack: 40,
            hp: 300,
            effect_caster_hp_percent_damage: 1.5,
            effect_splash_caster_hp_percent: 4.0,
            effect_max_distance: 35,
            effect_melee_distance: 35,
            effect_ranged_percent: 50.0,
            ..Self::base()
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
                hp,
                effect_caster_hp_percent_damage,
                effect_splash_caster_hp_percent,
                effect_max_distance,
                effect_melee_distance,
                effect_ranged_percent
            ]
        );
        self
    }

    fn splash_targets(&self, ctx: &StableSim<'_>, caster_team: usize, target: usize) -> Vec<usize> {
        let range = (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        let range_sq = range * range;

        let mut splashed = Vec::new();
        for index in 0..ctx.entity_count() {
            let Some(entity_ref) = ctx.entity_at(index) else {
                continue;
            };
            let id = entity_ref.id();
            if id == target {
                continue;
            }
            // Towers are enemy entities too, and Cleave is not meant for them.
            if !entity_ref.is_alive() || entity_ref.is_tower() || entity_ref.team() == caster_team {
                continue;
            }
            if ctx.distance_sq(target, id) > range_sq {
                continue;
            }
            splashed.push(id);
        }
        splashed
    }
}

impl Default for TitanicHydra {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for TitanicHydra {
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
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.procs.clear();
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }
        let Some((caster_team, max_hp)) = ctx.get_entity(caster).map(|c| (c.team(), c.hp().1))
        else {
            return;
        };
        let mut on_hit = percent_of(max_hp, self.effect_caster_hp_percent_damage);
        let mut splash = percent_of(max_hp, self.effect_splash_caster_hp_percent);

        let reach = (self.effect_melee_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        if ctx.distance_sq(caster, target) > reach * reach {
            on_hit = percent_of(on_hit, self.effect_ranged_percent);
            splash = percent_of(splash, self.effect_ranged_percent);
        }

        self.procs.push_physical(ctx, target, on_hit);
        if splash == 0 {
            return;
        }

        let splashed = self.splash_targets(ctx, caster_team, target);
        ctx.play_view_effect(
            self.cleave_effect,
            caster,
            &InputTargetV1::target(target),
            0,
            0,
            0,
        );

        for id in splashed {
            ctx.deal_damage(caster, id, splash, 0, AttackTypeV1::Item);
        }
    }

    /// Lands the on-hit damage whose delay has run out.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::Hp, ItemTagV1::MyHpPercentDamage]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

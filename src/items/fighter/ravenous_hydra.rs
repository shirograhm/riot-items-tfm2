use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ItemMeta, DISTANCE_UNITS_PER_RANGE};

#[derive(Clone, Debug)]
pub struct RavenousHydra {
    meta: ItemMeta,
    cleave_effect: &'static str,
    price: usize,
    attack: i32,
    vamp: i32,
    skill_cooldown_mult: i32,
    effect_ad_percent_damage: f64,
    effect_max_distance: usize,
    effect_melee_distance: usize,
    effect_ranged_percent: f64,
}

impl RavenousHydra {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "ravenous_hydra",
                &["tiamat"],
                &["radiant_ravenous_hydra"],
            ),
            cleave_effect: "riot_ravenous_hydra_cleave",
            price: 1350,
            attack: 55,
            vamp: 10,
            skill_cooldown_mult: 10,
            effect_ad_percent_damage: 30.0,
            effect_max_distance: 35,
            effect_melee_distance: 35,
            effect_ranged_percent: 50.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_ravenous_hydra", &["ravenous_hydra"]),
            price: 1900,
            attack: 90,
            vamp: 15,
            skill_cooldown_mult: 15,
            effect_ad_percent_damage: 40.0,
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
                vamp,
                skill_cooldown_mult,
                effect_ad_percent_damage,
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

impl Default for RavenousHydra {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for RavenousHydra {
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
            vamp: self.vamp,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
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
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let caster_team = caster_ref.team();
        let mut damage = percent_of(caster_ref.stat().attack, self.effect_ad_percent_damage);

        let reach = (self.effect_melee_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        if ctx.distance_sq(caster, target) > reach * reach {
            damage = percent_of(damage, self.effect_ranged_percent);
        }
        if damage == 0 {
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
            ctx.deal_damage(caster, id, damage, 0, AttackTypeV1::Item);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::Vamp, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

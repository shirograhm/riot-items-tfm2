use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, DISTANCE_UNITS_PER_RANGE};

#[derive(Clone, Debug)]
pub struct Tiamat {
    cleave_effect: &'static str,
    price: usize,
    attack: i32,
    effect_ad_percent_damage: f64,
    effect_max_distance: usize,
    effect_melee_distance: usize,
    effect_ranged_percent: f64,
}

impl Default for Tiamat {
    fn default() -> Self {
        Self {
            // The same swing Ravenous Hydra draws: one Cleave, one animation.
            cleave_effect: "riot_ravenous_hydra_cleave",
            price: 800,
            attack: 50,
            effect_ad_percent_damage: 20.0,
            effect_max_distance: 35,
            effect_melee_distance: 35,
            effect_ranged_percent: 50.0,
        }
    }
}

impl Tiamat {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [
                price,
                attack,
                effect_ad_percent_damage,
                effect_max_distance,
                effect_melee_distance,
                effect_ranged_percent
            ]
        );
        item
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

impl StableItem for Tiamat {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "tiamat".to_string()
    }

    fn icon(&self) -> String {
        "tiamat".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["soldiers_longsword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["ravenous_hydra".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack: self.attack,
            ..Default::default()
        }
    }

    /// Cleave. The weaker half of Ravenous Hydra's, on the same terms: the
    /// splash is measured around the target, and a ranged wielder lands it at
    /// reduced strength.
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
        vec![ItemTagV1::Ad]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, apply_lethality, cleave_targets, percent_of, ItemMeta, DISTANCE_UNITS_PER_RANGE,
};

// Profane Hydra: Lethality and Ravenous Hydra's Cleave, without League's
// Heretical Cleave active. It shares Tiamat's and Ravenous Hydra's swing.
#[derive(Clone, Debug)]
pub struct ProfaneHydra {
    meta: ItemMeta,
    cleave_effect: &'static str,
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_lethality: usize,
    effect_ad_percent_damage: f64,
    effect_max_distance: usize,
    effect_melee_distance: usize,
    effect_ranged_percent: f64,
}

impl ProfaneHydra {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "profane_hydra",
                &["serrated_dirk", "tiamat"],
                &["radiant_profane_hydra"],
            ),
            cleave_effect: "riot_ravenous_hydra_cleave",
            price: 700,
            attack: 40,
            skill_cooldown_mult: 10,
            effect_lethality: 18,
            effect_ad_percent_damage: 30.0,
            effect_max_distance: 35,
            effect_melee_distance: 35,
            effect_ranged_percent: 50.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_profane_hydra", &["profane_hydra"]),
            price: 1000,
            attack: 60,
            skill_cooldown_mult: 15,
            effect_ad_percent_damage: 40.0,
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
                skill_cooldown_mult,
                effect_lethality,
                effect_ad_percent_damage,
                effect_max_distance,
                effect_melee_distance,
                effect_ranged_percent
            ]
        );
        self
    }
}

impl Default for ProfaneHydra {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for ProfaneHydra {
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
            skill_cooldown_mult: self.skill_cooldown_mult,
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
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }
        // Lethality on all damage except to turrets, Cleave's splash included.
        apply_lethality(ctx, caster, target, self.effect_lethality, damage);

        if attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let caster_team = caster_ref.team();
        let mut splash = percent_of(caster_ref.stat().attack, self.effect_ad_percent_damage);

        let reach = (self.effect_melee_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        if ctx.distance_sq(caster, target) > reach * reach {
            splash = percent_of(splash, self.effect_ranged_percent);
        }
        if splash == 0 {
            return;
        }

        let splashed = cleave_targets(ctx, caster_team, target, self.effect_max_distance);
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

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

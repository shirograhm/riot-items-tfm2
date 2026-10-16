use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, apply_tiered_adaptive_force, ItemMeta, DISTANCE_UNITS_PER_RANGE};

#[derive(Clone, Debug)]
pub struct DiamondTippedSpear {
    meta: ItemMeta,
    adaptive_force_buff: &'static str,
    // The previous tier's buff and the Adaptive Force it grants. When the holder
    // already carries that item, this one only tops up the difference instead of
    // granting its full amount. `None` on the base variant, which has no earlier
    // Adaptive Force tier to stack with.
    upgrades_from: Option<(&'static str, i32)>,
    price: usize,
    attack_speed_mult: i32,
    adaptive_force: i32,
    skill_cooldown_mult: i32,
    effect_max_percent_bonus: f64,
    effect_max_distance: usize,
}

impl DiamondTippedSpear {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "diamond_tipped_spear",
                &["scouts_slingshot"],
                &["radiant_diamond_tipped_spear"],
            ),
            adaptive_force_buff: "diamond_tipped_spear_adaptive_force",
            upgrades_from: None,
            price: 750,
            attack_speed_mult: 30,
            adaptive_force: 30,
            skill_cooldown_mult: 10,
            effect_max_percent_bonus: 25.0,
            effect_max_distance: 100,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_diamond_tipped_spear", &["diamond_tipped_spear"]),
            adaptive_force_buff: "radiant_diamond_tipped_spear_adaptive_force",
            upgrades_from: Some(("diamond_tipped_spear_adaptive_force", 30)),
            price: 1150,
            attack_speed_mult: 55,
            adaptive_force: 50,
            skill_cooldown_mult: 10,
            effect_max_percent_bonus: 25.0,
            effect_max_distance: 100,
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
                attack_speed_mult,
                adaptive_force,
                skill_cooldown_mult,
                effect_max_percent_bonus,
                effect_max_distance
            ]
        );
        self
    }
}

impl Default for DiamondTippedSpear {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for DiamondTippedSpear {
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
            attack_speed_mult: self.attack_speed_mult,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        apply_tiered_adaptive_force(
            ctx,
            player,
            self.adaptive_force,
            self.adaptive_force_buff,
            self.upgrades_from,
        );
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        apply_tiered_adaptive_force(
            ctx,
            player,
            self.adaptive_force,
            self.adaptive_force_buff,
            self.upgrades_from,
        );
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };
        if entity_ref.is_tower() {
            return;
        }

        let distance_sq = ctx.distance_sq(caster, target);
        let distance = (distance_sq as f64).sqrt();
        let ratio =
            (distance / (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as f64).min(1.0);
        let scaling = 1.0 + (self.effect_max_percent_bonus / 100.0) * ratio;
        *damage = (*damage as f64 * scaling).round() as usize;
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::Ap, ItemTagV1::AttackSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}

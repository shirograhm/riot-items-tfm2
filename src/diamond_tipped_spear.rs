use mod_api::*;

use crate::apply_adaptive_force;
use crate::config::ItemConfig;

// Config `effect_max_distance` is expressed in attack-range units; multiply by
// this to convert to the raw game distance units that `distance_sq` works in.
const DISTANCE_UNITS_PER_RANGE: f64 = 1000.0;

#[derive(Clone, Debug)]
pub struct DiamondTippedSpear {
    price: usize,
    attack_speed_mult: i32,
    adaptive_force: i32,
    skill_cooldown_mult: i32,
    effect_max_percent_bonus: f64,
    effect_max_distance: usize,
}

impl Default for DiamondTippedSpear {
    fn default() -> Self {
        Self {
            price: 1500,
            attack_speed_mult: 35,
            adaptive_force: 75,
            skill_cooldown_mult: 10,
            effect_max_percent_bonus: 30.0,
            effect_max_distance: 100,
        }
    }
}

impl DiamondTippedSpear {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            adaptive_force: cfg.adaptive_force.unwrap_or(d.adaptive_force),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_max_percent_bonus: cfg
                .effect_max_percent_bonus
                .unwrap_or(d.effect_max_percent_bonus),
            effect_max_distance: cfg.effect_max_distance.unwrap_or(d.effect_max_distance),
        }
    }
}

impl ModItemInfo for DiamondTippedSpear {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "diamond_tipped_spear"
    }

    fn icon(&self) -> &str {
        "diamond_tipped_spear"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["scouts_slingshot".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_diamond_tipped_spear".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack_speed_mult: self.attack_speed_mult,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        apply_adaptive_force(
            ctx,
            player,
            self.adaptive_force,
            "diamond_tipped_spear_adaptive_force",
        );
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        apply_adaptive_force(
            ctx,
            player,
            self.adaptive_force,
            "diamond_tipped_spear_adaptive_force",
        );
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
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
            (distance / (self.effect_max_distance as f64 * DISTANCE_UNITS_PER_RANGE)).min(1.0);
        let scaling = 1.0 + (self.effect_max_percent_bonus / 100.0) * ratio;
        *damage = (*damage as f64 * scaling) as usize;
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AP, ItemTag::AS]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

#[derive(Clone, Debug)]
pub struct RadiantDiamondTippedSpear {
    price: usize,
    attack_speed_mult: i32,
    adaptive_force: i32,
    skill_cooldown_mult: i32,
    effect_max_percent_bonus: f64,
    effect_max_distance: usize,
}

impl Default for RadiantDiamondTippedSpear {
    fn default() -> Self {
        Self {
            price: 2250,
            attack_speed_mult: 60,
            adaptive_force: 120,
            skill_cooldown_mult: 10,
            effect_max_percent_bonus: 30.0,
            effect_max_distance: 100,
        }
    }
}

impl RadiantDiamondTippedSpear {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            adaptive_force: cfg.adaptive_force.unwrap_or(d.adaptive_force),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_max_percent_bonus: cfg
                .effect_max_percent_bonus
                .unwrap_or(d.effect_max_percent_bonus),
            effect_max_distance: cfg.effect_max_distance.unwrap_or(d.effect_max_distance),
        }
    }

    pub fn apply_buff(&self, ctx: &mut GameCtx, player: usize) {
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(entity_ref) = player_ref.champion() else {
            return;
        };

        let is_prior_buff_applied = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "diamond_tipped_spear_adaptive_force");
        let force_to_apply = if is_prior_buff_applied {
            self.adaptive_force - DiamondTippedSpear::default().adaptive_force
        } else {
            self.adaptive_force
        };

        apply_adaptive_force(
            ctx,
            player,
            force_to_apply,
            "radiant_diamond_tipped_spear_adaptive_force",
        );
    }
}

impl ModItemInfo for RadiantDiamondTippedSpear {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_diamond_tipped_spear"
    }

    fn icon(&self) -> &str {
        "radiant_diamond_tipped_spear"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["diamond_tipped_spear".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack_speed_mult: self.attack_speed_mult,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.apply_buff(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_buff(ctx, player);
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
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
            (distance / (self.effect_max_distance as f64 * DISTANCE_UNITS_PER_RANGE)).min(1.0);
        let scaling = 1.0 + (self.effect_max_percent_bonus / 100.0) * ratio;
        *damage = (*damage as f64 * scaling) as usize;
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AP, ItemTag::AS]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

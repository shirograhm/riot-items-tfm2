use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_lethality, percent_of};

// Lethality (see serrated_dirk) plus Sabotage: a kill readies a single Sabotage
// charge (max one at a time -- a later kill just refreshes it). The charge
// empowers the wielder's next basic attack against a turret to deal bonus physical
// damage (flat + % of Attack Damage) and is consumed by that hit, so one kill is
// one proc and there is no cooldown between kills. The charge lasts
// `effect_duration_seconds`; if it isn't spent on a turret in that time it lapses.
// Tracked per-item as a tick countdown (0 = no charge) since the engine has no
// removable buff to model a consumable charge.

fn sabotage_bonus(ctx: &mut GameCtx, caster: usize, flat: usize, ad_percent: f64) -> usize {
    let caster_ad = ctx.get_entity(caster).map(|c| c.stat().attack).unwrap_or(0);
    flat + percent_of(caster_ad, ad_percent)
}

#[derive(Clone, Debug)]
pub struct Bastionbreaker {
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_lethality: usize,
    effect_bonus_flat_damage: usize,
    effect_ad_percent_damage: f64,
    effect_duration_seconds: f64,
    // Remaining ticks on the current Sabotage charge (0 = no charge).
    sabotage_remaining: usize,
}

impl Default for Bastionbreaker {
    fn default() -> Self {
        Self {
            price: 1250,
            attack: 65,
            skill_cooldown_mult: 15,
            effect_lethality: 22,
            effect_bonus_flat_damage: 150,
            effect_ad_percent_damage: 15.0,
            effect_duration_seconds: 90.0,
            sabotage_remaining: 0,
        }
    }
}

impl Bastionbreaker {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_lethality: cfg.effect_lethality.unwrap_or(d.effect_lethality),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_ad_percent_damage: cfg
                .effect_ad_percent_damage
                .unwrap_or(d.effect_ad_percent_damage),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            sabotage_remaining: 0,
        }
    }

    fn charge_ticks(&self) -> usize {
        (self.effect_duration_seconds * 60.0).round() as usize
    }
}

impl ModItemInfo for Bastionbreaker {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "bastionbreaker"
    }

    fn icon(&self) -> &str {
        "bastionbreaker"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["serrated_dirk".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_bastionbreaker".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.sabotage_remaining = 0;
    }

    fn update(&mut self, _ctx: &mut GameCtx, _rng_seed: u64, _player: usize) {
        self.sabotage_remaining = self.sabotage_remaining.saturating_sub(1);
    }

    fn on_kill(&mut self, ctx: &mut GameCtx, _rng_seed: u64, _player: usize, entity: usize) {
        // Check if target is a champion
        let Some(target_ref) = ctx.get_entity(entity) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        };

        self.sabotage_remaining = self.charge_ticks();
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
        apply_lethality(ctx, target, self.effect_lethality, damage);

        if self.sabotage_remaining == 0 {
            return;
        }
        let is_tower = ctx
            .get_entity(target)
            .map(|t| t.is_tower())
            .unwrap_or(false);
        if !is_tower {
            return;
        }
        self.sabotage_remaining = 0; // spend one charge on this turret hit
        let bonus = sabotage_bonus(
            ctx,
            caster,
            self.effect_bonus_flat_damage,
            self.effect_ad_percent_damage,
        );
        ctx.deal_damage(caster, target, bonus, 0, AttackType::Item);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantBastionbreaker {
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_lethality: usize,
    effect_bonus_flat_damage: usize,
    effect_ad_percent_damage: f64,
    effect_duration_seconds: f64,
    sabotage_remaining: usize,
}

impl Default for RadiantBastionbreaker {
    fn default() -> Self {
        Self {
            price: 1850,
            attack: 110,
            skill_cooldown_mult: 20,
            effect_lethality: 22,
            effect_bonus_flat_damage: 200,
            effect_ad_percent_damage: 20.0,
            effect_duration_seconds: 90.0,
            sabotage_remaining: 0,
        }
    }
}

impl RadiantBastionbreaker {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_lethality: cfg.effect_lethality.unwrap_or(d.effect_lethality),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_ad_percent_damage: cfg
                .effect_ad_percent_damage
                .unwrap_or(d.effect_ad_percent_damage),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            sabotage_remaining: 0,
        }
    }

    fn charge_ticks(&self) -> usize {
        (self.effect_duration_seconds * 60.0).round() as usize
    }
}

impl ModItemInfo for RadiantBastionbreaker {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_bastionbreaker"
    }

    fn icon(&self) -> &str {
        "radiant_bastionbreaker"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["bastionbreaker".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.sabotage_remaining = 0;
    }

    fn update(&mut self, _ctx: &mut GameCtx, _rng_seed: u64, _player: usize) {
        self.sabotage_remaining = self.sabotage_remaining.saturating_sub(1);
    }

    fn on_kill(&mut self, ctx: &mut GameCtx, _rng_seed: u64, _player: usize, entity: usize) {
        // Check if target is a champion
        let Some(target_ref) = ctx.get_entity(entity) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        };

        self.sabotage_remaining = self.charge_ticks();
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
        apply_lethality(ctx, target, self.effect_lethality, damage);

        if self.sabotage_remaining == 0 {
            return;
        }
        let is_tower = ctx
            .get_entity(target)
            .map(|t| t.is_tower())
            .unwrap_or(false);
        if !is_tower {
            return;
        }
        self.sabotage_remaining = 0;
        let bonus = sabotage_bonus(
            ctx,
            caster,
            self.effect_bonus_flat_damage,
            self.effect_ad_percent_damage,
        );
        ctx.deal_damage(caster, target, bonus, 0, AttackType::Item);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

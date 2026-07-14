use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_lethality, count_takedowns, mark_enemy_champion, percent_of};

// Lethality (see serrated_dirk) plus Sabotage: scoring a takedown on an enemy
// champion grants a Sabotage charge lasting `effect_duration_seconds`. While the
// charge is active, the wielder's next basic attack against a turret spends it to
// deal bonus physical damage (flat + % of Attack Damage). Takedowns are detected
// via the shared damage-mark system (see `mark_enemy_champion`/`count_takedowns`).
// The charge is tracked per-item as a tick countdown (0 = no charge) since the
// engine has no removable buff to model a consumable, expiring charge.

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
    // Ticks remaining on the held Sabotage charge (0 = no charge held).
    sabotage_charge: usize,
    // Enemy champions recently damaged: (entity id, remaining window ticks).
    takedown_marks: Vec<(usize, usize)>,
}

impl Default for Bastionbreaker {
    fn default() -> Self {
        Self {
            price: 1400,
            attack: 70,
            skill_cooldown_mult: 15,
            effect_lethality: 22,
            effect_bonus_flat_damage: 150,
            effect_ad_percent_damage: 15.0,
            effect_duration_seconds: 90.0,
            sabotage_charge: 0,
            takedown_marks: Vec::new(),
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
            sabotage_charge: 0,
            takedown_marks: Vec::new(),
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
        self.sabotage_charge = 0;
        self.takedown_marks.clear();
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, _player: usize) {
        // Expire the held charge, then (re)grant a full-duration one on a takedown.
        self.sabotage_charge = self.sabotage_charge.saturating_sub(1);
        if count_takedowns(&mut self.takedown_marks, ctx) > 0 {
            self.sabotage_charge = self.charge_ticks();
        }
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
        mark_enemy_champion(&mut self.takedown_marks, ctx, caster, target);

        if self.sabotage_charge == 0 {
            return;
        }
        let is_tower = ctx.get_entity(target).map(|t| t.is_tower()).unwrap_or(false);
        if !is_tower {
            return;
        }
        let bonus = sabotage_bonus(
            ctx,
            caster,
            self.effect_bonus_flat_damage,
            self.effect_ad_percent_damage,
        );
        ctx.deal_damage(caster, target, bonus, 0, AttackType::Item);
        self.sabotage_charge = 0; // spend the charge
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        mark_enemy_champion(&mut self.takedown_marks, ctx, caster, target);
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
    sabotage_charge: usize,
    takedown_marks: Vec<(usize, usize)>,
}

impl Default for RadiantBastionbreaker {
    fn default() -> Self {
        Self {
            price: 2000,
            attack: 115,
            skill_cooldown_mult: 20,
            effect_lethality: 22,
            effect_bonus_flat_damage: 200,
            effect_ad_percent_damage: 20.0,
            effect_duration_seconds: 90.0,
            sabotage_charge: 0,
            takedown_marks: Vec::new(),
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
            sabotage_charge: 0,
            takedown_marks: Vec::new(),
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
        self.sabotage_charge = 0;
        self.takedown_marks.clear();
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, _player: usize) {
        self.sabotage_charge = self.sabotage_charge.saturating_sub(1);
        if count_takedowns(&mut self.takedown_marks, ctx) > 0 {
            self.sabotage_charge = self.charge_ticks();
        }
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
        mark_enemy_champion(&mut self.takedown_marks, ctx, caster, target);

        if self.sabotage_charge == 0 {
            return;
        }
        let is_tower = ctx.get_entity(target).map(|t| t.is_tower()).unwrap_or(false);
        if !is_tower {
            return;
        }
        let bonus = sabotage_bonus(
            ctx,
            caster,
            self.effect_bonus_flat_damage,
            self.effect_ad_percent_damage,
        );
        ctx.deal_damage(caster, target, bonus, 0, AttackType::Item);
        self.sabotage_charge = 0; // spend the charge
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        mark_enemy_champion(&mut self.takedown_marks, ctx, caster, target);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, apply_lethality, is_enemy_champion, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct Opportunity {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    move_speed_mult: i32,
    effect_lethality: usize,
    effect_bonus_lethality: usize,
    effect_out_of_combat_seconds: f64,
    effect_duration_seconds: f64,
    /// Ticks since the wielder last damaged an enemy champion. Reset on every
    /// attack or ability hit against one, so it measures time out of combat.
    idle_ticks: usize,
    /// Whether Preparation's bonus lethality is currently applying.
    prepared: bool,
}

impl Opportunity {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("opportunity", &["serrated_dirk"], &["radiant_opportunity"]),
            price: 1300,
            attack: 70,
            move_speed_mult: 0,
            effect_lethality: 18,
            effect_bonus_lethality: 7,
            effect_out_of_combat_seconds: 7.0,
            effect_duration_seconds: 3.5,
            idle_ticks: 0,
            prepared: false,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_opportunity", &["opportunity"]),
            price: 1950,
            attack: 100,
            move_speed_mult: 5,
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
                move_speed_mult,
                effect_lethality,
                effect_bonus_lethality,
                effect_out_of_combat_seconds,
                effect_duration_seconds
            ]
        );
        self
    }

    /// Lethality to apply right now: the base amount, plus Preparation's bonus
    /// while it is up.
    fn active_lethality(&self) -> usize {
        if self.prepared {
            self.effect_lethality + self.effect_bonus_lethality
        } else {
            self.effect_lethality
        }
    }

    /// Restarts the out-of-combat timer when `target` is an enemy champion.
    /// Deliberately leaves `prepared` alone: an already-earned bonus lingers for
    /// `effect_duration_seconds` past this point (see `update`).
    fn note_combat(&mut self, ctx: &mut GameCtx, caster: usize, target: usize) {
        if is_enemy_champion(ctx, caster, target) {
            self.idle_ticks = 0;
        }
    }
}

impl Default for Opportunity {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for Opportunity {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        self.meta.key
    }

    fn icon(&self) -> &str {
        self.meta.key
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

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.idle_ticks = 0;
        self.prepared = false;
    }

    fn update(&mut self, _ctx: &mut GameCtx, _rng_seed: u64, _player: usize) {
        self.idle_ticks = self.idle_ticks.saturating_add(1);
        // Two thresholds off the same timer, and the linger window is the shorter
        // of the two: the bonus is earned once the wielder has been out of combat
        // for the full ramp, and is only lost once combat has held it below that
        // ramp for longer than the linger window.
        if self.idle_ticks >= ticks(self.effect_out_of_combat_seconds) {
            self.prepared = true;
        } else if self.idle_ticks > ticks(self.effect_duration_seconds) {
            self.prepared = false;
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
        // Lethality resolves before the timer resets, so the attack that breaks
        // out-of-combat still lands with Preparation up.
        apply_lethality(ctx, target, self.active_lethality(), damage);
        self.note_combat(ctx, caster, target);
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        self.note_combat(ctx, caster, target);
    }

    fn tags(&self) -> Vec<ItemTag> {
        let mut tags = vec![ItemTag::AD];
        if self.move_speed_mult > 0 {
            tags.push(ItemTag::MoveSpeed);
        }
        tags
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

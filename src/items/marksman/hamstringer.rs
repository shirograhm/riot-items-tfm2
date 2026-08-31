use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, percent_of, ticks, ItemMeta, DOT_TICK_RATE};

const LEVEL_STEPS: f64 = 11.0;

#[derive(Clone, Debug)]
pub struct Hamstringer {
    meta: ItemMeta,
    slow_buff: &'static str,
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    crit_chance: i32,
    effect_min_bonus_damage: usize,
    effect_max_bonus_damage: usize,
    effect_crit_percent_damage: f64,
    effect_slow_amount: i32,
    effect_duration_seconds: f64,
    /// `(target, ticks left, ticks until the next instance, damage per instance)`.
    bleeds: Vec<(usize, usize, usize, usize)>,
}

impl Hamstringer {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("hamstringer", &["noonquiver"], &["radiant_hamstringer"]),
            slow_buff: "hamstringer_slow",
            price: 1450,
            attack: 45,
            attack_speed_mult: 25,
            crit_chance: 20,
            effect_min_bonus_damage: 55,
            effect_max_bonus_damage: 110,
            effect_crit_percent_damage: 100.0,
            effect_slow_amount: 7,
            effect_duration_seconds: 2.0,
            // Non-vital stats (internals)
            bleeds: Vec::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_hamstringer", &["hamstringer"]),
            price: 2150,
            attack: 75,
            attack_speed_mult: 45,
            crit_chance: 25,
            effect_min_bonus_damage: 70,
            effect_max_bonus_damage: 180,
            effect_crit_percent_damage: 100.0,
            effect_slow_amount: 7,
            effect_duration_seconds: 2.0,
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
                attack_speed_mult,
                crit_chance,
                effect_min_bonus_damage,
                effect_max_bonus_damage,
                effect_crit_percent_damage,
                effect_slow_amount,
                effect_duration_seconds
            ]
        );
        self
    }

    fn bleed_damage(&self, level: usize, crit_chance: usize) -> usize {
        let per_level = ((self.effect_max_bonus_damage - self.effect_min_bonus_damage) as f64
            / LEVEL_STEPS)
            .round() as usize;
        self.effect_min_bonus_damage
            + level.saturating_sub(1) * per_level
            + percent_of(crit_chance, self.effect_crit_percent_damage)
    }

    fn duration_ticks(&self) -> usize {
        ticks(self.effect_duration_seconds).max(DOT_TICK_RATE)
    }

    fn instance_count(&self) -> usize {
        (self.duration_ticks() / DOT_TICK_RATE).max(1)
    }

    fn apply_bleed(&mut self, target: usize, level: usize, crit_chance: usize) {
        let duration = self.duration_ticks();
        let per_instance = (self.bleed_damage(level, crit_chance) as f64
            / self.instance_count() as f64)
            .round() as usize;
        match self.bleeds.iter_mut().find(|(id, _, _, _)| *id == target) {
            Some(bleed) => {
                bleed.1 = duration;
                bleed.3 = per_instance;
            }
            None => self
                .bleeds
                .push((target, duration, DOT_TICK_RATE, per_instance)),
        }
    }

    fn tick_bleeds(&mut self, ctx: &mut StableSim<'_>, caster: usize) {
        let mut kept = Vec::with_capacity(self.bleeds.len());
        for (id, remaining, until_next, per_instance) in std::mem::take(&mut self.bleeds) {
            let remaining = remaining.saturating_sub(1);
            let mut until_next = until_next.saturating_sub(1);
            if until_next == 0 {
                let Some(target_ref) = ctx.get_entity(id) else {
                    continue;
                };
                if !target_ref.is_alive() {
                    continue;
                }
                ctx.deal_damage(caster, id, per_instance, 0, AttackTypeV1::Item);
                until_next = DOT_TICK_RATE;
            }
            if remaining > 0 {
                kept.push((id, remaining, until_next, per_instance));
            }
        }
        self.bleeds = kept;
    }
}

impl Default for Hamstringer {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Hamstringer {
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
            attack_speed_mult: self.attack_speed_mult,
            crit_chance: self.crit_chance,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.bleeds.clear();
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if self.bleeds.is_empty() {
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(player_champion) = player_ref.champion() else {
            return;
        };
        let player_champion_id = player_champion.id();

        self.tick_bleeds(ctx, player_champion_id);
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        is_crit: bool,
    ) {
        if !is_crit || attack_type == AttackTypeV1::Item {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }
        let level = caster_ref.level();
        let crit_chance = caster_ref.stat().crit_chance;

        self.apply_bleed(target, level, crit_chance);

        if has_buff(&target_ref, self.slow_buff) {
            return;
        }
        ctx.add_buff(
            target,
            &BuffV1 {
                move_speed_mult: -self.effect_slow_amount,
                ..BuffV1::timed(self.slow_buff, ticks(self.effect_duration_seconds))
            },
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::AttackSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

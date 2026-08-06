use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct Stormrazor {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    crit_chance: i32,
    effect_max_stacks: usize,
    effect_move_speed_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_duration_seconds: f64,
    energized_stacks: usize,
    energized_update_tick: usize,
}

impl Stormrazor {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("stormrazor", &["noonquiver"], &["radiant_stormrazor"]),
            price: 1550,
            attack: 65,
            attack_speed_mult: 20,
            crit_chance: 20,
            effect_max_stacks: 100,
            effect_move_speed_mult: 35,
            effect_bonus_flat_damage: 100,
            effect_duration_seconds: 1.5,
            // Non-vital stats (internals)
            energized_stacks: 0,
            energized_update_tick: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_stormrazor", &["stormrazor"]),
            price: 2200,
            attack: 100,
            attack_speed_mult: 40,
            crit_chance: 25,
            effect_max_stacks: 100,
            effect_move_speed_mult: 35,
            effect_bonus_flat_damage: 100,
            effect_duration_seconds: 1.5,
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
                effect_max_stacks,
                effect_move_speed_mult,
                effect_bonus_flat_damage,
                effect_duration_seconds
            ]
        );
        self
    }
}

impl Default for Stormrazor {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Stormrazor {
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
        self.energized_stacks = 0;
    }

    fn on_base_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        if self.energized_stacks >= self.effect_max_stacks {
            ctx.deal_damage(
                caster,
                target,
                0,
                self.effect_bonus_flat_damage,
                AttackTypeV1::Item,
            );
            ctx.add_buff(
                caster,
                &BuffV1 {
                    move_speed_mult: self.effect_move_speed_mult,
                    ..BuffV1::timed("stormrazor_move_speed", ticks(self.effect_duration_seconds))
                },
            );

            self.energized_stacks = 0;
        }

        // Gain 5 energized stacks on base attacks
        self.energized_stacks = (self.energized_stacks + 5).min(self.effect_max_stacks);
    }

    fn update(&mut self, _ctx: &mut StableSim<'_>, _rng_seed: u64, _player: usize) {
        // Add 1 energized stack per 0.2 seconds
        if self.energized_update_tick >= 12 {
            self.energized_stacks = (self.energized_stacks + 1).min(self.effect_max_stacks);
            self.energized_update_tick = 0;
        } else {
            self.energized_update_tick += 1;
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::AttackSpeed, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

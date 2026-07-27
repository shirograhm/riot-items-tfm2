use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, buff_name, ticks, ItemMeta};

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

impl ModItemInfo for Stormrazor {
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
            attack_speed_mult: self.attack_speed_mult,
            crit_chance: self.crit_chance,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut GameCtx, _player: usize) {
        self.energized_stacks = 0;
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        damage_type: DamageType,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };

        if self.energized_stacks >= self.effect_max_stacks {
            if target_ref.is_tower() {
                return;
            }

            ctx.deal_damage(
                caster,
                target,
                0,
                self.effect_bonus_flat_damage,
                AttackType::Item,
            );
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: ticks(self.effect_duration_seconds),
                    },
                    move_speed_mult: self.effect_move_speed_mult,
                    name: buff_name("stormrazor_move_speed"),
                    ..Default::default()
                },
            );

            self.energized_stacks = 0;
        }

        // Gain 5 energized stacks on attacks, up to the max stacks
        if damage_type == DamageType::AD {
            self.energized_stacks += 5;
        }
    }

    fn update(&mut self, _ctx: &mut GameCtx, _rng_seed: u64, _player: usize) {
        // Add 1 energized stack per 0.2 seconds
        if self.energized_update_tick >= 12 {
            self.energized_stacks += 1;
            self.energized_update_tick = 0;
        } else {
            self.energized_update_tick += 1;
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AS, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

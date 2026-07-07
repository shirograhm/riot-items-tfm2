use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct Stormrazor {
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

impl Default for Stormrazor {
    fn default() -> Self {
        Self {
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
}

impl Stormrazor {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            crit_chance: cfg.crit_chance.unwrap_or(d.crit_chance),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_move_speed_mult: cfg
                .effect_move_speed_mult
                .unwrap_or(d.effect_move_speed_mult),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            energized_stacks: d.energized_stacks,
            energized_update_tick: d.energized_update_tick,
        }
    }
}

impl ModItemInfo for Stormrazor {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "stormrazor"
    }

    fn icon(&self) -> &str {
        "t12_6"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["bf_sword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_stormrazor".to_string()]
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
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    move_speed_mult: self.effect_move_speed_mult,
                    name: ArrayString::try_from("stormrazor_move_speed").unwrap(),
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

#[derive(Clone, Debug)]
pub struct RadiantStormrazor {
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

impl Default for RadiantStormrazor {
    fn default() -> Self {
        Self {
            price: 2200,
            attack: 100,
            attack_speed_mult: 40,
            crit_chance: 25,
            effect_max_stacks: 100,
            effect_move_speed_mult: 35,
            effect_bonus_flat_damage: 100,
            effect_duration_seconds: 1.5,
            energized_stacks: 0,
            energized_update_tick: 0,
        }
    }
}

impl RadiantStormrazor {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            crit_chance: cfg.crit_chance.unwrap_or(d.crit_chance),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_move_speed_mult: cfg
                .effect_move_speed_mult
                .unwrap_or(d.effect_move_speed_mult),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            energized_stacks: d.energized_stacks,
            energized_update_tick: d.energized_update_tick,
        }
    }
}

impl ModItemInfo for RadiantStormrazor {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_stormrazor"
    }

    fn icon(&self) -> &str {
        "t12_7"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["stormrazor".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_stormrazor".to_string()]
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
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    move_speed_mult: self.effect_move_speed_mult,
                    name: ArrayString::try_from("stormrazor_move_speed").unwrap(),
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

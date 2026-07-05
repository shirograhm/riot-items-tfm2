use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct SpearOfShojin {
    price: usize,
    hp: i32,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_stack_attack_mult: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: usize,
}

impl Default for SpearOfShojin {
    fn default() -> Self {
        Self {
            price: 1400,
            hp: 350,
            attack: 35,
            skill_cooldown_mult: 10,
            effect_stack_attack_mult: 3,
            effect_max_stacks: 4,
            effect_duration_seconds: 5,
        }
    }
}

impl SpearOfShojin {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            attack: cfg.attack.unwrap_or(d.attack),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_stack_attack_mult: cfg
                .effect_stack_attack_mult
                .unwrap_or(d.effect_stack_attack_mult),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }

    fn add_attack_stack(&self, ctx: &mut GameCtx, caster: usize, target: usize) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }
        if !target_ref.is_champion() {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let stack_count = (0..caster_ref.buff_count())
            .filter(|&i| caster_ref.buff_at(i).name.as_str() == "spear_of_shojin_buff")
            .count();
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_duration_seconds * 60,
                    },
                    attack_mult: self.effect_stack_attack_mult,
                    name: ArrayString::try_from("spear_of_shojin_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
    }
}

impl ModItemInfo for SpearOfShojin {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "spear_of_shojin"
    }

    fn icon(&self) -> &str {
        "t10_3"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "soldiers_longsword".to_string(),
            "hardened_heart".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_spear_of_shojin".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        self.add_attack_stack(ctx, caster, target);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantSpearOfShojin {
    price: usize,
    hp: i32,
    attack: i32,
    skill_cooldown_mult: i32,
    effect_stack_attack_mult: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: usize,
}

impl Default for RadiantSpearOfShojin {
    fn default() -> Self {
        Self {
            price: 2200,
            hp: 600,
            attack: 60,
            skill_cooldown_mult: 20,
            effect_stack_attack_mult: 3,
            effect_max_stacks: 4,
            effect_duration_seconds: 5,
        }
    }
}

impl RadiantSpearOfShojin {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            attack: cfg.attack.unwrap_or(d.attack),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_stack_attack_mult: cfg
                .effect_stack_attack_mult
                .unwrap_or(d.effect_stack_attack_mult),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }

    fn add_attack_stack(&self, ctx: &mut GameCtx, caster: usize, target: usize) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }
        if !target_ref.is_champion() {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let stack_count = (0..caster_ref.buff_count())
            .filter(|&i| caster_ref.buff_at(i).name.as_str() == "radiant_spear_of_shojin_buff")
            .count();
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_duration_seconds * 60,
                    },
                    attack_mult: self.effect_stack_attack_mult,
                    name: ArrayString::try_from("radiant_spear_of_shojin_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
    }
}

impl ModItemInfo for RadiantSpearOfShojin {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_spear_of_shojin"
    }

    fn icon(&self) -> &str {
        "t10_4"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["spear_of_shojin".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        self.add_attack_stack(ctx, caster, target);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

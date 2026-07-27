use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::{has_buff, ticks};

#[derive(Clone, Debug)]
pub struct Morellonomicon {
    price: usize,
    hp: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_heal_reduce: usize,
    effect_duration_seconds: f64,
}

impl Default for Morellonomicon {
    fn default() -> Self {
        Self {
            price: 1300,
            hp: 200,
            magic_power: 120,
            skill_cooldown_mult: 10,
            effect_heal_reduce: 40,
            effect_duration_seconds: 2.0,
        }
    }
}

impl Morellonomicon {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_heal_reduce: cfg.effect_heal_reduce.unwrap_or(d.effect_heal_reduce),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for Morellonomicon {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "morellonomicon"
    }

    fn icon(&self) -> &str {
        "morellonomicon"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["oblivion_orb".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_morellonomicon".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        damage_type: DamageType,
    ) {
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };

        if damage_type != DamageType::AP {
            return;
        }

        let already_reduced = has_buff(&entity_ref, "40_percent_heal_cut");
        if !already_reduced {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: ticks(self.effect_duration_seconds),
                    },
                    heal_reduce: self.effect_heal_reduce,
                    name: ArrayString::try_from("40_percent_heal_cut").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::AP,
            ItemTag::CooltimeReduce,
            ItemTag::HealReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Clone, Debug)]
pub struct RadiantMorellonomicon {
    price: usize,
    hp: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_heal_reduce: usize,
    effect_duration_seconds: f64,
}

impl Default for RadiantMorellonomicon {
    fn default() -> Self {
        Self {
            price: 1850,
            hp: 350,
            magic_power: 190,
            skill_cooldown_mult: 10,
            effect_heal_reduce: 40,
            effect_duration_seconds: 2.0,
        }
    }
}

impl RadiantMorellonomicon {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_heal_reduce: cfg.effect_heal_reduce.unwrap_or(d.effect_heal_reduce),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for RadiantMorellonomicon {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_morellonomicon"
    }

    fn icon(&self) -> &str {
        "radiant_morellonomicon"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["morellonomicon".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        damage_type: DamageType,
    ) {
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };

        if damage_type != DamageType::AP {
            return;
        }

        let already_reduced = has_buff(&entity_ref, "40_percent_heal_cut");
        if !already_reduced {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: ticks(self.effect_duration_seconds),
                    },
                    heal_reduce: self.effect_heal_reduce,
                    name: ArrayString::try_from("40_percent_heal_cut").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::AP,
            ItemTag::CooltimeReduce,
            ItemTag::HealReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

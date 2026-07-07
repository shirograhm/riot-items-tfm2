use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct BlackfireTorch {
    price: usize,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_stack_magic_power: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
}

impl Default for BlackfireTorch {
    fn default() -> Self {
        Self {
            price: 1300,
            magic_power: 130,
            skill_cooldown_mult: 15,
            effect_stack_magic_power: 10,
            effect_max_stacks: 4,
            effect_duration_seconds: 4.0,
        }
    }
}

impl BlackfireTorch {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_stack_magic_power: cfg
                .effect_stack_magic_power
                .unwrap_or(d.effect_stack_magic_power),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for BlackfireTorch {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "blackfire_torch"
    }

    fn icon(&self) -> &str {
        "t7_6"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["staff_of_rapture".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_blackfire_torch".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, _target: usize) {
        let Some(entity_ref) = ctx.get_entity(caster) else {
            return;
        };
        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| entity_ref.buff_at(i).name.as_str() == "blackfire_torch_buff")
            .count();
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    magic_power: self.effect_stack_magic_power,
                    name: ArrayString::try_from("blackfire_torch_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Clone, Debug)]
pub struct RadiantBlackfireTorch {
    price: usize,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_stack_magic_power: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
}

impl Default for RadiantBlackfireTorch {
    fn default() -> Self {
        Self {
            price: 1900,
            magic_power: 175,
            skill_cooldown_mult: 25,
            effect_stack_magic_power: 30,
            effect_max_stacks: 4,
            effect_duration_seconds: 4.0,
        }
    }
}

impl RadiantBlackfireTorch {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_stack_magic_power: cfg
                .effect_stack_magic_power
                .unwrap_or(d.effect_stack_magic_power),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for RadiantBlackfireTorch {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_blackfire_torch"
    }

    fn icon(&self) -> &str {
        "t7_7"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["blackfire_torch".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, _target: usize) {
        let Some(entity_ref) = ctx.get_entity(caster) else {
            return;
        };
        let stack_count = (0..entity_ref.buff_count())
            .filter(|&i| entity_ref.buff_at(i).name.as_str() == "radiant_blackfire_torch_buff")
            .count();
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    magic_power: self.effect_stack_magic_power,
                    name: ArrayString::try_from("radiant_blackfire_torch_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

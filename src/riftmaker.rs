use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

#[derive(Clone, Debug)]
pub struct Riftmaker {
    price: usize,
    hp: i32,
    magic_power: i32,
    effect_caster_hp_percent_power: f64,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
}

impl Default for Riftmaker {
    fn default() -> Self {
        Self {
            price: 1300,
            hp: 400,
            magic_power: 75,
            effect_caster_hp_percent_power: 1.0,
            effect_max_stacks: 3,
            effect_duration_seconds: 5.0,
        }
    }
}

impl Riftmaker {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            effect_caster_hp_percent_power: cfg
                .effect_caster_hp_percent_power
                .unwrap_or(d.effect_caster_hp_percent_power),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for Riftmaker {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "riftmaker"
    }

    fn icon(&self) -> &str {
        "t7_2"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["haunting_guise".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_riftmaker".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            magic_power: self.magic_power,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, _target: usize) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let bonus_power = percent_of(
            caster_ref.hp().max,
            self.effect_caster_hp_percent_power as f64,
        );
        let stack_count = (0..caster_ref.buff_count())
            .filter(|&i| caster_ref.buff_at(i).name.as_str() == "riftmaker_magic_power_buff")
            .count();
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    magic_power: bonus_power as i32,
                    name: ArrayString::try_from("riftmaker_magic_power_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Clone, Debug)]
pub struct RadiantRiftmaker {
    price: usize,
    hp: i32,
    magic_power: i32,
    effect_caster_hp_percent_power: f64,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
}

impl Default for RadiantRiftmaker {
    fn default() -> Self {
        Self {
            price: 1900,
            hp: 600,
            magic_power: 150,
            effect_caster_hp_percent_power: 1.0,
            effect_max_stacks: 3,
            effect_duration_seconds: 5.0,
        }
    }
}

impl RadiantRiftmaker {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            effect_caster_hp_percent_power: cfg
                .effect_caster_hp_percent_power
                .unwrap_or(d.effect_caster_hp_percent_power),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for RadiantRiftmaker {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_riftmaker"
    }

    fn icon(&self) -> &str {
        "t7_3"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["riftmaker".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            magic_power: self.magic_power,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, _target: usize) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let bonus_power = percent_of(
            caster_ref.hp().max,
            self.effect_caster_hp_percent_power as f64,
        );
        let stack_count = (0..caster_ref.buff_count())
            .filter(|&i| {
                caster_ref.buff_at(i).name.as_str() == "radiant_riftmaker_magic_power_buff"
            })
            .count();
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                caster,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    magic_power: bonus_power as i32,
                    name: ArrayString::try_from("radiant_riftmaker_magic_power_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

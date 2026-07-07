use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct Terminus {
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    crit_chance: i32,
    effect_armor_pen_per_stack: usize,
    effect_magic_pen_per_stack: usize,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
    flip_flop: bool,
}

impl Default for Terminus {
    fn default() -> Self {
        Self {
            price: 1400,
            attack: 30,
            attack_speed_mult: 35,
            crit_chance: 20,
            effect_armor_pen_per_stack: 4,
            effect_magic_pen_per_stack: 4,
            effect_max_stacks: 4,
            effect_duration_seconds: 4.0,
            flip_flop: false,
        }
    }
}

impl Terminus {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            crit_chance: cfg.crit_chance.unwrap_or(d.crit_chance),
            effect_armor_pen_per_stack: cfg
                .effect_armor_pen_per_stack
                .unwrap_or(d.effect_armor_pen_per_stack),
            effect_magic_pen_per_stack: cfg
                .effect_magic_pen_per_stack
                .unwrap_or(d.effect_magic_pen_per_stack),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            flip_flop: false,
        }
    }
}

impl ModItemInfo for Terminus {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "terminus"
    }

    fn icon(&self) -> &str {
        "t8_7"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["twin_stormblade".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_terminus".to_string()]
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
        self.flip_flop = true;
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        _target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(caster_entity_ref) = ctx.get_entity(caster) else {
            return;
        };
        let defenses_stack_count = (0..caster_entity_ref.buff_count())
            .filter(|&i| caster_entity_ref.buff_at(i).name.as_str() == "terminus_armor_pen_buff")
            .count();
        let resistance_stack_count = (0..caster_entity_ref.buff_count())
            .filter(|&i| {
                caster_entity_ref.buff_at(i).name.as_str() == "terminus_magic_resistance_pen_buff"
            })
            .count();
        if self.flip_flop {
            if defenses_stack_count < self.effect_max_stacks {
                ctx.add_buff(
                    caster,
                    BuffState {
                        duration: BuffType::Time {
                            tick: (self.effect_duration_seconds * 60.0) as usize,
                        },
                        defence_penetration: self.effect_armor_pen_per_stack,
                        name: ArrayString::try_from("terminus_armor_pen_buff").unwrap(),
                        ..Default::default()
                    },
                );
            }
            self.flip_flop = false;
        } else {
            if resistance_stack_count < self.effect_max_stacks {
                ctx.add_buff(
                    caster,
                    BuffState {
                        duration: BuffType::Time {
                            tick: (self.effect_duration_seconds * 60.0) as usize,
                        },
                        magic_resistance_penetration: self.effect_magic_pen_per_stack,
                        name: ArrayString::try_from("terminus_magic_resistance_pen_buff").unwrap(),
                        ..Default::default()
                    },
                );
            }
            self.flip_flop = true;
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AS, ItemTag::DefensePenetration]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

#[derive(Clone, Debug)]
pub struct RadiantTerminus {
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    crit_chance: i32,
    effect_armor_pen_per_stack: usize,
    effect_magic_pen_per_stack: usize,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
    flip_flop: bool,
}

impl Default for RadiantTerminus {
    fn default() -> Self {
        Self {
            price: 2000,
            attack: 50,
            attack_speed_mult: 60,
            crit_chance: 25,
            effect_armor_pen_per_stack: 4,
            effect_magic_pen_per_stack: 4,
            effect_max_stacks: 4,
            effect_duration_seconds: 4.0,
            flip_flop: false,
        }
    }
}

impl RadiantTerminus {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            crit_chance: cfg.crit_chance.unwrap_or(d.crit_chance),
            effect_armor_pen_per_stack: cfg
                .effect_armor_pen_per_stack
                .unwrap_or(d.effect_armor_pen_per_stack),
            effect_magic_pen_per_stack: cfg
                .effect_magic_pen_per_stack
                .unwrap_or(d.effect_magic_pen_per_stack),
            effect_max_stacks: cfg.effect_max_stacks.unwrap_or(d.effect_max_stacks),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            flip_flop: false,
        }
    }
}

impl ModItemInfo for RadiantTerminus {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_terminus"
    }

    fn icon(&self) -> &str {
        "t8_8"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["terminus".to_string()]
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
        self.flip_flop = true;
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        _target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(caster_entity_ref) = ctx.get_entity(caster) else {
            return;
        };
        let defenses_stack_count = (0..caster_entity_ref.buff_count())
            .filter(|&i| {
                caster_entity_ref.buff_at(i).name.as_str() == "radiant_terminus_armor_pen_buff"
            })
            .count();
        let resistance_stack_count = (0..caster_entity_ref.buff_count())
            .filter(|&i| {
                caster_entity_ref.buff_at(i).name.as_str()
                    == "radiant_terminus_magic_resistance_pen_buff"
            })
            .count();
        if self.flip_flop {
            if defenses_stack_count < self.effect_max_stacks {
                ctx.add_buff(
                    caster,
                    BuffState {
                        duration: BuffType::Time {
                            tick: (self.effect_duration_seconds * 60.0) as usize,
                        },
                        defence_penetration: self.effect_armor_pen_per_stack,
                        name: ArrayString::try_from("radiant_terminus_armor_pen_buff").unwrap(),
                        ..Default::default()
                    },
                );
            }
            self.flip_flop = false;
        } else {
            if resistance_stack_count < self.effect_max_stacks {
                ctx.add_buff(
                    caster,
                    BuffState {
                        duration: BuffType::Time {
                            tick: (self.effect_duration_seconds * 60.0) as usize,
                        },
                        magic_resistance_penetration: self.effect_magic_pen_per_stack,
                        name: ArrayString::try_from("radiant_terminus_magic_resistance_pen_buff")
                            .unwrap(),
                        ..Default::default()
                    },
                );
            }
            self.flip_flop = true;
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AS, ItemTag::DefensePenetration]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

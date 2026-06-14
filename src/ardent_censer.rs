use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct ArdentCenser {
    price: usize,
    magic_power: i32,
    hp_regen: i32,
    skill_cooldown_mult: i32,
    move_speed_mult: i32,
    effect_force_buff: i32,
    effect_attack_speed_buff: i32,
    effect_duration_seconds: usize,
}

impl Default for ArdentCenser {
    fn default() -> Self {
        Self {
            price: 1100,
            magic_power: 100,
            hp_regen: 20,
            skill_cooldown_mult: 10,
            move_speed_mult: 10,
            effect_force_buff: 60,
            effect_attack_speed_buff: 20,
            effect_duration_seconds: 6,
        }
    }
}

impl ArdentCenser {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            hp_regen: cfg.hp_regen.unwrap_or(d.hp_regen),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            move_speed_mult: cfg.move_speed_mult.unwrap_or(d.move_speed_mult),
            effect_force_buff: cfg.effect_force_buff.unwrap_or(d.effect_force_buff),
            effect_attack_speed_buff: cfg
                .effect_attack_speed_buff
                .unwrap_or(d.effect_attack_speed_buff),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for ArdentCenser {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "ardent_censer"
    }

    fn icon(&self) -> &str {
        "t9_7"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["spirit_crystal".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_ardent_censer".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            hp_regen: self.hp_regen,
            skill_cooldown_mult: self.skill_cooldown_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_healed(&mut self, ctx: &mut GameCtx, caster: Option<usize>, target: usize, _heal: usize) {
        let Some(caster_id) = caster else {
            return;
        };
        let Some(caster_ref) = ctx.get_entity(caster_id) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };

        let is_attack_speed_buff_applied = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "ardent_censer_attack_speed_buff");
        let is_force_buff_applied = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "ardent_censer_force_buff");

        if caster_ref.team() == target_ref.team() {
            if !is_force_buff_applied {
                if target_ref.stat().magic_power > target_ref.stat().attack {
                    ctx.add_buff(
                        target,
                        BuffState {
                            duration: BuffType::Time {
                                tick: self.effect_duration_seconds * 60,
                            },
                            magic_power: self.effect_force_buff * 2,
                            name: ArrayString::try_from("ardent_censer_force_buff").unwrap(),
                            ..Default::default()
                        },
                    )
                } else {
                    ctx.add_buff(
                        target,
                        BuffState {
                            duration: BuffType::Time {
                                tick: self.effect_duration_seconds * 60,
                            },
                            attack: self.effect_force_buff,
                            name: ArrayString::try_from("ardent_censer_force_buff").unwrap(),
                            ..Default::default()
                        },
                    )
                }
            }

            if !is_attack_speed_buff_applied {
                ctx.add_buff(
                    target,
                    BuffState {
                        duration: BuffType::Time {
                            tick: self.effect_duration_seconds * 60,
                        },
                        attack_speed_mult: self.effect_attack_speed_buff,
                        name: ArrayString::try_from("ardent_censer_attack_speed_buff").unwrap(),
                        ..Default::default()
                    },
                );
            }
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::AP,
            ItemTag::HPRegen,
            ItemTag::CooltimeReduce,
            ItemTag::MoveSpeed,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Clone, Debug)]
pub struct RadiantArdentCenser {
    price: usize,
    magic_power: i32,
    hp_regen: i32,
    skill_cooldown_mult: i32,
    move_speed_mult: i32,
    effect_force_buff: i32,
    effect_attack_speed_buff: i32,
    effect_duration_seconds: usize,
}

impl Default for RadiantArdentCenser {
    fn default() -> Self {
        Self {
            price: 1650,
            magic_power: 150,
            hp_regen: 30,
            skill_cooldown_mult: 20,
            move_speed_mult: 20,
            effect_force_buff: 60,
            effect_attack_speed_buff: 20,
            effect_duration_seconds: 6,
        }
    }
}

impl RadiantArdentCenser {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            hp_regen: cfg.hp_regen.unwrap_or(d.hp_regen),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            move_speed_mult: cfg.move_speed_mult.unwrap_or(d.move_speed_mult),
            effect_force_buff: cfg.effect_force_buff.unwrap_or(d.effect_force_buff),
            effect_attack_speed_buff: cfg
                .effect_attack_speed_buff
                .unwrap_or(d.effect_attack_speed_buff),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for RadiantArdentCenser {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_ardent_censer"
    }

    fn icon(&self) -> &str {
        "t9_8"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["ardent_censer".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            hp_regen: self.hp_regen,
            skill_cooldown_mult: self.skill_cooldown_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_healed(&mut self, ctx: &mut GameCtx, caster: Option<usize>, target: usize, _heal: usize) {
        let Some(caster_id) = caster else {
            return;
        };
        let Some(caster_ref) = ctx.get_entity(caster_id) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };

        let is_attack_speed_buff_applied = (0..target_ref.buff_count()).any(|i| {
            target_ref.buff_at(i).name.as_str() == "radiant_ardent_censer_attack_speed_buff"
        });
        let is_force_buff_applied = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "radiant_ardent_censer_force_buff");

        if caster_ref.team() == target_ref.team() {
            if !is_force_buff_applied {
                if target_ref.stat().magic_power > target_ref.stat().attack {
                    ctx.add_buff(
                        target,
                        BuffState {
                            duration: BuffType::Time {
                                tick: self.effect_duration_seconds * 60,
                            },
                            magic_power: self.effect_force_buff * 2,
                            name: ArrayString::try_from("radiant_ardent_censer_force_buff")
                                .unwrap(),
                            ..Default::default()
                        },
                    )
                } else {
                    ctx.add_buff(
                        target,
                        BuffState {
                            duration: BuffType::Time {
                                tick: self.effect_duration_seconds * 60,
                            },
                            attack: self.effect_force_buff,
                            name: ArrayString::try_from("radiant_ardent_censer_force_buff")
                                .unwrap(),
                            ..Default::default()
                        },
                    )
                }
            }

            if !is_attack_speed_buff_applied {
                ctx.add_buff(
                    target,
                    BuffState {
                        duration: BuffType::Time {
                            tick: self.effect_duration_seconds * 60,
                        },
                        attack_speed_mult: self.effect_attack_speed_buff,
                        name: ArrayString::try_from("radiant_ardent_censer_attack_speed_buff")
                            .unwrap(),
                        ..Default::default()
                    },
                );
            }
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::AP,
            ItemTag::HPRegen,
            ItemTag::CooltimeReduce,
            ItemTag::MoveSpeed,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

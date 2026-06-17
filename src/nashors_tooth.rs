use arrayvec::ArrayString;
use mod_api::*;
use std::fmt::Write;

use crate::config::ItemConfig;
use crate::percent_of;

#[derive(Clone, Debug)]
pub struct NashorsTooth {
    price: usize,
    magic_power: i32,
    attack_speed_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ap_percent_damage: f64,
    on_hit_cooldown_seconds: f64,
}

impl Default for NashorsTooth {
    fn default() -> Self {
        Self {
            price: 1450,
            magic_power: 115,
            attack_speed_mult: 25,
            effect_bonus_flat_damage: 35,
            effect_ap_percent_damage: 3.0,
            on_hit_cooldown_seconds: 0.5,
        }
    }
}

impl NashorsTooth {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_ap_percent_damage: cfg
                .effect_ap_percent_damage
                .unwrap_or(d.effect_ap_percent_damage),
            on_hit_cooldown_seconds: cfg
                .on_hit_cooldown_seconds
                .unwrap_or(d.on_hit_cooldown_seconds),
        }
    }
}

impl ModItemInfo for NashorsTooth {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "nashors_tooth"
    }

    fn icon(&self) -> &str {
        "t8_3"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "needlessly_large_rod".to_string(),
            "wind_dagger".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_nashors_tooth".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            attack_speed_mult: self.attack_speed_mult,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.stat().magic_power, self.effect_ap_percent_damage);

        // CD String per champion
        let mut cooldown_str = ArrayString::<64>::new();
        write!(&mut cooldown_str, "nashors_tooth_cooldown_{}", target).unwrap();

        if self.on_hit_cooldown_seconds > 0.0 {
            let is_cooldown_ticking = (0..caster_ref.buff_count())
                .any(|i| caster_ref.buff_at(i).name.as_str() == cooldown_str.as_str());
            if !is_cooldown_ticking {
                ctx.add_buff(
                    caster,
                    BuffState {
                        duration: BuffType::Time {
                            tick: (self.on_hit_cooldown_seconds * 60.0).round() as usize,
                        },
                        name: cooldown_str,
                        ..Default::default()
                    },
                );
                ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
            }
        } else {
            ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::AS]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Clone, Debug)]
pub struct RadiantNashorsTooth {
    price: usize,
    magic_power: i32,
    attack_speed_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ap_percent_damage: f64,
    on_hit_cooldown_seconds: f64,
}

impl Default for RadiantNashorsTooth {
    fn default() -> Self {
        Self {
            price: 2050,
            magic_power: 180,
            attack_speed_mult: 40,
            effect_bonus_flat_damage: 50,
            effect_ap_percent_damage: 5.0,
            on_hit_cooldown_seconds: 0.5,
        }
    }
}

impl RadiantNashorsTooth {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_ap_percent_damage: cfg
                .effect_ap_percent_damage
                .unwrap_or(d.effect_ap_percent_damage),
            on_hit_cooldown_seconds: cfg
                .on_hit_cooldown_seconds
                .unwrap_or(d.on_hit_cooldown_seconds),
        }
    }
}

impl ModItemInfo for RadiantNashorsTooth {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_nashors_tooth"
    }

    fn icon(&self) -> &str {
        "t8_4"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["nashors_tooth".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            attack_speed_mult: self.attack_speed_mult,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.stat().magic_power, self.effect_ap_percent_damage);

        // CD String per champion
        let mut cooldown_str = ArrayString::<64>::new();
        write!(
            &mut cooldown_str,
            "radiant_nashors_tooth_cooldown_{}",
            target
        )
        .unwrap();

        if self.on_hit_cooldown_seconds > 0.0 {
            let is_cooldown_ticking = (0..caster_ref.buff_count())
                .any(|i| caster_ref.buff_at(i).name.as_str() == cooldown_str.as_str());
            if !is_cooldown_ticking {
                ctx.add_buff(
                    caster,
                    BuffState {
                        duration: BuffType::Time {
                            tick: (self.on_hit_cooldown_seconds * 60.0).round() as usize,
                        },
                        name: cooldown_str,
                        ..Default::default()
                    },
                );
                ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
            }
        } else {
            ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::AS]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

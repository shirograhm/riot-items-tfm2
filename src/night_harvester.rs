use arrayvec::ArrayString;
use mod_api::*;
use std::fmt::Write;

use crate::config::ItemConfig;
use crate::percent_of;

// Soulrend: dealing ability damage to an enemy champion deals bonus magic damage
// (flat + % of the wielder's Ability Power) and grants the wielder a burst of
// movement speed, gated by a per-target cooldown (see nashors_tooth.rs for the
// same `on_skill_hit` + per-target cooldown-buff pattern).

#[derive(Clone, Debug)]
pub struct NightHarvester {
    price: usize,
    hp: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ap_percent_damage: f64,
    effect_move_speed_mult: i32,
    effect_duration_seconds: usize,
    effect_cooldown_seconds: usize,
}

impl Default for NightHarvester {
    fn default() -> Self {
        Self {
            price: 1400,
            hp: 300,
            magic_power: 100,
            skill_cooldown_mult: 10,
            effect_bonus_flat_damage: 160,
            effect_ap_percent_damage: 40.0,
            effect_move_speed_mult: 40,
            effect_duration_seconds: 2,
            effect_cooldown_seconds: 10,
        }
    }
}

impl NightHarvester {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_ap_percent_damage: cfg
                .effect_ap_percent_damage
                .unwrap_or(d.effect_ap_percent_damage),
            effect_move_speed_mult: cfg
                .effect_move_speed_mult
                .unwrap_or(d.effect_move_speed_mult),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
        }
    }
}

impl ModItemInfo for NightHarvester {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "night_harvester"
    }

    fn icon(&self) -> &str {
        "t11_2"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "ring_of_reincarnation".to_string(),
            "spirit_crystal".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_night_harvester".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }

        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.stat().magic_power, self.effect_ap_percent_damage);

        // Per-target cooldown, tracked as a buff on the caster keyed by target.
        let mut cooldown_str = ArrayString::<64>::new();
        write!(&mut cooldown_str, "night_harvester_cooldown_{}", target).unwrap();
        let is_cooldown_ticking = (0..caster_ref.buff_count())
            .any(|i| caster_ref.buff_at(i).name.as_str() == cooldown_str.as_str());
        if is_cooldown_ticking {
            return;
        }

        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time {
                    tick: self.effect_cooldown_seconds * 60,
                },
                name: cooldown_str,
                ..Default::default()
            },
        );
        ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time {
                    tick: self.effect_duration_seconds * 60,
                },
                move_speed_mult: self.effect_move_speed_mult,
                name: ArrayString::try_from("night_harvester_soulrend").unwrap(),
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Clone, Debug)]
pub struct RadiantNightHarvester {
    price: usize,
    hp: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ap_percent_damage: f64,
    effect_move_speed_mult: i32,
    effect_duration_seconds: usize,
    effect_cooldown_seconds: usize,
}

impl Default for RadiantNightHarvester {
    fn default() -> Self {
        Self {
            price: 2000,
            hp: 500,
            magic_power: 160,
            skill_cooldown_mult: 10,
            effect_bonus_flat_damage: 160,
            effect_ap_percent_damage: 40.0,
            effect_move_speed_mult: 40,
            effect_duration_seconds: 2,
            effect_cooldown_seconds: 10,
        }
    }
}

impl RadiantNightHarvester {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_ap_percent_damage: cfg
                .effect_ap_percent_damage
                .unwrap_or(d.effect_ap_percent_damage),
            effect_move_speed_mult: cfg
                .effect_move_speed_mult
                .unwrap_or(d.effect_move_speed_mult),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
        }
    }
}

impl ModItemInfo for RadiantNightHarvester {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_night_harvester"
    }

    fn icon(&self) -> &str {
        "t11_3"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["night_harvester".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, target: usize) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }

        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_ref.stat().magic_power, self.effect_ap_percent_damage);

        // Per-target cooldown, tracked as a buff on the caster keyed by target.
        let mut cooldown_str = ArrayString::<64>::new();
        write!(&mut cooldown_str, "radiant_night_harvester_cooldown_{}", target).unwrap();
        let is_cooldown_ticking = (0..caster_ref.buff_count())
            .any(|i| caster_ref.buff_at(i).name.as_str() == cooldown_str.as_str());
        if is_cooldown_ticking {
            return;
        }

        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time {
                    tick: self.effect_cooldown_seconds * 60,
                },
                name: cooldown_str,
                ..Default::default()
            },
        );
        ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time {
                    tick: self.effect_duration_seconds * 60,
                },
                move_speed_mult: self.effect_move_speed_mult,
                name: ArrayString::try_from("radiant_night_harvester_soulrend").unwrap(),
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

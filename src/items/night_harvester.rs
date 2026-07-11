use arrayvec::ArrayString;
use mod_api::*;

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
    effect_duration_seconds: f64,
    effect_cooldown_seconds: f64,
}

impl Default for NightHarvester {
    fn default() -> Self {
        Self {
            price: 1400,
            hp: 300,
            magic_power: 100,
            skill_cooldown_mult: 10,
            effect_bonus_flat_damage: 150,
            effect_ap_percent_damage: 30.0,
            effect_move_speed_mult: 40,
            effect_duration_seconds: 2.0,
            effect_cooldown_seconds: 45.0,
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
        "night_harvester"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "staff_of_rapture".to_string(),
            "ring_of_reincarnation".to_string(),
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

        let is_cooldown_ticking = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "night_harvester_cooldown");
        if is_cooldown_ticking {
            return;
        }

        ctx.add_buff(
            target,
            BuffState {
                duration: BuffType::Time {
                    tick: (self.effect_cooldown_seconds * 60.0) as usize,
                },
                name: ArrayString::try_from("night_harvester_cooldown").unwrap(),
                ..Default::default()
            },
        );
        ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time {
                    tick: (self.effect_duration_seconds * 60.0) as usize,
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
    effect_duration_seconds: f64,
    effect_cooldown_seconds: f64,
}

impl Default for RadiantNightHarvester {
    fn default() -> Self {
        Self {
            price: 2000,
            hp: 500,
            magic_power: 160,
            skill_cooldown_mult: 10,
            effect_bonus_flat_damage: 150,
            effect_ap_percent_damage: 30.0,
            effect_move_speed_mult: 40,
            effect_duration_seconds: 2.0,
            effect_cooldown_seconds: 45.0,
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
        "radiant_night_harvester"
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

        let is_cooldown_ticking = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "radiant_night_harvester_cooldown");
        if is_cooldown_ticking {
            return;
        }

        ctx.add_buff(
            target,
            BuffState {
                duration: BuffType::Time {
                    tick: (self.effect_cooldown_seconds * 60.0) as usize,
                },
                name: ArrayString::try_from("radiant_night_harvester_cooldown").unwrap(),
                ..Default::default()
            },
        );
        ctx.deal_damage(caster, target, 0, bonus_damage, AttackType::Item);
        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time {
                    tick: (self.effect_duration_seconds * 60.0) as usize,
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

use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct RylaisCrystalScepter {
    price: usize,
    hp: i32,
    magic_power: i32,
    effect_slow_amount: i32,
    effect_duration_seconds: f64,
}

impl Default for RylaisCrystalScepter {
    fn default() -> Self {
        Self {
            price: 1350,
            hp: 250,
            magic_power: 125,
            effect_slow_amount: 15,
            effect_duration_seconds: 2.0,
        }
    }
}

impl RylaisCrystalScepter {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            effect_slow_amount: cfg.effect_slow_amount.unwrap_or(d.effect_slow_amount),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for RylaisCrystalScepter {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "rylais_crystal_scepter"
    }

    fn icon(&self) -> &str {
        "rylais_crystal_scepter"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "hardened_heart".to_string(),
            "needlessly_large_rod".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_rylais_crystal_scepter".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            magic_power: self.magic_power,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, _caster: usize, target: usize) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let already_slowed = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "rylais_crystal_scepter_slow");
        if !already_slowed {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    move_speed_mult: -self.effect_slow_amount,
                    name: ArrayString::try_from("rylais_crystal_scepter_slow").unwrap(),
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
pub struct RadiantRylaisCrystalScepter {
    price: usize,
    hp: i32,
    magic_power: i32,
    effect_slow_amount: i32,
    effect_duration_seconds: f64,
}

impl Default for RadiantRylaisCrystalScepter {
    fn default() -> Self {
        Self {
            price: 1900,
            hp: 400,
            magic_power: 200,
            effect_slow_amount: 15,
            effect_duration_seconds: 2.0,
        }
    }
}

impl RadiantRylaisCrystalScepter {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            effect_slow_amount: cfg.effect_slow_amount.unwrap_or(d.effect_slow_amount),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for RadiantRylaisCrystalScepter {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_rylais_crystal_scepter"
    }

    fn icon(&self) -> &str {
        "radiant_rylais_crystal_scepter"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["rylais_crystal_scepter".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            magic_power: self.magic_power,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, _caster: usize, target: usize) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let already_slowed = (0..target_ref.buff_count())
            .any(|i| target_ref.buff_at(i).name.as_str() == "rylais_crystal_scepter_slow");
        if !already_slowed {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: (self.effect_duration_seconds * 60.0) as usize,
                    },
                    move_speed_mult: -self.effect_slow_amount,
                    name: ArrayString::try_from("rylais_crystal_scepter_slow").unwrap(),
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

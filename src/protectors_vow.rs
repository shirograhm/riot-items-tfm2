use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;
use crate::{BUFF_REFRESH_DURATION_TICKS, BUFF_REFRESH_PERIOD_TICKS};

#[derive(Clone, Debug)]
pub struct ProtectorsVow {
    price: usize,
    hp: i32,
    defence: i32,
    effect_bonus_flat_hp: i32,
    effect_caster_defence_percent_hp: f64,
    refresh_cooldown: usize,
}

impl Default for ProtectorsVow {
    fn default() -> Self {
        Self {
            price: 1300,
            hp: 350,
            defence: 50,
            effect_bonus_flat_hp: 50,
            effect_caster_defence_percent_hp: 80.0,
            refresh_cooldown: 0,
        }
    }
}

impl ProtectorsVow {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            defence: cfg.defence.unwrap_or(d.defence),
            effect_bonus_flat_hp: cfg.effect_bonus_flat_hp.unwrap_or(d.effect_bonus_flat_hp),
            effect_caster_defence_percent_hp: cfg
                .effect_caster_defence_percent_hp
                .unwrap_or(d.effect_caster_defence_percent_hp),
            refresh_cooldown: 0,
        }
    }

    fn apply_awe(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };

        let target = self.effect_bonus_flat_hp
            + percent_of(
                champion_ref.stat().defence,
                self.effect_caster_defence_percent_hp,
            ) as i32;
        if target <= 0 {
            return;
        }

        let entity_id = champion_ref.id();
        ctx.add_buff(
            entity_id,
            BuffState {
                name: ArrayString::try_from("protectors_vow_awe").unwrap(),
                duration: BuffType::Time {
                    tick: BUFF_REFRESH_DURATION_TICKS,
                },
                hp: target,
                ..Default::default()
            },
        );
        self.refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
    }
}

impl ModItemInfo for ProtectorsVow {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "protectors_vow"
    }

    fn icon(&self) -> &str {
        "protectors_vow"
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
            "gatekeepers_armor".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_protectors_vow".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            defence: self.defence,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.refresh_cooldown = 0;
        self.apply_awe(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_awe(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Defense
    }
}

#[derive(Clone, Debug)]
pub struct RadiantProtectorsVow {
    price: usize,
    hp: i32,
    defence: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_hp: i32,
    effect_caster_defence_percent_hp: f64,
    refresh_cooldown: usize,
}

impl Default for RadiantProtectorsVow {
    fn default() -> Self {
        Self {
            price: 1800,
            hp: 550,
            defence: 75,
            skill_cooldown_mult: 15,
            effect_bonus_flat_hp: 50,
            effect_caster_defence_percent_hp: 80.0,
            refresh_cooldown: 0,
        }
    }
}

impl RadiantProtectorsVow {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            defence: cfg.defence.unwrap_or(d.defence),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            effect_bonus_flat_hp: cfg.effect_bonus_flat_hp.unwrap_or(d.effect_bonus_flat_hp),
            effect_caster_defence_percent_hp: cfg
                .effect_caster_defence_percent_hp
                .unwrap_or(d.effect_caster_defence_percent_hp),
            refresh_cooldown: 0,
        }
    }

    fn apply_awe(&mut self, ctx: &mut GameCtx, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };

        let target = self.effect_bonus_flat_hp
            + percent_of(
                champion_ref.stat().defence,
                self.effect_caster_defence_percent_hp,
            ) as i32;
        if target <= 0 {
            return;
        }

        let entity_id = champion_ref.id();
        ctx.add_buff(
            entity_id,
            BuffState {
                name: ArrayString::try_from("radiant_protectors_vow_awe").unwrap(),
                duration: BuffType::Time {
                    tick: BUFF_REFRESH_DURATION_TICKS,
                },
                hp: target,
                ..Default::default()
            },
        );
        self.refresh_cooldown = BUFF_REFRESH_PERIOD_TICKS;
    }
}

impl ModItemInfo for RadiantProtectorsVow {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_protectors_vow"
    }

    fn icon(&self) -> &str {
        "radiant_protectors_vow"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["protectors_vow".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            defence: self.defence,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.refresh_cooldown = 0;
        self.apply_awe(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_awe(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Defense
    }
}

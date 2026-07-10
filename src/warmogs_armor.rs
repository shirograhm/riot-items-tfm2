use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;
use crate::{BUFF_REFRESH_DURATION_TICKS, BUFF_REFRESH_PERIOD_TICKS};

const REGEN_PERIOD_TICKS: usize = 60;

#[derive(Clone, Debug)]
pub struct WarmogsArmor {
    price: usize,
    hp: i32,
    hp_regen: i32,
    effect_caster_hp_percent_heal: f64,
    effect_move_speed_mult: i32,
    effect_duration_seconds: f64,
    regen_cooldown: usize,
    move_speed_cooldown: usize,
}

impl Default for WarmogsArmor {
    fn default() -> Self {
        Self {
            price: 1450,
            hp: 600,
            hp_regen: 6,
            effect_caster_hp_percent_heal: 3.0,
            effect_move_speed_mult: 4,
            effect_duration_seconds: 6.0,
            regen_cooldown: 0,
            move_speed_cooldown: 0,
        }
    }
}

impl WarmogsArmor {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            hp_regen: cfg.hp_regen.unwrap_or(d.hp_regen),
            effect_caster_hp_percent_heal: cfg
                .effect_caster_hp_percent_heal
                .unwrap_or(d.effect_caster_hp_percent_heal),
            effect_move_speed_mult: cfg
                .effect_move_speed_mult
                .unwrap_or(d.effect_move_speed_mult),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            regen_cooldown: d.regen_cooldown,
            move_speed_cooldown: d.move_speed_cooldown,
        }
    }

    fn apply_passive(&mut self, ctx: &mut GameCtx, player: usize) {
        let (entity, max_hp, recently_damaged) = {
            let Some(player_ref) = ctx.get_player(player) else {
                return;
            };
            let Some(champion_ref) = player_ref.champion() else {
                return;
            };
            let recently_damaged = (0..champion_ref.buff_count())
                .any(|i| champion_ref.buff_at(i).name.as_str() == "warmogs_armor_recently_damaged");
            (champion_ref.id(), champion_ref.hp().max, recently_damaged)
        };

        // Warmog's Heart is suppressed while the holder has taken damage recently.
        if recently_damaged {
            return;
        }

        // Regenerate a share of maximum health every second.
        if self.regen_cooldown == 0 {
            let heal = percent_of(max_hp, self.effect_caster_hp_percent_heal) as i32;
            if heal > 0 {
                ctx.add_buff(
                    entity,
                    BuffState {
                        duration: BuffType::Time { tick: 60 },
                        hp_regen: heal,
                        ..Default::default()
                    },
                );
            }
            self.regen_cooldown = REGEN_PERIOD_TICKS;
        } else {
            self.regen_cooldown -= 1;
        }

        // ...and grant movement speed as a fixed-duration buff.
        if self.move_speed_cooldown == 0 {
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: BUFF_REFRESH_DURATION_TICKS,
                    },
                    move_speed_mult: self.effect_move_speed_mult,
                    name: ArrayString::try_from("warmogs_armor_move_speed").unwrap(),
                    ..Default::default()
                },
            );
            self.move_speed_cooldown = BUFF_REFRESH_PERIOD_TICKS;
        } else {
            self.move_speed_cooldown -= 1;
        }
    }
}

impl ModItemInfo for WarmogsArmor {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "warmogs_armor"
    }

    fn icon(&self) -> &str {
        "warmogs_armor"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["ring_of_reincarnation".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_warmogs_armor".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            hp_regen: self.hp_regen,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.regen_cooldown = 0;
        self.move_speed_cooldown = 0;
        self.apply_passive(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_passive(ctx, player);
    }

    fn on_damaged(
        &mut self,
        ctx: &mut GameCtx,
        _player: usize,
        entity: usize,
        attacker: usize,
        _damage: usize,
    ) {
        if attacker == entity {
            return;
        }
        ctx.add_buff(
            entity,
            BuffState {
                duration: BuffType::Time {
                    tick: (self.effect_duration_seconds * 60.0) as usize,
                },
                name: ArrayString::try_from("warmogs_armor_recently_damaged").unwrap(),
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::HPRegen, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Clone, Debug)]
pub struct RadiantWarmogsArmor {
    price: usize,
    hp: i32,
    hp_regen: i32,
    effect_caster_hp_percent_heal: f64,
    effect_move_speed_mult: i32,
    effect_duration_seconds: f64,
    regen_cooldown: usize,
    move_speed_cooldown: usize,
}

impl Default for RadiantWarmogsArmor {
    fn default() -> Self {
        Self {
            price: 2100,
            hp: 1000,
            hp_regen: 10,
            effect_caster_hp_percent_heal: 3.0,
            effect_move_speed_mult: 4,
            effect_duration_seconds: 6.0,
            regen_cooldown: 0,
            move_speed_cooldown: 0,
        }
    }
}

impl RadiantWarmogsArmor {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            hp_regen: cfg.hp_regen.unwrap_or(d.hp_regen),
            effect_caster_hp_percent_heal: cfg
                .effect_caster_hp_percent_heal
                .unwrap_or(d.effect_caster_hp_percent_heal),
            effect_move_speed_mult: cfg
                .effect_move_speed_mult
                .unwrap_or(d.effect_move_speed_mult),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            regen_cooldown: d.regen_cooldown,
            move_speed_cooldown: d.move_speed_cooldown,
        }
    }

    fn apply_passive(&mut self, ctx: &mut GameCtx, player: usize) {
        let (entity, max_hp, recently_damaged) = {
            let Some(player_ref) = ctx.get_player(player) else {
                return;
            };
            let Some(champion_ref) = player_ref.champion() else {
                return;
            };
            let recently_damaged = (0..champion_ref.buff_count()).any(|i| {
                champion_ref.buff_at(i).name.as_str() == "radiant_warmogs_armor_recently_damaged"
            });
            (champion_ref.id(), champion_ref.hp().max, recently_damaged)
        };

        // Warmog's Heart is suppressed while the holder has taken damage recently.
        if recently_damaged {
            return;
        }

        // Regenerate a share of maximum health every second.
        if self.regen_cooldown == 0 {
            let heal = percent_of(max_hp, self.effect_caster_hp_percent_heal) as i32;
            if heal > 0 {
                ctx.add_buff(
                    entity,
                    BuffState {
                        duration: BuffType::Time { tick: 60 },
                        hp_regen: heal,
                        ..Default::default()
                    },
                );
            }
            self.regen_cooldown = REGEN_PERIOD_TICKS;
        } else {
            self.regen_cooldown -= 1;
        }

        // ...and grant movement speed as a fixed-duration buff.
        if self.move_speed_cooldown == 0 {
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: BUFF_REFRESH_DURATION_TICKS,
                    },
                    move_speed_mult: self.effect_move_speed_mult,
                    name: ArrayString::try_from("radiant_warmogs_armor_move_speed").unwrap(),
                    ..Default::default()
                },
            );
            self.move_speed_cooldown = BUFF_REFRESH_PERIOD_TICKS;
        } else {
            self.move_speed_cooldown -= 1;
        }
    }
}

impl ModItemInfo for RadiantWarmogsArmor {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_warmogs_armor"
    }

    fn icon(&self) -> &str {
        "radiant_warmogs_armor"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["warmogs_armor".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            hp_regen: self.hp_regen,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.regen_cooldown = 0;
        self.apply_passive(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_passive(ctx, player);
    }

    fn on_damaged(
        &mut self,
        ctx: &mut GameCtx,
        _player: usize,
        entity: usize,
        attacker: usize,
        _damage: usize,
    ) {
        if attacker == entity {
            return;
        }
        ctx.add_buff(
            entity,
            BuffState {
                duration: BuffType::Time {
                    tick: (self.effect_duration_seconds * 60.0) as usize,
                },
                name: ArrayString::try_from("radiant_warmogs_armor_recently_damaged").unwrap(),
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::HPRegen, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

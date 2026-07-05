use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

const REGEN_PERIOD_TICKS: usize = 30;
const MOVE_SPEED_REFRESH_TICKS: usize = 30;

#[derive(Clone, Debug)]
pub struct WarmogsArmor {
    price: usize,
    hp: i32,
    effect_caster_hp_percent_heal: f64,
    effect_move_speed_mult: i32,
    effect_duration_seconds: usize,
    regen_cooldown: usize,
    ticks_since_damage: usize,
}

impl Default for WarmogsArmor {
    fn default() -> Self {
        Self {
            price: 1400,
            hp: 550,
            effect_caster_hp_percent_heal: 2.0,
            effect_move_speed_mult: 4,
            effect_duration_seconds: 6,
            regen_cooldown: 0,
            ticks_since_damage: 0,
        }
    }
}

impl WarmogsArmor {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
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
            ticks_since_damage: d.ticks_since_damage,
        }
    }

    fn apply_passive(&mut self, ctx: &mut GameCtx, player: usize) {
        let Some((entity, max_hp, has_move_speed)) = ({
            let Some(player_ref) = ctx.get_player(player) else {
                return;
            };
            let Some(entity_ref) = player_ref.champion() else {
                return;
            };
            let has_move_speed = (0..entity_ref.buff_count())
                .any(|i| entity_ref.buff_at(i).name.as_str() == "warmogs_armor_move_speed");
            Some((entity_ref.id(), entity_ref.hp().max, has_move_speed))
        }) else {
            return;
        };

        // Warmog's Heart: regenerate a share of maximum health every half-second.
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

        // ...and grant movement speed while no damage has been taken recently.
        if self.ticks_since_damage >= self.effect_duration_seconds * 60 && !has_move_speed {
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: MOVE_SPEED_REFRESH_TICKS,
                    },
                    move_speed_mult: self.effect_move_speed_mult,
                    name: ArrayString::try_from("warmogs_armor_move_speed").unwrap(),
                    ..Default::default()
                },
            );
        }

        self.ticks_since_damage = self.ticks_since_damage.saturating_add(1);
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
        "t11_6"
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
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.regen_cooldown = 0;
        self.ticks_since_damage = 0;
        self.apply_passive(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_passive(ctx, player);
    }

    fn on_damaged(
        &mut self,
        _ctx: &mut GameCtx,
        _player: usize,
        entity: usize,
        attacker: usize,
        _damage: usize,
    ) {
        if attacker == entity {
            return;
        }
        self.ticks_since_damage = 0;
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Clone, Debug)]
pub struct RadiantWarmogsArmor {
    price: usize,
    hp: i32,
    effect_caster_hp_percent_heal: f64,
    effect_move_speed_mult: i32,
    effect_duration_seconds: usize,
    regen_cooldown: usize,
    ticks_since_damage: usize,
}

impl Default for RadiantWarmogsArmor {
    fn default() -> Self {
        Self {
            price: 2000,
            hp: 900,
            effect_caster_hp_percent_heal: 2.0,
            effect_move_speed_mult: 4,
            effect_duration_seconds: 6,
            regen_cooldown: 0,
            ticks_since_damage: 0,
        }
    }
}

impl RadiantWarmogsArmor {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
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
            ticks_since_damage: d.ticks_since_damage,
        }
    }

    fn apply_passive(&mut self, ctx: &mut GameCtx, player: usize) {
        let Some((entity, max_hp, has_move_speed)) = ({
            let Some(player_ref) = ctx.get_player(player) else {
                return;
            };
            let Some(entity_ref) = player_ref.champion() else {
                return;
            };
            let has_move_speed = (0..entity_ref.buff_count())
                .any(|i| entity_ref.buff_at(i).name.as_str() == "radiant_warmogs_armor_move_speed");
            Some((entity_ref.id(), entity_ref.hp().max, has_move_speed))
        }) else {
            return;
        };

        // Warmog's Heart: regenerate a share of maximum health every half-second.
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

        // ...and grant movement speed while no damage has been taken recently.
        if self.ticks_since_damage >= self.effect_duration_seconds * 60 && !has_move_speed {
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: MOVE_SPEED_REFRESH_TICKS,
                    },
                    move_speed_mult: self.effect_move_speed_mult,
                    name: ArrayString::try_from("radiant_warmogs_armor_move_speed").unwrap(),
                    ..Default::default()
                },
            );
        }

        self.ticks_since_damage = self.ticks_since_damage.saturating_add(1);
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
        "t11_7"
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
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut GameCtx, player: usize) {
        self.regen_cooldown = 0;
        self.ticks_since_damage = 0;
        self.apply_passive(ctx, player);
    }

    fn update(&mut self, ctx: &mut GameCtx, _rng_seed: u64, player: usize) {
        self.apply_passive(ctx, player);
    }

    fn on_damaged(
        &mut self,
        _ctx: &mut GameCtx,
        _player: usize,
        entity: usize,
        attacker: usize,
        _damage: usize,
    ) {
        if attacker == entity {
            return;
        }
        self.ticks_since_damage = 0;
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

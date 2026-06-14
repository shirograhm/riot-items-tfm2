use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;
use crate::percent_of_i32;

#[derive(Clone, Debug)]
pub struct ProtoplasmHarness {
    price: usize,
    hp: i32,
    skill_cooldown_mult: i32,
    move_speed_mult: i32,
    effect_bonus_flat_hp: i32,
    effect_hp_percent_boost: f64,
    effect_hp_percent_threshold: f64,
    effect_duration_seconds: usize,
    effect_cooldown_seconds: usize,
}

impl Default for ProtoplasmHarness {
    fn default() -> Self {
        Self {
            price: 1000,
            hp: 400,
            skill_cooldown_mult: 20,
            move_speed_mult: 5,
            effect_bonus_flat_hp: 300,
            effect_hp_percent_boost: 25.0,
            effect_hp_percent_threshold: 40.0,
            effect_duration_seconds: 6,
            effect_cooldown_seconds: 30,
        }
    }
}

impl ProtoplasmHarness {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            move_speed_mult: cfg.move_speed_mult.unwrap_or(d.move_speed_mult),
            effect_bonus_flat_hp: cfg.effect_bonus_flat_hp.unwrap_or(d.effect_bonus_flat_hp),
            effect_hp_percent_boost: cfg
                .effect_hp_percent_boost
                .unwrap_or(d.effect_hp_percent_boost),
            effect_hp_percent_threshold: cfg
                .effect_hp_percent_threshold
                .unwrap_or(d.effect_hp_percent_threshold),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
        }
    }
}

impl ModItemInfo for ProtoplasmHarness {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "protoplasm_harness"
    }

    fn icon(&self) -> &str {
        "t9_3"
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
        vec!["radiant_protoplasm_harness".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            skill_cooldown_mult: self.skill_cooldown_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::CooltimeReduce, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }

    fn on_damaged(
        &mut self,
        ctx: &mut GameCtx,
        _player: usize,
        entity: usize,
        _attacker: usize,
        _damage: usize,
    ) {
        let Some(entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        let has_harness_buff: bool = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "protoplasm_harness_buff");
        let has_cooldown_buff: bool = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "protoplasm_harness_cooldown_buff");
        let hp_threshold = percent_of(entity_ref.hp().max, self.effect_hp_percent_threshold);

        if !has_harness_buff && !has_cooldown_buff && (entity_ref.hp().current <= hp_threshold) {
            let bonus_max_hp = self.effect_bonus_flat_hp
                + percent_of(entity_ref.hp().max, self.effect_hp_percent_boost) as i32;
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_duration_seconds * 60,
                    },
                    hp: bonus_max_hp,
                    name: ArrayString::try_from("protoplasm_harness_buff").unwrap(),
                    ..Default::default()
                },
            );
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time { tick: 60 },
                    hp_regen: percent_of_i32(bonus_max_hp, 50.0),
                    ..Default::default()
                },
            );
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_cooldown_seconds * 60,
                    },
                    name: ArrayString::try_from("protoplasm_harness_cooldown_buff").unwrap(),
                    ..Default::default()
                },
            );
        }
    }
}

#[derive(Clone, Debug)]
pub struct RadiantProtoplasmHarness {
    price: usize,
    hp: i32,
    skill_cooldown_mult: i32,
    move_speed_mult: i32,
    effect_bonus_flat_hp: i32,
    effect_hp_percent_boost: f64,
    effect_hp_percent_threshold: f64,
    effect_duration_seconds: usize,
    effect_cooldown_seconds: usize,
}

impl Default for RadiantProtoplasmHarness {
    fn default() -> Self {
        Self {
            price: 1600,
            hp: 800,
            skill_cooldown_mult: 20,
            move_speed_mult: 5,
            effect_bonus_flat_hp: 600,
            effect_hp_percent_boost: 25.0,
            effect_hp_percent_threshold: 40.0,
            effect_duration_seconds: 6,
            effect_cooldown_seconds: 30,
        }
    }
}

impl RadiantProtoplasmHarness {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            move_speed_mult: cfg.move_speed_mult.unwrap_or(d.move_speed_mult),
            effect_bonus_flat_hp: cfg.effect_bonus_flat_hp.unwrap_or(d.effect_bonus_flat_hp),
            effect_hp_percent_boost: cfg
                .effect_hp_percent_boost
                .unwrap_or(d.effect_hp_percent_boost),
            effect_hp_percent_threshold: cfg
                .effect_hp_percent_threshold
                .unwrap_or(d.effect_hp_percent_threshold),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
            effect_cooldown_seconds: cfg
                .effect_cooldown_seconds
                .unwrap_or(d.effect_cooldown_seconds),
        }
    }
}

impl ModItemInfo for RadiantProtoplasmHarness {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_protoplasm_harness"
    }

    fn icon(&self) -> &str {
        "t9_4"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["protoplasm_harness".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            skill_cooldown_mult: self.skill_cooldown_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::CooltimeReduce, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }

    fn on_damaged(
        &mut self,
        ctx: &mut GameCtx,
        _player: usize,
        entity: usize,
        _attacker: usize,
        _damage: usize,
    ) {
        let Some(entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        let has_harness_buff: bool = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "radiant_protoplasm_harness_buff");
        let has_cooldown_buff: bool = (0..entity_ref.buff_count()).any(|i| {
            entity_ref.buff_at(i).name.as_str() == "radiant_protoplasm_harness_cooldown_buff"
        });
        let hp_threshold = percent_of(entity_ref.hp().max, self.effect_hp_percent_threshold);

        if !has_harness_buff && !has_cooldown_buff && (entity_ref.hp().current <= hp_threshold) {
            let bonus_max_hp = self.effect_bonus_flat_hp
                + percent_of(entity_ref.hp().max, self.effect_hp_percent_boost) as i32;
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_duration_seconds * 60,
                    },
                    hp: bonus_max_hp,
                    name: ArrayString::try_from("radiant_protoplasm_harness_buff").unwrap(),
                    ..Default::default()
                },
            );
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time { tick: 60 },
                    hp_regen: percent_of_i32(bonus_max_hp, 50.0),
                    ..Default::default()
                },
            );
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_cooldown_seconds * 60,
                    },
                    name: ArrayString::try_from("radiant_protoplasm_harness_cooldown_buff")
                        .unwrap(),
                    ..Default::default()
                },
            );
        }
    }
}

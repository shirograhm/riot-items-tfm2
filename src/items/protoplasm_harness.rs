use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, buff_name, has_buff, percent_of, percent_of_i32, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct ProtoplasmHarness {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant
    // items keep independent stacks.
    buff_buff: &'static str,
    cooldown_buff_buff: &'static str,
    price: usize,
    hp: i32,
    skill_cooldown_mult: i32,
    move_speed_mult: i32,
    effect_bonus_flat_hp: i32,
    effect_hp_percent_boost: f64,
    effect_hp_percent_threshold: f64,
    effect_duration_seconds: f64,
    effect_cooldown_seconds: f64,
}

impl ProtoplasmHarness {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "protoplasm_harness",
                &["ring_of_reincarnation"],
                &["radiant_protoplasm_harness"],
            ),
            buff_buff: "protoplasm_harness_buff",
            cooldown_buff_buff: "protoplasm_harness_cooldown_buff",
            price: 1000,
            hp: 350,
            skill_cooldown_mult: 15,
            move_speed_mult: 5,
            effect_bonus_flat_hp: 300,
            effect_hp_percent_boost: 25.0,
            effect_hp_percent_threshold: 40.0,
            effect_duration_seconds: 6.0,
            effect_cooldown_seconds: 30.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_protoplasm_harness", &["protoplasm_harness"]),
            buff_buff: "radiant_protoplasm_harness_buff",
            cooldown_buff_buff: "radiant_protoplasm_harness_cooldown_buff",
            price: 1600,
            hp: 700,
            skill_cooldown_mult: 20,
            effect_bonus_flat_hp: 600,
            ..Self::base()
        }
    }

    pub fn with_config(cfg: &ItemConfig) -> Self {
        Self::base().configured(cfg)
    }

    pub fn radiant_with_config(cfg: &ItemConfig) -> Self {
        Self::radiant().configured(cfg)
    }

    fn configured(mut self, cfg: &ItemConfig) -> Self {
        apply_config!(
            self,
            cfg,
            [
                price,
                hp,
                skill_cooldown_mult,
                move_speed_mult,
                effect_bonus_flat_hp,
                effect_hp_percent_boost,
                effect_hp_percent_threshold,
                effect_duration_seconds,
                effect_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for ProtoplasmHarness {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for ProtoplasmHarness {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        self.meta.key
    }

    fn icon(&self) -> &str {
        self.meta.key
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        self.meta.tier
    }

    fn previous_tier(&self) -> Vec<String> {
        self.meta.previous_tier()
    }

    fn next_tier(&self) -> Vec<String> {
        self.meta.next_tier()
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
        let has_harness_buff: bool = has_buff(&entity_ref, self.buff_buff);
        let has_cooldown_buff: bool = has_buff(&entity_ref, self.cooldown_buff_buff);
        let hp_threshold = percent_of(entity_ref.hp().max, self.effect_hp_percent_threshold);

        if !has_harness_buff && !has_cooldown_buff && (entity_ref.hp().current <= hp_threshold) {
            let bonus_max_hp = self.effect_bonus_flat_hp
                + percent_of(entity_ref.hp().max, self.effect_hp_percent_boost) as i32;
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: ticks(self.effect_duration_seconds),
                    },
                    hp: bonus_max_hp,
                    name: buff_name(self.buff_buff),
                    ..Default::default()
                },
            );

            ctx.heal(entity, entity, percent_of_i32(bonus_max_hp, 50.0) as usize);
            ctx.add_buff(
                entity,
                BuffState {
                    duration: BuffType::Time {
                        tick: ticks(self.effect_cooldown_seconds),
                    },
                    name: buff_name(self.cooldown_buff_buff),
                    ..Default::default()
                },
            );
        }
    }
}

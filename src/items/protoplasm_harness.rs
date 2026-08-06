use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, percent_of, percent_of_i32, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct ProtoplasmHarness {
    meta: ItemMeta,
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
                &["winged_moonplate"],
                &["radiant_protoplasm_harness"],
            ),
            buff_buff: "protoplasm_harness_buff",
            cooldown_buff_buff: "protoplasm_harness_cooldown_buff",
            price: 1200,
            hp: 350,
            skill_cooldown_mult: 10,
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
            cooldown_buff_buff: "protoplasm_harness_cooldown_buff",
            price: 1650,
            hp: 650,
            effect_bonus_flat_hp: 600,
            effect_hp_percent_boost: 25.0,
            effect_hp_percent_threshold: 40.0,
            effect_duration_seconds: 6.0,
            effect_cooldown_seconds: 30.0,
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

impl StableItem for ProtoplasmHarness {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        self.meta.key.to_string()
    }

    fn icon(&self) -> String {
        self.meta.key.to_string()
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

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp: self.hp,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        _attacker: usize,
        _damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        let has_harness_buff: bool = has_buff(&entity_ref, self.buff_buff);
        let has_cooldown_buff: bool = has_buff(&entity_ref, self.cooldown_buff_buff);
        let hp_threshold = percent_of(entity_ref.hp().1, self.effect_hp_percent_threshold);

        if !has_harness_buff && !has_cooldown_buff && (entity_ref.hp().0 <= hp_threshold) {
            let bonus_max_hp = self.effect_bonus_flat_hp
                + percent_of(entity_ref.hp().1, self.effect_hp_percent_boost) as i32;
            ctx.add_buff(
                entity,
                &BuffV1 {
                    hp: bonus_max_hp,
                    ..BuffV1::timed(self.buff_buff, ticks(self.effect_duration_seconds))
                },
            );

            ctx.heal(entity, entity, percent_of_i32(bonus_max_hp, 50.0) as usize);
            ctx.add_buff(
                entity,
                &BuffV1::timed(self.cooldown_buff_buff, ticks(self.effect_cooldown_seconds)),
            );
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::CooltimeReduce,
            ItemTagV1::MoveSpeed,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}

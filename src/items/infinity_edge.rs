use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta};

#[derive(Clone, Debug)]
pub struct InfinityEdge {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    crit_chance: i32,
    effect_crit_damage_bonus: i32,
}

impl InfinityEdge {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("infinity_edge", &["bf_sword"], &["radiant_infinity_edge"]),
            price: 1500,
            attack: 75,
            crit_chance: 25,
            effect_crit_damage_bonus: 30,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_infinity_edge", &["infinity_edge"]),
            price: 2200,
            attack: 115,
            crit_chance: 50,
            effect_crit_damage_bonus: 30,
        }
    }

    pub fn with_config(cfg: &ItemConfig) -> Self {
        Self::base().configured(cfg)
    }

    pub fn radiant_with_config(cfg: &ItemConfig) -> Self {
        Self::radiant().configured(cfg)
    }

    fn configured(mut self, cfg: &ItemConfig) -> Self {
        apply_config!(self, cfg, [price, attack, crit_chance]);
        self
    }
}

impl Default for InfinityEdge {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for InfinityEdge {
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
            attack: self.attack,
            crit_chance: self.crit_chance,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        _sim: &mut StableSim<'_>,
        _caster: usize,
        _target: usize,
        damage: &mut usize,
        _damage_type: mod_api_stable::DamageTypeV1,
        attack_type: AttackTypeV1,
        is_crit: bool,
    ) {
        if attack_type == AttackTypeV1::BaseAttack && is_crit {
            *damage += *damage * (self.effect_crit_damage_bonus as f64 / 100.0) as usize
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

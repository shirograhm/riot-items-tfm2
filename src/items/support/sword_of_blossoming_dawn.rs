use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta};

/// Raw stats only — no passive yet.
#[derive(Clone, Debug)]
pub struct SwordOfBlossomingDawn {
    meta: ItemMeta,
    price: usize,
    attack_speed_mult: i32,
    hp: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
}

impl SwordOfBlossomingDawn {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "sword_of_blossoming_dawn",
                &["forbidden_idol"],
                &["radiant_sword_of_blossoming_dawn"],
            ),
            price: 1000,
            attack_speed_mult: 20,
            hp: 200,
            magic_power: 40,
            skill_cooldown_mult: 10,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant(
                "radiant_sword_of_blossoming_dawn",
                &["sword_of_blossoming_dawn"],
            ),
            price: 1700,
            attack_speed_mult: 35,
            hp: 350,
            magic_power: 70,
            skill_cooldown_mult: 15,
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
                attack_speed_mult,
                hp,
                magic_power,
                skill_cooldown_mult
            ]
        );
        self
    }
}

impl Default for SwordOfBlossomingDawn {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for SwordOfBlossomingDawn {
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
            attack_speed_mult: self.attack_speed_mult,
            hp: self.hp,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Ap,
            ItemTagV1::AttackSpeed,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}

use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta};

#[derive(Clone, Debug)]
pub struct ArdentCenser {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    move_speed_mult: i32,
}

impl ArdentCenser {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "ardent_censer",
                &["bandleglass_mirror"],
                &["radiant_ardent_censer"],
            ),
            price: 1000,
            hp: 200,
            hp_regen: 2,
            magic_power: 45,
            skill_cooldown_mult: 5,
            move_speed_mult: 5,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_ardent_censer", &["ardent_censer"]),
            price: 1700,
            hp: 400,
            hp_regen: 4,
            magic_power: 90,
            skill_cooldown_mult: 5,
            move_speed_mult: 5,
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
                hp_regen,
                magic_power,
                skill_cooldown_mult,
                move_speed_mult
            ]
        );
        self
    }
}

impl Default for ArdentCenser {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for ArdentCenser {
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
            hp_regen: self.hp_regen,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::HpRegen,
            ItemTagV1::Ap,
            ItemTagV1::MoveSpeed,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}

use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta};

#[derive(Clone, Debug)]
pub struct ExperimentalHexplate {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    attack_speed_mult: i32,
    // Radiant-only. Zero on the base variant, which contributes no movement speed
    // and correspondingly does not carry the `MoveSpeed` tag.
    move_speed_mult: i32,
    ult_cooldown_mult: i32,
}

impl ExperimentalHexplate {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "experimental_hexplate",
                &["ring_of_reincarnation"],
                &["radiant_experimental_hexplate"],
            ),
            price: 1200,
            hp: 350,
            attack_speed_mult: 35,
            move_speed_mult: 0,
            ult_cooldown_mult: 15,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_experimental_hexplate", &["experimental_hexplate"]),
            price: 1850,
            hp: 500,
            attack_speed_mult: 50,
            move_speed_mult: 5,
            ult_cooldown_mult: 25,
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
                attack_speed_mult,
                move_speed_mult,
                ult_cooldown_mult
            ]
        );
        self
    }
}

impl Default for ExperimentalHexplate {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for ExperimentalHexplate {
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
            attack_speed_mult: self.attack_speed_mult,
            move_speed_mult: self.move_speed_mult,
            ult_cooldown_mult: self.ult_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        let mut tags = vec![ItemTag::HP, ItemTag::AS];
        if self.move_speed_mult > 0 {
            tags.push(ItemTag::MoveSpeed);
        }
        tags.push(ItemTag::CooltimeReduce);
        tags
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

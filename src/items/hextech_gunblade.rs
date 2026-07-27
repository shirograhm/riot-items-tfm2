use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta};

#[derive(Clone, Debug)]
pub struct HextechGunblade {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    magic_power: i32,
    vamp: i32,
}

impl HextechGunblade {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "hextech_gunblade",
                &["ruinous_blade", "spirit_crystal"],
                &["radiant_hextech_gunblade"],
            ),
            price: 1500,
            attack: 50,
            magic_power: 100,
            vamp: 10,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_hextech_gunblade", &["hextech_gunblade"]),
            price: 2100,
            attack: 85,
            magic_power: 150,
            vamp: 15,
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
        apply_config!(self, cfg, [price, attack, magic_power, vamp]);
        self
    }
}

impl Default for HextechGunblade {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for HextechGunblade {
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
            attack: self.attack,
            magic_power: self.magic_power,
            vamp: self.vamp,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AP, ItemTag::Vamp]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

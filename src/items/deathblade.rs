use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta};
use mod_api::*;

#[derive(Clone, Debug)]
pub struct DeathBlade {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    attack_mult: i32,
}

impl DeathBlade {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("deathblade", &["bf_sword"], &["radiant_deathblade"]),
            price: 1400,
            attack: 90,
            attack_mult: 15,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_deathblade", &["deathblade"]),
            price: 2000,
            attack: 140,
            attack_mult: 25,
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
        apply_config!(self, cfg, [price, attack, attack_mult]);
        self
    }
}

impl Default for DeathBlade {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for DeathBlade {
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
            attack_mult: self.attack_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

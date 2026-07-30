use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta};
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct VoidStaff {
    meta: ItemMeta,
    price: usize,
    magic_power: i32,
    magic_resistance_penetration: usize,
}

impl VoidStaff {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("void_staff", &["blighting_jewel"], &["radiant_void_staff"]),
            price: 1500,
            magic_power: 95,
            magic_resistance_penetration: 25,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_void_staff", &["void_staff"]),
            price: 2200,
            magic_power: 160,
            magic_resistance_penetration: 40,
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
            [price, magic_power, magic_resistance_penetration]
        );
        self
    }
}

impl Default for VoidStaff {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for VoidStaff {
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
            magic_power: self.magic_power,
            magic_resistance_penetration: self.magic_resistance_penetration,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}

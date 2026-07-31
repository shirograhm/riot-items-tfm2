use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta};
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct RabadonsDeathcap {
    meta: ItemMeta,
    price: usize,
    magic_power: i32,
    magic_power_mult: i32,
}

impl RabadonsDeathcap {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "rabadons_deathcap",
                &["needlessly_large_rod"],
                &["radiant_rabadons_deathcap"],
            ),
            price: 1500,
            magic_power: 165,
            magic_power_mult: 20,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_rabadons_deathcap", &["rabadons_deathcap"]),
            price: 2300,
            magic_power: 230,
            magic_power_mult: 35,
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
        apply_config!(self, cfg, [price, magic_power, magic_power_mult]);
        self
    }
}

impl Default for RabadonsDeathcap {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for RabadonsDeathcap {
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
            magic_power_mult: self.magic_power_mult,
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

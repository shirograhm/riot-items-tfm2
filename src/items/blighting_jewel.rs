use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct BlightingJewel {
    price: usize,
    magic_power: i32,
    magic_resistance_penetration: usize,
}

impl Default for BlightingJewel {
    fn default() -> Self {
        Self {
            price: 800,
            magic_power: 80,
            magic_resistance_penetration: 10,
        }
    }
}

impl BlightingJewel {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [price, magic_power, magic_resistance_penetration]
        );
        item
    }
}

impl StableItem for BlightingJewel {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "blighting_jewel".to_string()
    }

    fn icon(&self) -> String {
        "blighting_jewel".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["spirit_crystal".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["void_staff".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            magic_power: self.magic_power,
            magic_resistance_penetration: self.magic_resistance_penetration,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap, ItemTagV1::MrPenetration]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}

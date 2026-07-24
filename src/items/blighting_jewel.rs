use crate::config::ItemConfig;
use mod_api::*;

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
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            magic_resistance_penetration: cfg
                .magic_resistance_penetration
                .unwrap_or(d.magic_resistance_penetration),
        }
    }
}

impl ModItemInfo for BlightingJewel {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "blighting_jewel"
    }

    fn icon(&self) -> &str {
        "blighting_jewel"
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
        vec!["void_staff".to_string(), "malignance".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            magic_resistance_penetration: self.magic_resistance_penetration,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::MRPenetration]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct NeedlesslyLargeRod {
    price: usize,
    magic_power: i32,
}

impl Default for NeedlesslyLargeRod {
    fn default() -> Self {
        Self {
            price: 850,
            magic_power: 115,
        }
    }
}

impl NeedlesslyLargeRod {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, magic_power]);
        item
    }
}

impl StableItem for NeedlesslyLargeRod {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "needlessly_large_rod".to_string()
    }

    fn icon(&self) -> String {
        "needlessly_large_rod".to_string()
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
        vec![
            "rabadons_deathcap".to_string(),
            "shadowflame".to_string(),
            "rylais_crystal_scepter".to_string(),
            "nashors_tooth".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            magic_power: self.magic_power,
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

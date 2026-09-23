use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct SorcerersShoes {
    price: usize,
    magic_resistance_penetration: usize,
    move_speed_mult: i32,
}

impl Default for SorcerersShoes {
    fn default() -> Self {
        Self {
            price: 650,
            magic_resistance_penetration: 15,
            move_speed_mult: 8,
        }
    }
}

impl SorcerersShoes {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, magic_resistance_penetration, move_speed_mult]);
        item
    }
}

impl StableItem for SorcerersShoes {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "sorcerers_shoes".to_string()
    }

    fn icon(&self) -> String {
        "sorcerers_shoes".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["boots".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            magic_resistance_penetration: self.magic_resistance_penetration,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::MrPenetration, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}

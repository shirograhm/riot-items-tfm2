use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct BootsOfSwiftness {
    price: usize,
    toughness: usize,
    move_speed_mult: i32,
}

impl Default for BootsOfSwiftness {
    fn default() -> Self {
        Self {
            price: 650,
            toughness: 10,
            move_speed_mult: 20,
        }
    }
}

impl BootsOfSwiftness {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, toughness, move_speed_mult]);
        item
    }
}

impl StableItem for BootsOfSwiftness {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "boots_of_swiftness".to_string()
    }

    fn icon(&self) -> String {
        "boots_of_swiftness".to_string()
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
            toughness: self.toughness,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Toughness, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Defense
    }
}

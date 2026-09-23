use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

/// The tier-1 boots every upgraded pair builds from.
#[derive(Clone, Debug)]
pub struct Boots {
    price: usize,
    move_speed_mult: i32,
}

impl Default for Boots {
    fn default() -> Self {
        Self {
            price: 250,
            move_speed_mult: 5,
        }
    }
}

impl Boots {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, move_speed_mult]);
        item
    }
}

impl StableItem for Boots {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "boots".to_string()
    }

    fn icon(&self) -> String {
        "boots".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        0
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "berserkers_greaves".to_string(),
            "boots_of_swiftness".to_string(),
            "gluttonous_greaves".to_string(),
            "ionian_boots_of_lucidity".to_string(),
            "mercurys_treads".to_string(),
            "plated_steelcaps".to_string(),
            "sorcerers_shoes".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Defense
    }
}

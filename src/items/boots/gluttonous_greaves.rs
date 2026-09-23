use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct GluttonousGreaves {
    price: usize,
    vamp: i32,
    move_speed_mult: i32,
}

impl Default for GluttonousGreaves {
    fn default() -> Self {
        Self {
            price: 650,
            vamp: 8,
            move_speed_mult: 8,
        }
    }
}

impl GluttonousGreaves {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, vamp, move_speed_mult]);
        item
    }
}

impl StableItem for GluttonousGreaves {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "gluttonous_greaves".to_string()
    }

    fn icon(&self) -> String {
        "gluttonous_greaves".to_string()
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
            vamp: self.vamp,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Vamp, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

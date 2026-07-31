use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct WingedMoonplate {
    price: usize,
    hp: i32,
    move_speed_mult: i32,
}

impl Default for WingedMoonplate {
    fn default() -> Self {
        Self {
            price: 800,
            hp: 250,
            move_speed_mult: 4,
        }
    }
}

impl WingedMoonplate {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, hp, move_speed_mult]);
        item
    }
}

impl StableItem for WingedMoonplate {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "winged_moonplate".to_string()
    }

    fn icon(&self) -> String {
        "winged_moonplate".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hardened_heart".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "dead_mans_plate".to_string(),
            "protoplasm_harness".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp: self.hp,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Defense
    }
}

use crate::apply_config;
use crate::config::ItemConfig;
use mod_api::*;

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
            hp: 200,
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

impl ModItemInfo for WingedMoonplate {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "winged_moonplate"
    }

    fn icon(&self) -> &str {
        "winged_moonplate"
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

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::MoveSpeed]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Defense
    }
}

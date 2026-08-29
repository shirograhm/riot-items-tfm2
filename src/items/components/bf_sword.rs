use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct BFSword {
    price: usize,
    attack: i32,
}

impl Default for BFSword {
    fn default() -> Self {
        Self {
            price: 850,
            attack: 65,
        }
    }
}

impl BFSword {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, attack]);
        item
    }
}

impl StableItem for BFSword {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "bf_sword".to_string()
    }

    fn icon(&self) -> String {
        "bf_sword".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["soldiers_longsword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["infinity_edge".to_string(), "deathblade".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack: self.attack,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

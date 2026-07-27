use crate::apply_config;
use crate::config::ItemConfig;
use mod_api::*;

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

impl ModItemInfo for BFSword {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "bf_sword"
    }

    fn icon(&self) -> &str {
        "bf_sword"
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

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

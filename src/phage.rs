use crate::config::ItemConfig;
use mod_api::*;

#[derive(Clone, Debug)]
pub struct Phage {
    price: usize,
    hp: i32,
    attack: i32,
}

impl Default for Phage {
    fn default() -> Self {
        Self {
            price: 850,
            hp: 200,
            attack: 30,
        }
    }
}

impl Phage {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            attack: cfg.attack.unwrap_or(d.attack),
        }
    }
}

impl ModItemInfo for Phage {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "phage"
    }

    fn icon(&self) -> &str {
        "phage"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "hardened_heart".to_string(),
            "soldiers_longsword".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["frozen_mallet".to_string(), "black_cleaver".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
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

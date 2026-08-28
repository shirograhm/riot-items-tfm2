use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct Noonquiver {
    price: usize,
    attack: i32,
    crit_chance: i32,
}

impl Default for Noonquiver {
    fn default() -> Self {
        Self {
            price: 800,
            attack: 45,
            crit_chance: 10,
        }
    }
}

impl Noonquiver {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, attack, crit_chance]);
        item
    }
}

impl StableItem for Noonquiver {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "noonquiver".to_string()
    }

    fn icon(&self) -> String {
        "noonquiver".to_string()
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
        vec![
            "stormrazor".to_string(),
            "yun_tal_wildarrows".to_string(),
            "collector".to_string(),
            "lord_dominiks_regards".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack: self.attack,
            crit_chance: self.crit_chance,
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

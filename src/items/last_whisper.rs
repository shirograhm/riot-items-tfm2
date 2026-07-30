use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct LastWhisper {
    price: usize,
    attack: i32,
    defence_penetration: usize,
}

impl Default for LastWhisper {
    fn default() -> Self {
        Self {
            price: 950,
            attack: 45,
            defence_penetration: 10,
        }
    }
}

impl LastWhisper {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, attack, defence_penetration]);
        item
    }
}

impl StableItem for LastWhisper {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "last_whisper".to_string()
    }

    fn icon(&self) -> String {
        "last_whisper".to_string()
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
            "lord_dominiks_regards".to_string(),
            "mortal_reminder".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack: self.attack,
            defence_penetration: self.defence_penetration,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::DefensePenetration]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

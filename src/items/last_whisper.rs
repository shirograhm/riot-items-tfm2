use crate::apply_config;
use crate::config::ItemConfig;
use mod_api::*;

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

impl ModItemInfo for LastWhisper {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "last_whisper"
    }

    fn icon(&self) -> &str {
        "last_whisper"
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

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            defence_penetration: self.defence_penetration,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::DefensePenetration]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

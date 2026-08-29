use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct CaulfieldsWarhammer {
    price: usize,
    attack: i32,
    skill_cooldown_mult: i32,
}

impl Default for CaulfieldsWarhammer {
    fn default() -> Self {
        Self {
            price: 950,
            attack: 45,
            skill_cooldown_mult: 10,
        }
    }
}

impl CaulfieldsWarhammer {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, attack, skill_cooldown_mult]);
        item
    }
}

impl StableItem for CaulfieldsWarhammer {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "caulfields_warhammer".to_string()
    }

    fn icon(&self) -> String {
        "caulfields_warhammer".to_string()
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
        vec!["eclipse".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack: self.attack,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

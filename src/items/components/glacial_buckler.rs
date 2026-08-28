use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct GlacialBuckler {
    price: usize,
    defence: i32,
    skill_cooldown_mult: i32,
}

impl Default for GlacialBuckler {
    fn default() -> Self {
        Self {
            price: 800,
            defence: 50,
            skill_cooldown_mult: 5,
        }
    }
}

impl GlacialBuckler {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, defence, skill_cooldown_mult]);
        item
    }
}

impl StableItem for GlacialBuckler {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "glacial_buckler".to_string()
    }

    fn icon(&self) -> String {
        "glacial_buckler".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["gatekeepers_armor".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["frozen_heart".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            defence: self.defence,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Defense, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Defense
    }
}

use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct GlowingMote {
    price: usize,
    skill_cooldown_mult: i32,
}

impl Default for GlowingMote {
    fn default() -> Self {
        Self {
            price: 500,
            skill_cooldown_mult: 10,
        }
    }
}

impl GlowingMote {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, skill_cooldown_mult]);
        item
    }
}

impl StableItem for GlowingMote {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "glowing_mote".to_string()
    }

    fn icon(&self) -> String {
        "glowing_mote".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        0
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["sheen".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}

use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct ForbiddenIdol {
    price: usize,
    hp: i32,
    hp_regen: i32,
}

impl Default for ForbiddenIdol {
    fn default() -> Self {
        Self {
            price: 150,
            hp: 200,
            hp_regen: 2,
        }
    }
}

impl ForbiddenIdol {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, hp, hp_regen]);
        item
    }
}

impl StableItem for ForbiddenIdol {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "forbidden_idol".to_string()
    }

    fn icon(&self) -> String {
        "forbidden_idol".to_string()
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
            "sword_of_blossoming_dawn".to_string(),
            "echoes_of_helia".to_string(),
            "staff_of_flowing_water".to_string(),
            "mikaels_blessing".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp: self.hp,
            hp_regen: self.hp_regen,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::HpRegen]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}

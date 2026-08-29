use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct ForbiddenIdol {
    price: usize,
    hp: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
}

impl Default for ForbiddenIdol {
    fn default() -> Self {
        Self {
            price: 650,
            hp: 200,
            magic_power: 20,
            skill_cooldown_mult: 10,
        }
    }
}

impl ForbiddenIdol {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, hp, magic_power, skill_cooldown_mult]);
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
            "bloodsong".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp: self.hp,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Ap,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}

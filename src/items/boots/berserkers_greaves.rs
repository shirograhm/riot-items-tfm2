use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct BerserkersGreaves {
    price: usize,
    attack_speed_mult: i32,
    move_speed_mult: i32,
}

impl Default for BerserkersGreaves {
    fn default() -> Self {
        Self {
            price: 650,
            attack_speed_mult: 20,
            move_speed_mult: 10,
        }
    }
}

impl BerserkersGreaves {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, attack_speed_mult, move_speed_mult]);
        item
    }
}

impl StableItem for BerserkersGreaves {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "berserkers_greaves".to_string()
    }

    fn icon(&self) -> String {
        "berserkers_greaves".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["boots".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack_speed_mult: self.attack_speed_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::AttackSpeed, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}

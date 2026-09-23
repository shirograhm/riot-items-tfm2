use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct IonianBootsOfLucidity {
    price: usize,
    skill_cooldown_mult: i32,
    move_speed_mult: i32,
}

impl Default for IonianBootsOfLucidity {
    fn default() -> Self {
        Self {
            price: 650,
            skill_cooldown_mult: 15,
            move_speed_mult: 8,
        }
    }
}

impl IonianBootsOfLucidity {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, skill_cooldown_mult, move_speed_mult]);
        item
    }
}

impl StableItem for IonianBootsOfLucidity {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "ionian_boots_of_lucidity".to_string()
    }

    fn icon(&self) -> String {
        "ionian_boots_of_lucidity".to_string()
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
            skill_cooldown_mult: self.skill_cooldown_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::CooltimeReduce, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}

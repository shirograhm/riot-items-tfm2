use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct HearthboundAxe {
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
}

impl Default for HearthboundAxe {
    fn default() -> Self {
        Self {
            price: 950,
            attack: 30,
            attack_speed_mult: 20,
        }
    }
}

impl HearthboundAxe {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, attack, attack_speed_mult]);
        item
    }
}

impl StableItem for HearthboundAxe {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "hearthbound_axe".to_string()
    }

    fn icon(&self) -> String {
        "hearthbound_axe".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["soldiers_longsword".to_string(), "wind_dagger".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["feral_flare".to_string()]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack: self.attack,
            attack_speed_mult: self.attack_speed_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::AttackSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

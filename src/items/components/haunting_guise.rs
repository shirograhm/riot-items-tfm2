use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct HauntingGuise {
    price: usize,
    magic_power: i32,
    hp: i32,
}

impl Default for HauntingGuise {
    fn default() -> Self {
        Self {
            price: 950,
            magic_power: 60,
            hp: 200,
        }
    }
}

impl HauntingGuise {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, magic_power, hp]);
        item
    }
}

impl StableItem for HauntingGuise {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "haunting_guise".to_string()
    }

    fn icon(&self) -> String {
        "haunting_guise".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hardened_heart".to_string(), "spirit_crystal".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "riftmaker".to_string(),
            "bloodletters_curse".to_string(),
            "dusk_and_dawn".to_string(),
            "liandrys_torment".to_string(),
            "grezs_spectral_lantern".to_string(),
            "night_harvester".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            magic_power: self.magic_power,
            hp: self.hp,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}

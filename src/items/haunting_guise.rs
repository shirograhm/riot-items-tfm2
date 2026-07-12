use crate::config::ItemConfig;
use mod_api::*;

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
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            hp: cfg.hp.unwrap_or(d.hp),
        }
    }
}

impl ModItemInfo for HauntingGuise {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "haunting_guise"
    }

    fn icon(&self) -> &str {
        "haunting_guise"
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
        ]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            hp: self.hp,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

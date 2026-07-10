use crate::config::ItemConfig;
use mod_api::*;

#[derive(Clone, Debug)]
pub struct AegisOfTheLegion {
    price: usize,
    hp: i32,
    defence: i32,
    magic_resistance: i32,
}

impl Default for AegisOfTheLegion {
    fn default() -> Self {
        Self {
            price: 950,
            hp: 150,
            defence: 40,
            magic_resistance: 40,
        }
    }
}

impl AegisOfTheLegion {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            defence: cfg.defence.unwrap_or(d.defence),
            magic_resistance: cfg.magic_resistance.unwrap_or(d.magic_resistance),
        }
    }
}

impl ModItemInfo for AegisOfTheLegion {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "aegis_of_the_legion"
    }

    fn icon(&self) -> &str {
        "aegis_of_the_legion"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["gatekeepers_armor".to_string(), "night_hood".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["deaths_dance".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            defence: self.defence,
            magic_resistance: self.magic_resistance,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::Defense, ItemTag::MagicResistance]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Defense
    }
}

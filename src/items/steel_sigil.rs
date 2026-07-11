use crate::config::ItemConfig;
use mod_api::*;

#[derive(Clone, Debug)]
pub struct SteelSigil {
    price: usize,
    attack: i32,
    defence: i32,
}

impl Default for SteelSigil {
    fn default() -> Self {
        Self {
            price: 950,
            attack: 30,
            defence: 50,
        }
    }
}

impl SteelSigil {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            defence: cfg.defence.unwrap_or(d.defence),
        }
    }
}

impl ModItemInfo for SteelSigil {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "steel_sigil"
    }

    fn icon(&self) -> &str {
        "steel_sigil"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["soldiers_longsword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["deaths_dance".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            defence: self.defence,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

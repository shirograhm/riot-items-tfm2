use crate::config::ItemConfig;
use mod_api::*;

#[derive(Clone, Debug)]
pub struct Noonquiver {
    price: usize,
    attack: i32,
    crit_chance: i32,
}

impl Default for Noonquiver {
    fn default() -> Self {
        Self {
            price: 800,
            attack: 45,
            crit_chance: 10,
        }
    }
}

impl Noonquiver {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            crit_chance: cfg.crit_chance.unwrap_or(d.crit_chance),
        }
    }
}

impl ModItemInfo for Noonquiver {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "noonquiver"
    }

    fn icon(&self) -> &str {
        "noonquiver"
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
        vec!["stormrazor".to_string(), "yun_tal_wildarrows".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            crit_chance: self.crit_chance,
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

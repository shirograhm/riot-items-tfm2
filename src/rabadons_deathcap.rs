use crate::config::ItemConfig;
use mod_api::*;

#[derive(Clone, Debug)]
pub struct RabadonsDeathcap {
    price: usize,
    magic_power: i32,
    magic_power_mult: i32,
}

impl Default for RabadonsDeathcap {
    fn default() -> Self {
        Self {
            price: 1500,
            magic_power: 165,
            magic_power_mult: 20,
        }
    }
}

impl RabadonsDeathcap {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            magic_power_mult: cfg.magic_power_mult.unwrap_or(d.magic_power_mult),
        }
    }
}

impl ModItemInfo for RabadonsDeathcap {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "rabadons_deathcap"
    }

    fn icon(&self) -> &str {
        "rabadons_deathcap"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["needlessly_large_rod".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_rabadons_deathcap".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            magic_power_mult: self.magic_power_mult,
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

#[derive(Clone, Debug)]
pub struct RadiantRabadonsDeathcap {
    price: usize,
    magic_power: i32,
    magic_power_mult: i32,
}

impl Default for RadiantRabadonsDeathcap {
    fn default() -> Self {
        Self {
            price: 2200,
            magic_power: 240,
            magic_power_mult: 35,
        }
    }
}

impl RadiantRabadonsDeathcap {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            magic_power_mult: cfg.magic_power_mult.unwrap_or(d.magic_power_mult),
        }
    }
}

impl ModItemInfo for RadiantRabadonsDeathcap {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_rabadons_deathcap"
    }

    fn icon(&self) -> &str {
        "radiant_rabadons_deathcap"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["rabadons_deathcap".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            magic_power_mult: self.magic_power_mult,
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

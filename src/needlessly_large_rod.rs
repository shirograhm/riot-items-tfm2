use mod_api::*;
use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct NeedlesslyLargeRod {
    price: usize,
    magic_power: i32,
}

impl Default for NeedlesslyLargeRod {
    fn default() -> Self {
        Self { price: 850, magic_power: 115 }
    }
}

impl NeedlesslyLargeRod {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
        }
    }
}

impl ModItemInfo for NeedlesslyLargeRod {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "needlessly_large_rod"
    }

    fn icon(&self) -> &str {
        "t6_9"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["spirit_crystal".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["rabadons_deathcap".to_string(), "shadowflame".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
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

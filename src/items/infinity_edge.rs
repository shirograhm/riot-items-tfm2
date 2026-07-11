use mod_api::*;
use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct InfinityEdge {
    price: usize,
    attack: i32,
    crit_chance: i32,
}

impl Default for InfinityEdge {
    fn default() -> Self {
        Self { price: 1300, attack: 80, crit_chance: 25 }
    }
}

impl InfinityEdge {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            crit_chance: cfg.crit_chance.unwrap_or(d.crit_chance),
        }
    }
}

impl ModItemInfo for InfinityEdge {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "infinity_edge"
    }

    fn icon(&self) -> &str {
        "infinity_edge"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["bf_sword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_infinity_edge".to_string()]
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

#[derive(Clone, Debug)]
pub struct RadiantInfinityEdge {
    price: usize,
    attack: i32,
    crit_chance: i32,
}

impl Default for RadiantInfinityEdge {
    fn default() -> Self {
        Self { price: 1900, attack: 120, crit_chance: 50 }
    }
}

impl RadiantInfinityEdge {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            crit_chance: cfg.crit_chance.unwrap_or(d.crit_chance),
        }
    }
}

impl ModItemInfo for RadiantInfinityEdge {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_infinity_edge"
    }

    fn icon(&self) -> &str {
        "radiant_infinity_edge"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["infinity_edge".to_string()]
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

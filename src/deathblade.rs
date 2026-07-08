use crate::config::ItemConfig;
use mod_api::*;

#[derive(Clone, Debug)]
pub struct DeathBlade {
    price: usize,
    attack: i32,
    attack_mult: i32,
}

impl Default for DeathBlade {
    fn default() -> Self {
        Self {
            price: 1400,
            attack: 90,
            attack_mult: 15,
        }
    }
}

impl DeathBlade {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            attack_mult: cfg.attack_mult.unwrap_or(d.attack_mult),
        }
    }
}

impl ModItemInfo for DeathBlade {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "deathblade"
    }

    fn icon(&self) -> &str {
        "deathblade"
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
        vec!["radiant_deathblade".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            attack_mult: self.attack_mult,
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
pub struct RadiantDeathBlade {
    price: usize,
    attack: i32,
    attack_mult: i32,
}

impl Default for RadiantDeathBlade {
    fn default() -> Self {
        Self {
            price: 2000,
            attack: 140,
            attack_mult: 25,
        }
    }
}

impl RadiantDeathBlade {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            attack_mult: cfg.attack_mult.unwrap_or(d.attack_mult),
        }
    }
}

impl ModItemInfo for RadiantDeathBlade {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_deathblade"
    }

    fn icon(&self) -> &str {
        "radiant_deathblade"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["deathblade".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            attack_mult: self.attack_mult,
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

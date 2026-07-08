use crate::config::ItemConfig;
use mod_api::*;

#[derive(Clone, Debug)]
pub struct BlackCleaver {
    price: usize,
    attack: i32,
    hp: i32,
    skill_cooldown_mult: i32,
}

impl Default for BlackCleaver {
    fn default() -> Self {
        Self {
            price: 1500,
            attack: 45,
            hp: 300,
            skill_cooldown_mult: 5,
        }
    }
}

impl BlackCleaver {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            hp: cfg.hp.unwrap_or(d.hp),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
        }
    }
}

impl ModItemInfo for BlackCleaver {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "black_cleaver"
    }

    fn icon(&self) -> &str {
        "black_cleaver"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["phage".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_black_cleaver".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            hp: self.hp,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantBlackCleaver {
    price: usize,
    attack: i32,
    hp: i32,
    skill_cooldown_mult: i32,
}

impl Default for RadiantBlackCleaver {
    fn default() -> Self {
        Self {
            price: 2200,
            attack: 70,
            hp: 500,
            skill_cooldown_mult: 10,
        }
    }
}

impl RadiantBlackCleaver {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            hp: cfg.hp.unwrap_or(d.hp),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
        }
    }
}

impl ModItemInfo for RadiantBlackCleaver {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_black_cleaver"
    }

    fn icon(&self) -> &str {
        "radiant_black_cleaver"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["black_cleaver".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            hp: self.hp,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

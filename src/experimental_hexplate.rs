use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct ExperimentalHexplate {
    price: usize,
    hp: i32,
    attack_speed_mult: i32,
    ult_cooldown_mult: i32,
}

impl Default for ExperimentalHexplate {
    fn default() -> Self {
        Self {
            price: 1200,
            hp: 350,
            attack_speed_mult: 35,
            ult_cooldown_mult: 15,
        }
    }
}

impl ExperimentalHexplate {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            ult_cooldown_mult: cfg.ult_cooldown_mult.unwrap_or(d.ult_cooldown_mult),
        }
    }
}

impl ModItemInfo for ExperimentalHexplate {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "experimental_hexplate"
    }

    fn icon(&self) -> &str {
        "experimental_hexplate"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["ring_of_reincarnation".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_experimental_hexplate".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            attack_speed_mult: self.attack_speed_mult,
            ult_cooldown_mult: self.ult_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AS, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

#[derive(Clone, Debug)]
pub struct RadiantExperimentalHexplate {
    price: usize,
    hp: i32,
    attack_speed_mult: i32,
    move_speed_mult: i32,
    ult_cooldown_mult: i32,
}

impl Default for RadiantExperimentalHexplate {
    fn default() -> Self {
        Self {
            price: 1850,
            hp: 500,
            attack_speed_mult: 50,
            move_speed_mult: 5,
            ult_cooldown_mult: 25,
        }
    }
}

impl RadiantExperimentalHexplate {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            attack_speed_mult: cfg.attack_speed_mult.unwrap_or(d.attack_speed_mult),
            move_speed_mult: cfg.move_speed_mult.unwrap_or(d.move_speed_mult),
            ult_cooldown_mult: cfg.ult_cooldown_mult.unwrap_or(d.ult_cooldown_mult),
        }
    }
}

impl ModItemInfo for RadiantExperimentalHexplate {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_experimental_hexplate"
    }

    fn icon(&self) -> &str {
        "radiant_experimental_hexplate"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["experimental_hexplate".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            attack_speed_mult: self.attack_speed_mult,
            move_speed_mult: self.move_speed_mult,
            ult_cooldown_mult: self.ult_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::AS,
            ItemTag::MoveSpeed,
            ItemTag::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

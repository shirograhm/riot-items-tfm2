use crate::config::ItemConfig;
use mod_api::*;

#[derive(Clone, Debug)]
pub struct Malignance {
    price: usize,
    magic_power: i32,
    magic_resistance_penetration: usize,
    skill_cooldown_mult: i32,
    ult_cooldown_mult: i32,
}

impl Default for Malignance {
    fn default() -> Self {
        Self {
            price: 1400,
            magic_power: 120,
            magic_resistance_penetration: 6,
            skill_cooldown_mult: 12,
            ult_cooldown_mult: 12,
        }
    }
}

impl Malignance {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            magic_resistance_penetration: cfg
                .magic_resistance_penetration
                .unwrap_or(d.magic_resistance_penetration),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            ult_cooldown_mult: cfg.ult_cooldown_mult.unwrap_or(d.ult_cooldown_mult),
        }
    }
}

impl ModItemInfo for Malignance {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "malignance"
    }

    fn icon(&self) -> &str {
        "malignance"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["staff_of_rapture".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_malignance".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            magic_resistance_penetration: self.magic_resistance_penetration,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.ult_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::MRPenetration, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Clone, Debug)]
pub struct RadiantMalignance {
    price: usize,
    magic_power: i32,
    magic_resistance_penetration: usize,
    skill_cooldown_mult: i32,
    ult_cooldown_mult: i32,
}

impl Default for RadiantMalignance {
    fn default() -> Self {
        Self {
            price: 2000,
            magic_power: 200,
            magic_resistance_penetration: 10,
            skill_cooldown_mult: 20,
            ult_cooldown_mult: 20,
        }
    }
}

impl RadiantMalignance {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            magic_resistance_penetration: cfg
                .magic_resistance_penetration
                .unwrap_or(d.magic_resistance_penetration),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
            ult_cooldown_mult: cfg.ult_cooldown_mult.unwrap_or(d.ult_cooldown_mult),
        }
    }
}

impl ModItemInfo for RadiantMalignance {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_malignance"
    }

    fn icon(&self) -> &str {
        "radiant_malignance"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["malignance".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            magic_resistance_penetration: self.magic_resistance_penetration,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.ult_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::MRPenetration, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

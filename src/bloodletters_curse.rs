use crate::config::ItemConfig;
use mod_api::*;

#[derive(Clone, Debug)]
pub struct BloodlettersCurse {
    price: usize,
    magic_power: i32,
    hp: i32,
    skill_cooldown_mult: i32,
}

impl Default for BloodlettersCurse {
    fn default() -> Self {
        Self {
            price: 1500,
            magic_power: 110,
            hp: 300,
            skill_cooldown_mult: 5,
        }
    }
}

impl BloodlettersCurse {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            hp: cfg.hp.unwrap_or(d.hp),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
        }
    }
}

impl ModItemInfo for BloodlettersCurse {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "bloodletters_curse"
    }

    fn icon(&self) -> &str {
        "bloodletters_curse"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["haunting_guise".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_bloodletters_curse".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            hp: self.hp,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Clone, Debug)]
pub struct RadiantBloodlettersCurse {
    price: usize,
    magic_power: i32,
    hp: i32,
    skill_cooldown_mult: i32,
}

impl Default for RadiantBloodlettersCurse {
    fn default() -> Self {
        Self {
            price: 2200,
            magic_power: 180,
            hp: 500,
            skill_cooldown_mult: 10,
        }
    }
}

impl RadiantBloodlettersCurse {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            hp: cfg.hp.unwrap_or(d.hp),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
        }
    }
}

impl ModItemInfo for RadiantBloodlettersCurse {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_bloodletters_curse"
    }

    fn icon(&self) -> &str {
        "radiant_bloodletters_curse"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["bloodletters_curse".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            hp: self.hp,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

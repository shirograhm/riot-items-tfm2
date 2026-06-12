use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct ProtectorsVow {
    price: usize,
    hp: i32,
    defence: i32,
}

impl Default for ProtectorsVow {
    fn default() -> Self {
        Self {
            price: 1300,
            hp: 350,
            defence: 50,
        }
    }
}

impl ProtectorsVow {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            defence: cfg.defence.unwrap_or(d.defence),
        }
    }
}

impl ModItemInfo for ProtectorsVow {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "protectors_vow"
    }

    fn icon(&self) -> &str {
        "t6_7"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "ring_of_reincarnation".to_string(),
            "gatekeepers_armor".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_protectors_vow".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            defence: self.defence,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Defense
    }
}

#[derive(Clone, Debug)]
pub struct RadiantProtectorsVow {
    price: usize,
    hp: i32,
    defence: i32,
    skill_cooldown_mult: i32,
}

impl Default for RadiantProtectorsVow {
    fn default() -> Self {
        Self {
            price: 1800,
            hp: 550,
            defence: 75,
            skill_cooldown_mult: 15,
        }
    }
}

impl RadiantProtectorsVow {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            defence: cfg.defence.unwrap_or(d.defence),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
        }
    }
}

impl ModItemInfo for RadiantProtectorsVow {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_protectors_vow"
    }

    fn icon(&self) -> &str {
        "t6_8"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["protectors_vow".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            defence: self.defence,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Defense
    }
}

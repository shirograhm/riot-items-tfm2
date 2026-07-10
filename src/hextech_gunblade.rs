use mod_api::*;

use crate::config::ItemConfig;

// Stat-only item (AD / AP / Omnivamp). No active effect implemented.

#[derive(Clone, Debug)]
pub struct HextechGunblade {
    price: usize,
    attack: i32,
    magic_power: i32,
    vamp: i32,
}

impl Default for HextechGunblade {
    fn default() -> Self {
        Self {
            price: 1500,
            attack: 50,
            magic_power: 100,
            vamp: 10,
        }
    }
}

impl HextechGunblade {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            vamp: cfg.vamp.unwrap_or(d.vamp),
        }
    }
}

impl ModItemInfo for HextechGunblade {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "hextech_gunblade"
    }

    fn icon(&self) -> &str {
        "hextech_gunblade"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["ruinous_blade".to_string(), "spirit_crystal".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_hextech_gunblade".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            magic_power: self.magic_power,
            vamp: self.vamp,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AP, ItemTag::Vamp]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

#[derive(Clone, Debug)]
pub struct RadiantHextechGunblade {
    price: usize,
    attack: i32,
    magic_power: i32,
    vamp: i32,
}

impl Default for RadiantHextechGunblade {
    fn default() -> Self {
        Self {
            price: 2100,
            attack: 85,
            magic_power: 150,
            vamp: 15,
        }
    }
}

impl RadiantHextechGunblade {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            vamp: cfg.vamp.unwrap_or(d.vamp),
        }
    }
}

impl ModItemInfo for RadiantHextechGunblade {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_hextech_gunblade"
    }

    fn icon(&self) -> &str {
        "radiant_hextech_gunblade"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hextech_gunblade".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            magic_power: self.magic_power,
            vamp: self.vamp,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::AP, ItemTag::Vamp]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

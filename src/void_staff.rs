use crate::config::ItemConfig;
use mod_api::*;

#[derive(Clone, Debug)]
pub struct VoidStaff {
    price: usize,
    magic_power: i32,
    magic_resistance_penetration: usize,
}

impl Default for VoidStaff {
    fn default() -> Self {
        Self {
            price: 1500,
            magic_power: 95,
            magic_resistance_penetration: 25,
        }
    }
}

impl VoidStaff {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            magic_resistance_penetration: cfg
                .magic_resistance_penetration
                .unwrap_or(d.magic_resistance_penetration),
        }
    }
}

impl ModItemInfo for VoidStaff {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "void_staff"
    }

    fn icon(&self) -> &str {
        "void_staff"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["blighting_jewel".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_void_staff".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            magic_resistance_penetration: self.magic_resistance_penetration,
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
pub struct RadiantVoidStaff {
    price: usize,
    magic_power: i32,
    magic_resistance_penetration: usize,
}

impl Default for RadiantVoidStaff {
    fn default() -> Self {
        Self {
            price: 2200,
            magic_power: 160,
            magic_resistance_penetration: 40,
        }
    }
}

impl RadiantVoidStaff {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            magic_resistance_penetration: cfg
                .magic_resistance_penetration
                .unwrap_or(d.magic_resistance_penetration),
        }
    }
}

impl ModItemInfo for RadiantVoidStaff {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_void_staff"
    }

    fn icon(&self) -> &str {
        "radiant_void_staff"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["void_staff".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            magic_resistance_penetration: self.magic_resistance_penetration,
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

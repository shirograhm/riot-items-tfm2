use crate::config::ItemConfig;
use mod_api::*;

#[derive(Clone, Debug)]
pub struct BandleglassMirror {
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
}

impl Default for BandleglassMirror {
    fn default() -> Self {
        Self {
            price: 650,
            hp: 250,
            hp_regen: 2,
            magic_power: 20,
            skill_cooldown_mult: 5,
        }
    }
}

impl BandleglassMirror {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            hp_regen: cfg.hp_regen.unwrap_or(d.hp_regen),
            magic_power: cfg.magic_power.unwrap_or(d.magic_power),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
        }
    }
}

impl ModItemInfo for BandleglassMirror {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "bandleglass_mirror"
    }

    fn icon(&self) -> &str {
        "bandleglass_mirror"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hardened_heart".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "zekes_herald".to_string(),
            "echoes_of_helia".to_string(),
            "bloodsong".to_string(),
        ]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            hp_regen: self.hp_regen,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::HPRegen, ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

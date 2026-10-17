use crate::apply_config;
use crate::config::ItemConfig;
use mod_api_stable::*;

#[derive(Clone, Debug)]
pub struct BandleglassMirror {
    price: usize,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
}

impl Default for BandleglassMirror {
    fn default() -> Self {
        Self {
            price: 400,
            hp_regen: 1,
            magic_power: 20,
            skill_cooldown_mult: 10,
        }
    }
}

impl BandleglassMirror {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(
            item,
            cfg,
            [price, hp_regen, magic_power, skill_cooldown_mult]
        );
        item
    }
}

impl StableItem for BandleglassMirror {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "bandleglass_mirror".to_string()
    }

    fn icon(&self) -> String {
        "bandleglass_mirror".to_string()
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["arcane_crystal".to_string(), "glowing_mote".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "ardent_censer".to_string(),
            "echoes_of_helia".to_string(),
            "chemtech_putrifier".to_string(),
            "staff_of_flowing_water".to_string(),
            "imperial_mandate".to_string(),
            "moonstone_renewer".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp_regen: self.hp_regen,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::HpRegen, ItemTagV1::Ap, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}

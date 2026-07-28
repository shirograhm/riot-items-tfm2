use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta};
use mod_api::*;

#[derive(Clone, Debug)]
pub struct Malignance {
    meta: ItemMeta,
    price: usize,
    magic_power: i32,
    skill_cooldown_mult: i32,
    ult_cooldown_mult: i32,
}

impl Malignance {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("malignance", &["staff_of_rapture"], &["radiant_malignance"]),
            price: 1250,
            magic_power: 120,
            skill_cooldown_mult: 12,
            ult_cooldown_mult: 12,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_malignance", &["malignance"]),
            price: 1900,
            magic_power: 200,
            skill_cooldown_mult: 20,
            ult_cooldown_mult: 20,
            ..Self::base()
        }
    }

    pub fn with_config(cfg: &ItemConfig) -> Self {
        Self::base().configured(cfg)
    }

    pub fn radiant_with_config(cfg: &ItemConfig) -> Self {
        Self::radiant().configured(cfg)
    }

    fn configured(mut self, cfg: &ItemConfig) -> Self {
        apply_config!(
            self,
            cfg,
            [price, magic_power, skill_cooldown_mult, ult_cooldown_mult]
        );
        self
    }
}

impl Default for Malignance {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for Malignance {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        self.meta.key
    }

    fn icon(&self) -> &str {
        self.meta.key
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        self.meta.tier
    }

    fn previous_tier(&self) -> Vec<String> {
        self.meta.previous_tier()
    }

    fn next_tier(&self) -> Vec<String> {
        self.meta.next_tier()
    }

    fn stat(&self) -> BuffState {
        BuffState {
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.ult_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AP, ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

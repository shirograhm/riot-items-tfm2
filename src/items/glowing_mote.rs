use crate::config::ItemConfig;
use mod_api::*;

#[derive(Clone, Debug)]
pub struct GlowingMote {
    price: usize,
    skill_cooldown_mult: i32,
}

impl Default for GlowingMote {
    fn default() -> Self {
        Self {
            price: 500,
            skill_cooldown_mult: 10,
        }
    }
}

impl GlowingMote {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            skill_cooldown_mult: cfg.skill_cooldown_mult.unwrap_or(d.skill_cooldown_mult),
        }
    }
}

impl ModItemInfo for GlowingMote {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "glowing_mote"
    }

    fn icon(&self) -> &str {
        "glowing_mote"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        0
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["sheen".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::CooltimeReduce]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AttackSpeed
    }
}

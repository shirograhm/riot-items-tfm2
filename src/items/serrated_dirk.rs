use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, apply_lethality};

#[derive(Clone, Debug)]
pub struct SerratedDirk {
    price: usize,
    attack: i32,
    effect_lethality: usize,
}

impl Default for SerratedDirk {
    fn default() -> Self {
        Self {
            price: 800,
            attack: 45,
            effect_lethality: 10,
        }
    }
}

impl SerratedDirk {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let mut item = Self::default();
        apply_config!(item, cfg, [price, attack, effect_lethality]);
        item
    }
}

impl ModItemInfo for SerratedDirk {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "serrated_dirk"
    }

    fn icon(&self) -> &str {
        "serrated_dirk"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        2
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["soldiers_longsword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec![
            "hubris".to_string(),
            "bastionbreaker".to_string(),
            "serpents_fang".to_string(),
            "collector".to_string(),
            "opportunity".to_string(),
        ]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
        apply_lethality(ctx, target, self.effect_lethality, damage);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

use mod_api_stable::*;

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

impl StableItem for SerratedDirk {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        "serrated_dirk".to_string()
    }

    fn icon(&self) -> String {
        "serrated_dirk".to_string()
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
            "voltaic_cyclosword".to_string(),
        ]
    }

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            attack: self.attack,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        apply_lethality(ctx, caster, target, self.effect_lethality, damage);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

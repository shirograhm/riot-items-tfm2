use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ItemMeta};

#[derive(Clone, Debug)]
pub struct Shadowflame {
    meta: ItemMeta,
    price: usize,
    magic_power: i32,
    magic_resistance_penetration: usize,
    effect_hp_percent_threshold: f64,
}

impl Shadowflame {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "shadowflame",
                &["needlessly_large_rod"],
                &["radiant_shadowflame"],
            ),
            price: 1350,
            magic_power: 115,
            magic_resistance_penetration: 15,
            effect_hp_percent_threshold: 30.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_shadowflame", &["shadowflame"]),
            price: 1800,
            magic_power: 210,
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
            [
                price,
                magic_power,
                magic_resistance_penetration,
                effect_hp_percent_threshold
            ]
        );
        self
    }
}

impl Default for Shadowflame {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Shadowflame {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        self.meta.key.to_string()
    }

    fn icon(&self) -> String {
        self.meta.key.to_string()
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

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            magic_power: self.magic_power,
            magic_resistance_penetration: self.magic_resistance_penetration,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        _caster: usize,
        target: usize,
        damage: &mut usize,
        damage_type: DamageTypeV1,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        let hp_threshold = percent_of(target_ref.hp().1, self.effect_hp_percent_threshold);
        if target_ref.hp().0 < hp_threshold && damage_type == DamageTypeV1::Ap {
            *damage = (*damage as f64 * 1.2) as usize;
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ap, ItemTagV1::MrPenetration]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}

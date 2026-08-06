use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct RylaisCrystalScepter {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    magic_power: i32,
    effect_slow_amount: i32,
    effect_duration_seconds: f64,
}

impl RylaisCrystalScepter {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "rylais_crystal_scepter",
                &["hardened_heart", "needlessly_large_rod"],
                &["radiant_rylais_crystal_scepter"],
            ),
            price: 1350,
            hp: 250,
            magic_power: 125,
            effect_slow_amount: 15,
            effect_duration_seconds: 2.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant(
                "radiant_rylais_crystal_scepter",
                &["rylais_crystal_scepter"],
            ),
            price: 1900,
            hp: 400,
            magic_power: 200,
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
                hp,
                magic_power,
                effect_slow_amount,
                effect_duration_seconds
            ]
        );
        self
    }
}

impl Default for RylaisCrystalScepter {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for RylaisCrystalScepter {
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
            hp: self.hp,
            magic_power: self.magic_power,
            ..Default::default()
        }
    }

    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() || is_ally {
            return;
        }

        let already_slowed = has_buff(&target_ref, "rylais_crystal_scepter_slow");
        if !already_slowed {
            ctx.add_buff(
                target,
                &BuffV1 {
                    move_speed_mult: -self.effect_slow_amount,
                    ..BuffV1::timed(
                        "rylais_crystal_scepter_slow",
                        ticks(self.effect_duration_seconds),
                    )
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::Ap]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}

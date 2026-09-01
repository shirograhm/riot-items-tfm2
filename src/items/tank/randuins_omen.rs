use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ItemMeta};

// Resilience: Heal for 30% of the damage taken from critical strikes.
#[derive(Clone, Debug)]
pub struct RanduinsOmen {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    defence: i32,
    effect_crit_damage_percent_heal: f64,
}

impl RanduinsOmen {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "randuins_omen",
                &["black_knights_heavy_plate", "ring_of_reincarnation"],
                &["radiant_randuins_omen"],
            ),
            price: 1500,
            hp: 300,
            defence: 70,
            effect_crit_damage_percent_heal: 30.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_randuins_omen", &["randuins_omen"]),
            price: 2000,
            hp: 550,
            defence: 90,
            effect_crit_damage_percent_heal: 30.0,
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
            [price, hp, defence, effect_crit_damage_percent_heal]
        );
        self
    }
}

impl Default for RanduinsOmen {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for RanduinsOmen {
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
            defence: self.defence,
            ..Default::default()
        }
    }

    // The host resolves the hit before this runs, so `damage` is what the crit
    // actually landed for: the heal is a share of the wound, not of the raw
    // swing the attacker threw.
    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        _attacker: usize,
        damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        is_crit: bool,
    ) {
        if !is_crit {
            return;
        }

        let heal = percent_of(damage, self.effect_crit_damage_percent_heal);
        if heal == 0 {
            return;
        }

        ctx.heal(entity, entity, heal);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::Defense, ItemTagV1::Vamp]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Defense
    }
}

use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta};

fn apply_fray(ctx: &mut StableSim<'_>, caster: usize, target: usize, magic_damage: usize) {}

#[derive(Clone, Debug)]
pub struct WitsEnd {
    meta: ItemMeta,
    price: usize,
    attack_speed_mult: i32,
    magic_resistance: i32,
    toughness: usize,
    effect_bonus_magic_damage: usize,
    on_hit_cooldown_seconds: f64,
}

impl WitsEnd {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("wits_end", &["scouts_slingshot"], &["radiant_wits_end"]),
            price: 1400,
            attack_speed_mult: 40,
            magic_resistance: 80,
            toughness: 20,
            effect_bonus_magic_damage: 45,
            on_hit_cooldown_seconds: 0.5,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_wits_end", &["wits_end"]),
            price: 2000,
            attack_speed_mult: 65,
            magic_resistance: 130,
            toughness: 30,
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
                attack_speed_mult,
                magic_resistance,
                toughness,
                effect_bonus_magic_damage,
                on_hit_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for WitsEnd {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for WitsEnd {
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
            attack_speed_mult: self.attack_speed_mult,
            magic_resistance: self.magic_resistance,
            toughness: self.toughness,
            ..Default::default()
        }
    }

    fn on_base_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }

        ctx.deal_damage(
            caster,
            target,
            0,
            self.effect_bonus_magic_damage,
            AttackTypeV1::BaseAttack,
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::AttackSpeed,
            ItemTagV1::MagicResistance,
            ItemTagV1::Toughness,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}

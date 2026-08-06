use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, percent_of, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct FrozenMallet {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    attack: i32,
    effect_slow_amount: i32,
    effect_duration_seconds: f64,
    // Icathia's Curse, the radiant-only on-hit bonus. Zero on the base variant,
    // which skips the effect entirely.
    effect_bonus_flat_damage: usize,
    effect_caster_hp_percent_damage: f64,
}

impl FrozenMallet {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("frozen_mallet", &["phage"], &["radiant_frozen_mallet"]),
            price: 1400,
            hp: 400,
            attack: 40,
            effect_slow_amount: 15,
            effect_duration_seconds: 2.0,
            effect_bonus_flat_damage: 0,
            effect_caster_hp_percent_damage: 0.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_frozen_mallet", &["frozen_mallet"]),
            price: 2000,
            hp: 600,
            attack: 60,
            effect_bonus_flat_damage: 20,
            effect_caster_hp_percent_damage: 3.0,
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
                attack,
                effect_slow_amount,
                effect_duration_seconds,
                effect_bonus_flat_damage,
                effect_caster_hp_percent_damage,
            ]
        );
        self
    }
}

impl Default for FrozenMallet {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for FrozenMallet {
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
            attack: self.attack,
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
        let already_slowed = has_buff(&target_ref, "frozen_mallet_slow");

        let caster_hp_max = ctx.get_entity(caster).map(|c| c.hp().1).unwrap_or(0);
        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(caster_hp_max, self.effect_caster_hp_percent_damage);

        if !already_slowed {
            ctx.add_buff(
                target,
                &BuffV1 {
                    move_speed_mult: -self.effect_slow_amount,
                    ..BuffV1::timed("frozen_mallet_slow", ticks(self.effect_duration_seconds))
                },
            );
        }
        ctx.deal_damage(caster, target, bonus_damage, 0, AttackTypeV1::Item);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Ad,
            ItemTagV1::MyHpPercentDamage,
            ItemTagV1::MoveSpeed,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}

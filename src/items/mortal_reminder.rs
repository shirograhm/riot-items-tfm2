use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct MortalReminder {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    defence_penetration: usize,
    crit_chance: i32,
    effect_heal_reduce: usize,
    effect_duration_seconds: f64,
}

impl MortalReminder {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "mortal_reminder",
                &["executioners_calling", "last_whisper"],
                &["radiant_mortal_reminder"],
            ),
            price: 1400,
            attack: 45,
            defence_penetration: 20,
            crit_chance: 20,
            effect_heal_reduce: 40,
            effect_duration_seconds: 2.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_mortal_reminder", &["mortal_reminder"]),
            price: 2000,
            attack: 85,
            defence_penetration: 30,
            crit_chance: 25,
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
                attack,
                defence_penetration,
                crit_chance,
                effect_heal_reduce,
                effect_duration_seconds
            ]
        );
        self
    }
}

impl Default for MortalReminder {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for MortalReminder {
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
            attack: self.attack,
            defence_penetration: self.defence_penetration,
            crit_chance: self.crit_chance,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };

        if damage_type != DamageTypeV1::Ad {
            return;
        }

        let already_reduced = has_buff(&entity_ref, "40_percent_heal_cut");
        if !already_reduced {
            ctx.add_buff(
                target,
                &BuffV1 {
                    heal_reduce: self.effect_heal_reduce,
                    ..BuffV1::timed("40_percent_heal_cut", ticks(self.effect_duration_seconds))
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Ad,
            ItemTagV1::DefensePenetration,
            ItemTagV1::HealReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

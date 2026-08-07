use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta};

fn apply_giant_slayer(
    ctx: &mut StableSim<'_>,
    target: usize,
    damage: &mut usize,
    percent_per_step: f64,
    hp_per_step: usize,
    max_percent: f64,
) {
    let Some(target_ref) = ctx.get_entity(target) else {
        return;
    };
    if target_ref.is_tower() {
        return;
    }

    let steps = (target_ref.hp().1 / hp_per_step.max(1)) as f64;
    let bonus_percent = (percent_per_step * steps).min(max_percent);
    *damage = (*damage as f64 * (1.0 + bonus_percent / 100.0)).round() as usize;
}

#[derive(Clone, Debug)]
pub struct LordDominiksRegards {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    crit_chance: i32,
    defence_penetration: usize,
    effect_percent_bonus_damage: f64,
    effect_hp_per_stack: usize,
    effect_max_percent_bonus: f64,
}

impl LordDominiksRegards {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "lord_dominiks_regards",
                &["last_whisper", "noonquiver"],
                &["radiant_lord_dominiks_regards"],
            ),
            price: 1450,
            attack: 45,
            crit_chance: 20,
            defence_penetration: 25,
            effect_percent_bonus_damage: 3.0,
            effect_hp_per_stack: 1000,
            effect_max_percent_bonus: 15.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_lord_dominiks_regards", &["lord_dominiks_regards"]),
            price: 2000,
            attack: 85,
            crit_chance: 25,
            defence_penetration: 35,
            effect_percent_bonus_damage: 3.0,
            effect_hp_per_stack: 1000,
            effect_max_percent_bonus: 15.0,
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
                crit_chance,
                defence_penetration,
                effect_percent_bonus_damage,
                effect_hp_per_stack,
                effect_max_percent_bonus
            ]
        );
        self
    }
}

impl Default for LordDominiksRegards {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for LordDominiksRegards {
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
            crit_chance: self.crit_chance,
            defence_penetration: self.defence_penetration,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        _caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        apply_giant_slayer(
            ctx,
            target,
            damage,
            self.effect_percent_bonus_damage,
            self.effect_hp_per_stack,
            self.effect_max_percent_bonus,
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::DefensePenetration]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

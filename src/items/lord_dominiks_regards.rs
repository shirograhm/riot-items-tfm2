use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta};

fn apply_giant_slayer(
    ctx: &mut GameCtx,
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
    let steps = (target_ref.hp().max / hp_per_step.max(1)) as f64;
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

impl ModItemInfo for LordDominiksRegards {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        self.meta.key
    }

    fn icon(&self) -> &str {
        self.meta.key
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

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            crit_chance: self.crit_chance,
            defence_penetration: self.defence_penetration,
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
        apply_giant_slayer(
            ctx,
            target,
            damage,
            self.effect_percent_bonus_damage,
            self.effect_hp_per_stack,
            self.effect_max_percent_bonus,
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD, ItemTag::DefensePenetration]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

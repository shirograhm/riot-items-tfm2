use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_lethality, percent_of};

// Lethality (see serrated_dirk) plus Shield Reaver: basic attacks against an
// enemy champion that currently has a shield deal bonus physical damage (flat +
// % of Attack Damage).

#[derive(Clone, Debug)]
pub struct SerpentsFang {
    price: usize,
    attack: i32,
    effect_lethality: usize,
    effect_bonus_flat_damage: usize,
    effect_ad_percent_damage: f64,
}

impl Default for SerpentsFang {
    fn default() -> Self {
        Self {
            price: 1200,
            attack: 55,
            effect_lethality: 15,
            effect_bonus_flat_damage: 50,
            effect_ad_percent_damage: 10.0,
        }
    }
}

impl SerpentsFang {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            effect_lethality: cfg.effect_lethality.unwrap_or(d.effect_lethality),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_ad_percent_damage: cfg
                .effect_ad_percent_damage
                .unwrap_or(d.effect_ad_percent_damage),
        }
    }
}

// Shared Shield Reaver: on a basic attack against a shielded enemy champion, deal
// bonus physical damage (flat + % of the caster's Attack Damage).
fn shield_reaver(
    ctx: &mut GameCtx,
    caster: usize,
    target: usize,
    flat: usize,
    ad_percent: f64,
) {
    let shielded_champion = ctx
        .get_entity(target)
        .map(|t| t.is_champion() && t.shield() > 0)
        .unwrap_or(false);
    if !shielded_champion {
        return;
    }
    let caster_ad = ctx.get_entity(caster).map(|c| c.stat().attack).unwrap_or(0);
    let bonus = flat + percent_of(caster_ad, ad_percent);
    ctx.deal_damage(caster, target, bonus, 0, AttackType::Item);
}

impl ModItemInfo for SerpentsFang {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "serpents_fang"
    }

    fn icon(&self) -> &str {
        "serpents_fang"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["serrated_dirk".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_serpents_fang".to_string()]
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
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
        apply_lethality(ctx, target, self.effect_lethality, damage);
        shield_reaver(
            ctx,
            caster,
            target,
            self.effect_bonus_flat_damage,
            self.effect_ad_percent_damage,
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantSerpentsFang {
    price: usize,
    attack: i32,
    effect_lethality: usize,
    effect_bonus_flat_damage: usize,
    effect_ad_percent_damage: f64,
}

impl Default for RadiantSerpentsFang {
    fn default() -> Self {
        Self {
            price: 1800,
            attack: 95,
            effect_lethality: 15,
            effect_bonus_flat_damage: 85,
            effect_ad_percent_damage: 15.0,
        }
    }
}

impl RadiantSerpentsFang {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            effect_lethality: cfg.effect_lethality.unwrap_or(d.effect_lethality),
            effect_bonus_flat_damage: cfg
                .effect_bonus_flat_damage
                .unwrap_or(d.effect_bonus_flat_damage),
            effect_ad_percent_damage: cfg
                .effect_ad_percent_damage
                .unwrap_or(d.effect_ad_percent_damage),
        }
    }
}

impl ModItemInfo for RadiantSerpentsFang {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_serpents_fang"
    }

    fn icon(&self) -> &str {
        "radiant_serpents_fang"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["serpents_fang".to_string()]
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
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
        apply_lethality(ctx, target, self.effect_lethality, damage);
        shield_reaver(
            ctx,
            caster,
            target,
            self.effect_bonus_flat_damage,
            self.effect_ad_percent_damage,
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

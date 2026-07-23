use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_lethality, percent_of};

#[derive(Clone, Debug)]
pub struct Collector {
    price: usize,
    attack: i32,
    crit_chance: i32,
    effect_lethality: usize,
    effect_hp_percent_threshold: f64,
}

impl Default for Collector {
    fn default() -> Self {
        Self {
            price: 1450,
            attack: 60,
            crit_chance: 20,
            effect_lethality: 10,
            effect_hp_percent_threshold: 6.0,
        }
    }
}

impl Collector {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            crit_chance: cfg.crit_chance.unwrap_or(d.crit_chance),
            effect_lethality: cfg.effect_lethality.unwrap_or(d.effect_lethality),
            effect_hp_percent_threshold: cfg
                .effect_hp_percent_threshold
                .unwrap_or(d.effect_hp_percent_threshold),
        }
    }
}

impl ModItemInfo for Collector {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "collector"
    }

    fn icon(&self) -> &str {
        "collector"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["serrated_dirk".to_string(), "noonquiver".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_collector".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            crit_chance: self.crit_chance,
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

        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }

        let hp_threshold = percent_of(target_ref.hp().max, self.effect_hp_percent_threshold);
        if target_ref.hp().current - *damage <= hp_threshold {
            let lethal_damage = target_ref.hp().current;
            ctx.deal_damage(caster, target, lethal_damage, 0, AttackType::Item);
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantCollector {
    price: usize,
    attack: i32,
    crit_chance: i32,
    effect_lethality: usize,
    effect_hp_percent_threshold: f64,
}

impl Default for RadiantCollector {
    fn default() -> Self {
        Self {
            price: 2100,
            attack: 105,
            crit_chance: 25,
            effect_lethality: 10,
            effect_hp_percent_threshold: 6.0,
        }
    }
}

impl RadiantCollector {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            crit_chance: cfg.crit_chance.unwrap_or(d.crit_chance),
            effect_lethality: cfg.effect_lethality.unwrap_or(d.effect_lethality),
            effect_hp_percent_threshold: cfg
                .effect_hp_percent_threshold
                .unwrap_or(d.effect_hp_percent_threshold),
        }
    }
}

impl ModItemInfo for RadiantCollector {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_collector"
    }

    fn icon(&self) -> &str {
        "radiant_collector"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["collector".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            crit_chance: self.crit_chance,
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

        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }

        let hp_threshold = percent_of(target_ref.hp().max, self.effect_hp_percent_threshold);
        if target_ref.hp().current - *damage <= hp_threshold {
            let lethal_damage = target_ref.hp().current;
            ctx.deal_damage(caster, target, lethal_damage, 0, AttackType::Item);
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::AD]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

#[derive(Clone, Debug)]
pub struct Collector {
    price: usize,
    attack: i32,
    crit_chance: i32,
    defence_penetration: usize,
    effect_hp_percent_threshold: f64,
}

impl Default for Collector {
    fn default() -> Self {
        Self {
            price: 1400,
            attack: 55,
            crit_chance: 20,
            defence_penetration: 10,
            effect_hp_percent_threshold: 8.0,
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
            defence_penetration: cfg.defence_penetration.unwrap_or(d.defence_penetration),
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
        "t9_9"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["soldiers_longsword".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_collector".to_string()]
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
        caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageType,
    ) {
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
        vec![ItemTag::AD, ItemTag::DefensePenetration]
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
    defence_penetration: usize,
    effect_hp_percent_threshold: f64,
}

impl Default for RadiantCollector {
    fn default() -> Self {
        Self {
            price: 2000,
            attack: 105,
            crit_chance: 25,
            defence_penetration: 10,
            effect_hp_percent_threshold: 8.0,
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
            defence_penetration: cfg.defence_penetration.unwrap_or(d.defence_penetration),
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
        "t10_0"
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
            defence_penetration: self.defence_penetration,
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
        vec![ItemTag::AD, ItemTag::DefensePenetration]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

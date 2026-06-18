use arrayvec::ArrayString;
use mod_api::*;

use crate::config::ItemConfig;

#[derive(Clone, Debug)]
pub struct MortalReminder {
    price: usize,
    attack: i32,
    defence_penetration: usize,
    effect_heal_reduce: usize,
    effect_duration_seconds: usize,
}

impl Default for MortalReminder {
    fn default() -> Self {
        Self {
            price: 1400,
            attack: 55,
            defence_penetration: 20,
            effect_heal_reduce: 40,
            effect_duration_seconds: 2,
        }
    }
}

impl MortalReminder {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            defence_penetration: cfg.defence_penetration.unwrap_or(d.defence_penetration),
            effect_heal_reduce: cfg.effect_heal_reduce.unwrap_or(d.effect_heal_reduce),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for MortalReminder {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "mortal_reminder"
    }

    fn icon(&self) -> &str {
        "t7_8"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["executioners_calling".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_mortal_reminder".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            defence_penetration: self.defence_penetration,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };
        let already_reduced = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "40_percent_heal_cut");
        if !already_reduced {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_duration_seconds * 60,
                    },
                    heal_reduce: self.effect_heal_reduce,
                    name: ArrayString::try_from("40_percent_heal_cut").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::AD,
            ItemTag::DefensePenetration,
            ItemTag::HealReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

#[derive(Clone, Debug)]
pub struct RadiantMortalReminder {
    price: usize,
    attack: i32,
    defence_penetration: usize,
    crit_chance: i32,
    effect_heal_reduce: usize,
    effect_duration_seconds: usize,
}

impl Default for RadiantMortalReminder {
    fn default() -> Self {
        Self {
            price: 2000,
            attack: 70,
            defence_penetration: 30,
            crit_chance: 0,
            effect_heal_reduce: 40,
            effect_duration_seconds: 2,
        }
    }
}

impl RadiantMortalReminder {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            attack: cfg.attack.unwrap_or(d.attack),
            defence_penetration: cfg.defence_penetration.unwrap_or(d.defence_penetration),
            crit_chance: cfg.crit_chance.unwrap_or(d.crit_chance),
            effect_heal_reduce: cfg.effect_heal_reduce.unwrap_or(d.effect_heal_reduce),
            effect_duration_seconds: cfg
                .effect_duration_seconds
                .unwrap_or(d.effect_duration_seconds),
        }
    }
}

impl ModItemInfo for RadiantMortalReminder {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_mortal_reminder"
    }

    fn icon(&self) -> &str {
        "t7_9"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["mortal_reminder".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            attack: self.attack,
            defence_penetration: self.defence_penetration,
            crit_chance: self.crit_chance,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageType,
    ) {
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };
        let already_reduced = (0..entity_ref.buff_count())
            .any(|i| entity_ref.buff_at(i).name.as_str() == "40_percent_heal_cut");
        if !already_reduced {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: self.effect_duration_seconds * 60,
                    },
                    heal_reduce: self.effect_heal_reduce,
                    name: ArrayString::try_from("40_percent_heal_cut").unwrap(),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::AD,
            ItemTag::DefensePenetration,
            ItemTag::HealReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::AD
    }
}

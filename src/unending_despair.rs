use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

#[derive(Clone, Debug)]
pub struct UnendingDespair {
    price: usize,
    hp: i32,
    defence: i32,
    effect_bonus_flat_heal: i32,
    effect_caster_hp_percent_heal: f64,
}

impl Default for UnendingDespair {
    fn default() -> Self {
        Self {
            price: 1450,
            hp: 450,
            defence: 30,
            effect_bonus_flat_heal: 10,
            effect_caster_hp_percent_heal: 1.0,
        }
    }
}

impl UnendingDespair {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            defence: cfg.defence.unwrap_or(d.defence),
            effect_bonus_flat_heal: cfg
                .effect_bonus_flat_heal
                .unwrap_or(d.effect_bonus_flat_heal),
            effect_caster_hp_percent_heal: cfg
                .effect_caster_hp_percent_heal
                .unwrap_or(d.effect_caster_hp_percent_heal),
        }
    }
}

impl ModItemInfo for UnendingDespair {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "unending_despair"
    }

    fn icon(&self) -> &str {
        "t9_1"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec![
            "steel_armor".to_string(),
            "ring_of_reincarnation".to_string(),
        ]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_unending_despair".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            defence: self.defence,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, _target: usize) {
        let heal_amount = self.effect_bonus_flat_heal as usize
            + ctx
                .get_entity(caster)
                .map(|e| percent_of(e.hp().max, self.effect_caster_hp_percent_heal))
                .unwrap_or(0);
        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time { tick: 60 },
                hp_regen: heal_amount as i32,
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense, ItemTag::Vamp]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Clone, Debug)]
pub struct RadiantUnendingDespair {
    price: usize,
    hp: i32,
    defence: i32,
    effect_bonus_flat_heal: i32,
    effect_caster_hp_percent_heal: f64,
}

impl Default for RadiantUnendingDespair {
    fn default() -> Self {
        Self {
            price: 2100,
            hp: 700,
            defence: 50,
            effect_bonus_flat_heal: 25,
            effect_caster_hp_percent_heal: 3.0,
        }
    }
}

impl RadiantUnendingDespair {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            defence: cfg.defence.unwrap_or(d.defence),
            effect_bonus_flat_heal: cfg
                .effect_bonus_flat_heal
                .unwrap_or(d.effect_bonus_flat_heal),
            effect_caster_hp_percent_heal: cfg
                .effect_caster_hp_percent_heal
                .unwrap_or(d.effect_caster_hp_percent_heal),
        }
    }
}

impl ModItemInfo for RadiantUnendingDespair {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_unending_despair"
    }

    fn icon(&self) -> &str {
        "t9_2"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["unending_despair".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            defence: self.defence,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, _target: usize) {
        let heal_amount = self.effect_bonus_flat_heal as usize
            + ctx
                .get_entity(caster)
                .map(|e| percent_of(e.hp().max, self.effect_caster_hp_percent_heal))
                .unwrap_or(0);
        ctx.add_buff(
            caster,
            BuffState {
                duration: BuffType::Time { tick: 60 },
                hp_regen: heal_amount as i32,
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense, ItemTag::Vamp]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

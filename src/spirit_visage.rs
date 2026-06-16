use mod_api::*;

use crate::config::ItemConfig;
use crate::percent_of;

#[derive(Clone, Debug)]
pub struct SpiritVisage {
    price: usize,
    hp: i32,
    magic_resistance: i32,
    effect_heal_mult: f64,
}

impl Default for SpiritVisage {
    fn default() -> Self {
        Self {
            price: 1400,
            hp: 400,
            magic_resistance: 100,
            effect_heal_mult: 20.0,
        }
    }
}

impl SpiritVisage {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            magic_resistance: cfg.magic_resistance.unwrap_or(d.magic_resistance),
            effect_heal_mult: cfg.effect_heal_mult.unwrap_or(d.effect_heal_mult),
        }
    }
}

impl ModItemInfo for SpiritVisage {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "spirit_visage"
    }

    fn icon(&self) -> &str {
        "t8_9"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        3
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["hardened_heart".to_string(), "dusk_raven".to_string()]
    }

    fn next_tier(&self) -> Vec<String> {
        vec!["radiant_spirit_visage".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            magic_resistance: self.magic_resistance,
            ..Default::default()
        }
    }

    fn on_healed(&mut self, ctx: &mut GameCtx, _caster: Option<usize>, entity: usize, heal: usize) {
        let Some(_entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        let bonus_heal = percent_of(heal, self.effect_heal_mult);
        ctx.add_buff(
            entity,
            BuffState {
                duration: BuffType::Time { tick: 60 },
                hp_regen: bonus_heal as i32,
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::MagicResistance]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::MagicResistance
    }
}

#[derive(Clone, Debug)]
pub struct RadiantSpiritVisage {
    price: usize,
    hp: i32,
    magic_resistance: i32,
    effect_heal_mult: f64,
}

impl Default for RadiantSpiritVisage {
    fn default() -> Self {
        Self {
            price: 1900,
            hp: 600,
            magic_resistance: 150,
            effect_heal_mult: 20.0,
        }
    }
}

impl RadiantSpiritVisage {
    pub fn with_config(cfg: &ItemConfig) -> Self {
        let d = Self::default();
        Self {
            price: cfg.price.unwrap_or(d.price),
            hp: cfg.hp.unwrap_or(d.hp),
            magic_resistance: cfg.magic_resistance.unwrap_or(d.magic_resistance),
            effect_heal_mult: cfg.effect_heal_mult.unwrap_or(d.effect_heal_mult),
        }
    }
}

impl ModItemInfo for RadiantSpiritVisage {
    fn clone_box(&self) -> Box<dyn ModItemInfo> {
        Box::new(self.clone())
    }

    fn key(&self) -> &str {
        "radiant_spirit_visage"
    }

    fn icon(&self) -> &str {
        "t9_0"
    }

    fn price(&self) -> usize {
        self.price
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["spirit_visage".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: self.hp,
            magic_resistance: self.magic_resistance,
            ..Default::default()
        }
    }

    fn on_healed(&mut self, ctx: &mut GameCtx, _caster: Option<usize>, entity: usize, heal: usize) {
        let Some(_entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        let bonus_heal = percent_of(heal, self.effect_heal_mult);
        ctx.add_buff(
            entity,
            BuffState {
                duration: BuffType::Time { tick: 60 },
                hp_regen: bonus_heal as i32,
                ..Default::default()
            },
        );
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::MagicResistance]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::MagicResistance
    }
}

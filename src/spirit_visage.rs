use mod_api::*;

use crate::percent_of;

const SPIRIT_VISAGE_HEAL_MULT: f64 = 20.0;

#[derive(Default, Clone, Debug)]
pub struct SpiritVisage;

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
        1400
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
            hp: 400,
            magic_resistance: 50,
            ..Default::default()
        }
    }

    fn on_healed(&mut self, ctx: &mut GameCtx, _caster: Option<usize>, entity: usize, heal: usize) {
        // Healing you receive is increased by 20%
        let bonus_heal = percent_of(heal, SPIRIT_VISAGE_HEAL_MULT);
        ctx.heal(entity, entity, bonus_heal);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::MagicResistance]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

#[derive(Default, Clone, Debug)]
pub struct RadiantSpiritVisage;

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
        1900
    }

    fn tier(&self) -> usize {
        4
    }

    fn previous_tier(&self) -> Vec<String> {
        vec!["spirit_visage".to_string()]
    }

    fn stat(&self) -> BuffState {
        BuffState {
            hp: 600,
            magic_resistance: 100,
            ..Default::default()
        }
    }

    fn on_healed(&mut self, ctx: &mut GameCtx, _caster: Option<usize>, entity: usize, heal: usize) {
        // Healing you receive is increased by 20%
        let bonus_heal = percent_of(heal, SPIRIT_VISAGE_HEAL_MULT);
        ctx.heal(entity, entity, bonus_heal);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::MagicResistance]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

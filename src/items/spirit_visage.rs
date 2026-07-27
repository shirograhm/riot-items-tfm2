use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ItemMeta};

#[derive(Clone, Debug)]
pub struct SpiritVisage {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    magic_resistance: i32,
    effect_heal_mult: f64,
}

impl SpiritVisage {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "spirit_visage",
                &["hardened_heart", "dusk_raven"],
                &["radiant_spirit_visage"],
            ),
            price: 1400,
            hp: 400,
            magic_resistance: 100,
            effect_heal_mult: 20.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_spirit_visage", &["spirit_visage"]),
            price: 1900,
            hp: 600,
            magic_resistance: 150,
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
        apply_config!(self, cfg, [price, hp, magic_resistance, effect_heal_mult]);
        self
    }
}

impl Default for SpiritVisage {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for SpiritVisage {
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
        ctx.heal(entity, entity, bonus_heal);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::MagicResistance]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::MagicResistance
    }
}

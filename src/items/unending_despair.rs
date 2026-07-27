use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ItemMeta};

#[derive(Clone, Debug)]
pub struct UnendingDespair {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    defence: i32,
    effect_bonus_flat_heal: i32,
    effect_caster_hp_percent_heal: f64,
}

impl UnendingDespair {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "unending_despair",
                &["ring_of_reincarnation"],
                &["radiant_unending_despair"],
            ),
            price: 1450,
            hp: 450,
            defence: 30,
            effect_bonus_flat_heal: 35,
            effect_caster_hp_percent_heal: 1.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_unending_despair", &["unending_despair"]),
            price: 2100,
            hp: 700,
            defence: 50,
            effect_bonus_flat_heal: 50,
            effect_caster_hp_percent_heal: 2.5,
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
                hp,
                defence,
                effect_bonus_flat_heal,
                effect_caster_hp_percent_heal
            ]
        );
        self
    }
}

impl Default for UnendingDespair {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for UnendingDespair {
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
            defence: self.defence,
            ..Default::default()
        }
    }

    fn on_skill_hit(&mut self, ctx: &mut GameCtx, _rng_seed: u64, caster: usize, _target: usize) {
        let Some(entity_ref) = ctx.get_entity(caster) else {
            return;
        };
        let heal_amount = self.effect_bonus_flat_heal as usize
            + percent_of(entity_ref.hp().max, self.effect_caster_hp_percent_heal);

        ctx.heal(caster, caster, heal_amount);
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::Defense, ItemTag::Vamp]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Hp
    }
}

use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, buff_name, has_buff, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct Morellonomicon {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_heal_reduce: usize,
    effect_duration_seconds: f64,
}

impl Morellonomicon {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "morellonomicon",
                &["oblivion_orb"],
                &["radiant_morellonomicon"],
            ),
            price: 1300,
            hp: 200,
            magic_power: 120,
            skill_cooldown_mult: 10,
            effect_heal_reduce: 40,
            effect_duration_seconds: 2.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_morellonomicon", &["morellonomicon"]),
            price: 1850,
            hp: 350,
            magic_power: 190,
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
                magic_power,
                skill_cooldown_mult,
                effect_heal_reduce,
                effect_duration_seconds
            ]
        );
        self
    }
}

impl Default for Morellonomicon {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for Morellonomicon {
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
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_attack(
        &mut self,
        ctx: &mut GameCtx,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        damage_type: DamageType,
    ) {
        let Some(entity_ref) = ctx.get_entity(target) else {
            return;
        };

        if damage_type != DamageType::AP {
            return;
        }

        let already_reduced = has_buff(&entity_ref, "40_percent_heal_cut");
        if !already_reduced {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: ticks(self.effect_duration_seconds),
                    },
                    heal_reduce: self.effect_heal_reduce,
                    name: buff_name("40_percent_heal_cut"),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![
            ItemTag::HP,
            ItemTag::AP,
            ItemTag::CooltimeReduce,
            ItemTag::HealReduce,
        ]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

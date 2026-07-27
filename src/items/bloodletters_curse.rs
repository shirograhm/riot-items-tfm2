use mod_api::*;

use crate::config::ItemConfig;
use crate::{apply_config, buff_name, buff_stacks, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct BloodlettersCurse {
    meta: ItemMeta,
    price: usize,
    magic_power: i32,
    hp: i32,
    skill_cooldown_mult: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
    effect_percent_mr_shred: i32,
}

impl BloodlettersCurse {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "bloodletters_curse",
                &["haunting_guise"],
                &["radiant_bloodletters_curse"],
            ),
            price: 1500,
            magic_power: 110,
            hp: 300,
            skill_cooldown_mult: 5,
            effect_max_stacks: 5,
            effect_duration_seconds: 6.0,
            effect_percent_mr_shred: 6,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_bloodletters_curse", &["bloodletters_curse"]),
            price: 2200,
            magic_power: 180,
            hp: 500,
            skill_cooldown_mult: 10,
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
                magic_power,
                hp,
                skill_cooldown_mult,
                effect_max_stacks,
                effect_duration_seconds,
                effect_percent_mr_shred
            ]
        );
        self
    }
}

impl Default for BloodlettersCurse {
    fn default() -> Self {
        Self::base()
    }
}

impl ModItemInfo for BloodlettersCurse {
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
            magic_power: self.magic_power,
            hp: self.hp,
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
        if !entity_ref.is_champion() {
            return;
        }

        if damage_type != DamageType::AP {
            return;
        }

        let stack_count = buff_stacks(&entity_ref, "bloodletters_curse_mr_shred");
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                target,
                BuffState {
                    duration: BuffType::Time {
                        tick: ticks(self.effect_duration_seconds),
                    },
                    magic_resistance_mult: -self.effect_percent_mr_shred,
                    name: buff_name("bloodletters_curse_mr_shred"),
                    ..Default::default()
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTag> {
        vec![ItemTag::HP, ItemTag::AP]
    }

    fn category(&self) -> ItemCategory {
        ItemCategory::Magic
    }
}

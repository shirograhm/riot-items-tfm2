use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, buff_stacks, ticks, ItemMeta};

#[derive(Clone, Debug)]
pub struct JakshoTheProtean {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant
    // items keep independent stacks.
    stack_buff: &'static str,
    price: usize,
    hp: i32,
    defence: i32,
    magic_resistance: i32,
    effect_stack_defence_mult: i32,
    effect_stack_magic_resistance_mult: i32,
    effect_max_stacks: usize,
    effect_duration_seconds: f64,
}

impl JakshoTheProtean {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "jaksho_the_protean",
                &["aegis_of_the_legion"],
                &["radiant_jaksho_the_protean"],
            ),
            stack_buff: "jaksho_the_protean_stack",
            price: 1400,
            hp: 300,
            defence: 40,
            magic_resistance: 65,
            effect_stack_defence_mult: 6,
            effect_stack_magic_resistance_mult: 6,
            effect_max_stacks: 4,
            effect_duration_seconds: 4.0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_jaksho_the_protean", &["jaksho_the_protean"]),
            stack_buff: "radiant_jaksho_the_protean_stack",
            price: 2000,
            hp: 550,
            defence: 65,
            effect_stack_defence_mult: 10,
            effect_stack_magic_resistance_mult: 10,
            effect_max_stacks: 4,
            effect_duration_seconds: 4.0,
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
                magic_resistance,
                effect_stack_defence_mult,
                effect_stack_magic_resistance_mult,
                effect_max_stacks,
                effect_duration_seconds
            ]
        );
        self
    }
}

impl Default for JakshoTheProtean {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for JakshoTheProtean {
    fn clone_box(&self) -> Box<dyn StableItem> {
        Box::new(self.clone())
    }

    fn key(&self) -> String {
        self.meta.key.to_string()
    }

    fn icon(&self) -> String {
        self.meta.key.to_string()
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

    fn stat(&self) -> BuffV1 {
        BuffV1 {
            hp: self.hp,
            defence: self.defence,
            magic_resistance: self.magic_resistance,
            ..Default::default()
        }
    }

    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        attacker: usize,
        _damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        let Some(attacker_ref) = ctx.get_entity(attacker) else {
            return;
        };
        if !attacker_ref.is_champion() {
            return;
        }

        let stack_count = buff_stacks(&entity_ref, self.stack_buff);
        if stack_count < self.effect_max_stacks {
            ctx.add_buff(
                entity,
                &BuffV1 {
                    defence_mult: self.effect_stack_defence_mult,
                    magic_resistance_mult: self.effect_stack_magic_resistance_mult,
                    ..BuffV1::timed(self.stack_buff, ticks(self.effect_duration_seconds))
                },
            );
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Defense,
            ItemTagV1::MagicResistance,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}

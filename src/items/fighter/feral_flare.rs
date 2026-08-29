use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ItemMeta};

#[derive(Clone, Debug)]
pub struct FeralFlare {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    defence: i32,
    effect_bonus_magic_damage: usize,
    effect_stack_magic_damage: usize,
    effect_bonus_flat_heal: i32,
    effect_max_stacks: usize,
    effect_minion_percent: f64,
    // Non-vital stats (internals)
    feral_stacks: usize,
}

impl FeralFlare {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "feral_flare",
                &["hearthbound_axe"],
                &["radiant_feral_flare"],
            ),
            price: 1400,
            attack: 45,
            attack_speed_mult: 20,
            defence: 20,
            effect_bonus_magic_damage: 40,
            effect_stack_magic_damage: 1,
            effect_bonus_flat_heal: 15,
            effect_max_stacks: 30,
            effect_minion_percent: 300.0,
            feral_stacks: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_feral_flare", &["feral_flare"]),
            price: 2000,
            attack: 55,
            attack_speed_mult: 40,
            defence: 40,
            effect_bonus_magic_damage: 40,
            effect_stack_magic_damage: 1,
            effect_bonus_flat_heal: 15,
            effect_max_stacks: 30,
            effect_minion_percent: 300.0,
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
                attack,
                attack_speed_mult,
                defence,
                effect_bonus_magic_damage,
                effect_stack_magic_damage,
                effect_bonus_flat_heal,
                effect_max_stacks,
                effect_minion_percent
            ]
        );
        self
    }

    fn add_stack(&mut self) {
        if self.feral_stacks < self.effect_max_stacks {
            self.feral_stacks += 1;
        }
    }
}

impl Default for FeralFlare {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for FeralFlare {
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
            attack: self.attack,
            attack_speed_mult: self.attack_speed_mult,
            defence: self.defence,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.feral_stacks = 0;
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }
        let boosted = !target_ref.is_champion();

        let damage =
            self.effect_bonus_magic_damage + self.effect_stack_magic_damage * self.feral_stacks;
        let heal = self.effect_bonus_flat_heal.max(0) as usize;
        let (damage, heal) = if boosted {
            (
                percent_of(damage, self.effect_minion_percent),
                percent_of(heal, self.effect_minion_percent),
            )
        } else {
            (damage, heal)
        };

        ctx.deal_damage(caster, target, 0, damage, AttackTypeV1::Item);
        ctx.heal(caster, caster, heal);
    }

    fn on_kill(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _player: usize,
        _entity: usize,
        victim: usize,
    ) {
        if ctx.get_entity(victim).is_some_and(|v| v.is_champion()) {
            self.add_stack();
        }
    }

    fn on_assist(&mut self, _ctx: &mut StableSim<'_>, _player: usize, _entity: usize) {
        self.add_stack();
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::AttackSpeed, ItemTagV1::Defense]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}

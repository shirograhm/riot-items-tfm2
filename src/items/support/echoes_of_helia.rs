use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta};

#[derive(Clone, Debug)]
pub struct EchoesOfHelia {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_damage_conversion: f64,
    effect_min_stacks: usize,
    effect_max_stacks: usize,
    charge_stored: usize,
}

impl EchoesOfHelia {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "echoes_of_helia",
                &["bandleglass_mirror", "forbidden_idol"],
                &["radiant_echoes_of_helia"],
            ),
            price: 1100,
            hp: 250,
            hp_regen: 4,
            magic_power: 45,
            skill_cooldown_mult: 15,
            effect_damage_conversion: 30.0,
            effect_min_stacks: 130,
            effect_max_stacks: 350,
            // Non-vital stats (internals)
            charge_stored: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_echoes_of_helia", &["echoes_of_helia"]),
            price: 1500,
            hp: 450,
            hp_regen: 6,
            magic_power: 65,
            skill_cooldown_mult: 20,
            effect_damage_conversion: 30.0,
            effect_min_stacks: 130,
            effect_max_stacks: 350,
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
                hp_regen,
                magic_power,
                skill_cooldown_mult,
                effect_damage_conversion,
                effect_min_stacks,
                effect_max_stacks
            ]
        );
        self
    }

    pub fn save_charges(&mut self, level: usize, damage: f64) {
        let stack_gain = (damage * (self.effect_damage_conversion / 100.0)) as usize;
        let limit_per_level = (self.effect_max_stacks - self.effect_min_stacks) as f64 / 11.0;
        let max_limit = self.effect_min_stacks + (level - 1) * limit_per_level.round() as usize;

        if self.charge_stored + stack_gain > max_limit {
            self.charge_stored = max_limit;
        } else {
            self.charge_stored += stack_gain;
        }
    }
}

impl Default for EchoesOfHelia {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for EchoesOfHelia {
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
            hp_regen: self.hp_regen,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        _attacker: usize,
        damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        self.save_charges(entity_ref.level(), damage as f64);
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        _target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        self.save_charges(caster_ref.level(), *damage as f64);
    }

    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };

        if is_ally && target != caster && target_ref.is_champion() && target_ref.is_alive() {
            ctx.heal(caster, target, self.charge_stored);
        }

        self.charge_stored = 0;
    }

    /// Stored charges follow the item through the Radiant upgrade; the next
    /// `save_charges` clamps them back down if the wielder's level allows less.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.charge_stored as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.charge_stored = carry as usize;
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::HpRegen,
            ItemTagV1::Ap,
            ItemTagV1::CooltimeReduce,
            ItemTagV1::Vamp,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}

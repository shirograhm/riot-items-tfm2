use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ItemMeta, ProcQueue, Spellblade, SpellbladeBonus};

#[derive(Clone, Debug)]
pub struct DuskAndDawn {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    magic_power: i32,
    attack_speed_mult: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ap_percent_damage: f64,
    effect_caster_ap_percent_heal: f64,
    effect_caster_hp_percent_heal: f64,
    effect_cooldown_seconds: f64,
    spellblade: Spellblade,
    procs: ProcQueue,
}

impl DuskAndDawn {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "dusk_and_dawn",
                &["sheen", "haunting_guise"],
                &["radiant_dusk_and_dawn"],
            ),
            price: 700,
            hp: 100,
            magic_power: 30,
            attack_speed_mult: 25,
            skill_cooldown_mult: 10,
            effect_bonus_flat_damage: 85,
            effect_ap_percent_damage: 15.0,
            effect_caster_ap_percent_heal: 10.0,
            effect_caster_hp_percent_heal: 2.5,
            effect_cooldown_seconds: 1.5,
            // Non-vital stats (internals)
            spellblade: Spellblade::default(),
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_dusk_and_dawn", &["dusk_and_dawn"]),
            price: 1000,
            hp: 150,
            magic_power: 75,
            attack_speed_mult: 25,
            skill_cooldown_mult: 10,
            effect_bonus_flat_damage: 85,
            effect_ap_percent_damage: 15.0,
            effect_caster_ap_percent_heal: 10.0,
            effect_caster_hp_percent_heal: 2.5,
            effect_cooldown_seconds: 1.5,
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
                attack_speed_mult,
                skill_cooldown_mult,
                effect_bonus_flat_damage,
                effect_ap_percent_damage,
                effect_caster_ap_percent_heal,
                effect_caster_hp_percent_heal,
                effect_cooldown_seconds
            ]
        );
        self
    }

    /// What Spellblade adds to the empowered attack.
    pub(crate) fn spellblade_bonus(&self) -> SpellbladeBonus {
        SpellbladeBonus {
            flat: self.effect_bonus_flat_damage,
            ap_percent: self.effect_ap_percent_damage,
            ..Default::default()
        }
    }
}

impl Default for DuskAndDawn {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for DuskAndDawn {
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
            magic_power: self.magic_power,
            attack_speed_mult: self.attack_speed_mult,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.spellblade.reset();
        self.procs.clear();
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        is_crit: bool,
    ) {
        if !self.spellblade.is_ready() || attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        if !self.spellblade.wins(ctx, caster, self.meta.key) {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };

        let bonus_damage = self.spellblade_bonus().of(&caster_ref);
        let heal_amount = percent_of(
            caster_ref.stat().magic_power,
            self.effect_caster_ap_percent_heal,
        ) + percent_of(caster_ref.hp().1, self.effect_caster_hp_percent_heal);

        self.procs.on_hit_magic(ctx, target, damage, damage_type, is_crit, bonus_damage);
        ctx.heal(caster, caster, heal_amount);

        self.spellblade
            .spend(ctx, caster, target, self.effect_cooldown_seconds);
    }

    /// Lands the Spellblade damage whose delay has run out, and watches for
    /// the cast that readies the next one.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
        self.spellblade.update(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Ap,
            ItemTagV1::AttackSpeed,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}

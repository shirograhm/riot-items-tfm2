use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, ItemMeta, ProcQueue, Spellblade, SpellbladeBonus};

#[derive(Clone, Debug)]
pub struct LichBane {
    meta: ItemMeta,
    price: usize,
    magic_power: i32,
    attack_speed_mult: i32,
    skill_cooldown_mult: i32,
    effect_bonus_flat_damage: usize,
    effect_ap_percent_damage: f64,
    effect_cooldown_seconds: f64,
    spellblade: Spellblade,
    procs: ProcQueue,
}

impl LichBane {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "lich_bane",
                &["sheen", "hextech_alternator"],
                &["radiant_lich_bane"],
            ),
            price: 750,
            magic_power: 50,
            attack_speed_mult: 20,
            skill_cooldown_mult: 10,
            effect_bonus_flat_damage: 105,
            effect_ap_percent_damage: 30.0,
            effect_cooldown_seconds: 1.5,
            // Non-vital stats (internals)
            spellblade: Spellblade::default(),
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_lich_bane", &["lich_bane"]),
            price: 1050,
            magic_power: 90,
            attack_speed_mult: 25,
            skill_cooldown_mult: 15,
            effect_bonus_flat_damage: 105,
            effect_ap_percent_damage: 45.0,
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
                magic_power,
                attack_speed_mult,
                skill_cooldown_mult,
                effect_bonus_flat_damage,
                effect_ap_percent_damage,
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

impl Default for LichBane {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for LichBane {
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

        self.procs
            .on_hit_magic(ctx, target, damage, damage_type, is_crit, bonus_damage);
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
            ItemTagV1::Ap,
            ItemTagV1::AttackSpeed,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}

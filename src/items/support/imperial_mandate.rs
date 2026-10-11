use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, ticks, ImmobilizeWatch, ItemMeta};

#[derive(Clone, Debug)]
pub struct ImperialMandate {
    meta: ItemMeta,
    // Shared by both variants: Vulnerable is a state on the target, and the
    // two variants grant the same amount, so a second carrier refreshes it
    // rather than doubling it. The name is also the `view_buffs` binding in
    // `view/effects.view_effects` that draws the mini flag over the target;
    // rename both together or the flag stops showing.
    vulnerable_buff: &'static str,
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_damaged_amplify: usize,
    effect_duration_seconds: f64,
    // Non-vital stats (internals)
    watch: ImmobilizeWatch,
}

impl ImperialMandate {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "imperial_mandate",
                &["bandleglass_mirror"],
                &["radiant_imperial_mandate"],
            ),
            vulnerable_buff: "imperial_mandate_vulnerable",
            price: 550,
            hp: 100,
            hp_regen: 1,
            magic_power: 25,
            skill_cooldown_mult: 15,
            effect_damaged_amplify: 9,
            effect_duration_seconds: 3.0,
            // Non-vital stats (internals)
            watch: ImmobilizeWatch::default(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_imperial_mandate", &["imperial_mandate"]),
            price: 750,
            hp: 150,
            hp_regen: 2,
            magic_power: 40,
            skill_cooldown_mult: 20,
            effect_damaged_amplify: 9,
            effect_duration_seconds: 3.0,
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
                effect_damaged_amplify,
                effect_duration_seconds
            ]
        );
        self
    }

    fn mark(&self, ctx: &mut StableSim<'_>, target: usize) {
        refresh_buff(
            ctx,
            target,
            self.vulnerable_buff,
            &BuffV1 {
                damaged_amplify: self.effect_damaged_amplify,
                ..BuffV1::timed(self.vulnerable_buff, ticks(self.effect_duration_seconds))
            },
        );
    }
}

impl Default for ImperialMandate {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for ImperialMandate {
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
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.watch.reset();
    }

    // Command: immobilizing an enemy champion marks them. What counts as the
    // carrier immobilizing someone is `ImmobilizeWatch`'s to say: a skill hit
    // that leaves its target newly immobilized, there and then or a moment
    // later, and any new taunt onto the carrier.
    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _caster: usize,
        target: usize,
        is_ally: bool,
    ) {
        if self.watch.skill_hit(ctx, target, is_ally) {
            self.mark(ctx, target);
        }
    }

    // A skill's hit again, as `on_attack` tells of it, a tick of its damage
    // over time included: it is not known that `on_skill_hit` hears of every
    // one (see [`ImmobilizeWatch`]).
    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        _caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let ability = matches!(
            attack_type,
            AttackTypeV1::Skill | AttackTypeV1::Dot | AttackTypeV1::DotIgnoreShield
        );
        if ability && self.watch.skill_hit(ctx, target, false) {
            self.mark(ctx, target);
        }
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        for target in self.watch.update(ctx, player) {
            self.mark(ctx, target);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::HpRegen,
            ItemTagV1::Ap,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}

use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, heal_shield_power, is_monster, item_counts, percent_of, upgrade_carry, ItemMeta,
    ProcQueue,
};

// The upgrade line the stacks are noted under (`crate::upgrade_carry`), so
// they follow the carrier into the Radiant item.
const BASE_KEY: &str = "feral_flare";

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
    // What carries the stacks into an upgrade.
    carry: upgrade_carry::StackCarry,
    procs: ProcQueue,
}

impl FeralFlare {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                BASE_KEY,
                &["madreds_razors"],
                &["radiant_feral_flare"],
            ),
            price: 700,
            attack: 25,
            attack_speed_mult: 20,
            defence: 10,
            effect_bonus_magic_damage: 25,
            effect_stack_magic_damage: 1,
            effect_bonus_flat_heal: 10,
            effect_max_stacks: 30,
            effect_minion_percent: 150.0,
            feral_stacks: 0,
            carry: upgrade_carry::StackCarry::default(),
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_feral_flare", &[BASE_KEY]),
            price: 1000,
            attack: 30,
            attack_speed_mult: 40,
            defence: 20,
            effect_bonus_magic_damage: 25,
            effect_stack_magic_damage: 1,
            effect_bonus_flat_heal: 10,
            effect_max_stacks: 50,
            effect_minion_percent: 150.0,
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

    // Keeps the stacks where an upgrade can find them (`upgrade_carry::StackCarry`).
    fn carry_stacks(&mut self, ctx: &StableSim<'_>, player: usize) {
        self.carry.carry(
            BASE_KEY,
            &self.meta,
            ctx,
            player,
            &mut self.feral_stacks,
            self.effect_max_stacks,
        );
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

    // Feral stacks are permanent: they outlast a death, and a Radiant item
    // bought while dead takes them over here, before its first `update`,
    // which a dead carrier does not get.
    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.carry_stacks(ctx, player);
        self.procs.clear();
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

        // The stack count is already in `damage`, so the proc pays the Feral
        // bar as of the hit; the heal stays on the attack, landing on the
        // carrier.
        self.procs.push_magic(ctx, target, damage);
        // Sustain, not a cast heal: Heal and Shield Power leaves it alone.
        heal_shield_power::unraised(|| ctx.heal(caster, caster, heal));
    }

    // Lands the on-hit damage whose delay has run out.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.carry_stacks(ctx, player);
        // The Feral stacks, for the number on the item's icon.
        item_counts::note(ctx, player, self.meta.key, self.feral_stacks as u64);
        self.procs.update(ctx, player);
    }

    fn on_kill(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _player: usize,
        _entity: usize,
        victim: usize,
    ) {
        let Some(victim_ref) = ctx.get_entity(victim) else {
            return;
        };
        // Champion kills and monster kills both stack; minions do not, which is
        // what `is_monster` rules out on top of champions and towers.
        if victim_ref.is_champion() || is_monster(&victim_ref) {
            self.add_stack();
        }
    }

    fn on_assist(&mut self, _ctx: &mut StableSim<'_>, _player: usize, _entity: usize) {
        self.add_stack();
    }

    // Feral stacks survive the Radiant upgrade, clamped to the successor's cap.
    // The host (0.6.2) never calls these two (`crate::upgrade_carry`);
    // `carry_stacks` does the carrying.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.feral_stacks as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.feral_stacks = (carry as usize).min(self.effect_max_stacks);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::AttackSpeed, ItemTagV1::Defense]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}

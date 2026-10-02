use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, is_monster, percent_of, upgrade_carry, ItemMeta, ProcQueue};

/// The upgrade line the stacks are noted under (`crate::upgrade_carry`), so
/// they follow the carrier into the Radiant item.
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
    /// The stack count last noted for an upgrade to take over.
    noted_stacks: usize,
    /// Whether this instance has taken over the stacks of the item it replaced.
    inherited: bool,
    procs: ProcQueue,
}

impl FeralFlare {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                BASE_KEY,
                &["hearthbound_axe"],
                &["radiant_feral_flare"],
            ),
            price: 700,
            attack: 25,
            attack_speed_mult: 20,
            defence: 10,
            effect_bonus_magic_damage: 25,
            effect_stack_magic_damage: 1,
            effect_bonus_flat_heal: 10,
            effect_max_stacks: 50,
            effect_minion_percent: 150.0,
            feral_stacks: 0,
            noted_stacks: 0,
            inherited: false,
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

    /// Keeps the stacks where an upgrade can find them. The Radiant item
    /// arrives as a fresh instance: it takes over the base item's stacks once,
    /// clamped to its own ceiling, and every instance notes its count when it
    /// changes.
    fn carry_stacks(&mut self, ctx: &StableSim<'_>, player: usize) {
        if !std::mem::replace(&mut self.inherited, true) && self.meta.upgrades_from(BASE_KEY) {
            if let Some((_, stacks)) = upgrade_carry::latest(BASE_KEY, ctx, player) {
                let stacks = (stacks as usize).min(self.effect_max_stacks);
                self.feral_stacks = self.feral_stacks.max(stacks);
                self.noted_stacks = self.feral_stacks;
            }
        }
        if self.feral_stacks != self.noted_stacks {
            self.noted_stacks = self.feral_stacks;
            upgrade_carry::note(BASE_KEY, ctx, player, self.feral_stacks as u64);
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
        // The stacks start over each life, so there is nothing left to take over.
        self.inherited = true;
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

        // The stack count is already folded into `damage`, so the proc pays out
        // the Feral bar as it stood on the hit that earned it. The heal is left
        // on the attack itself: it lands on the carrier rather than the target,
        // so it has no number to collide with.
        self.procs.push_magic(ctx, target, damage);
        ctx.heal(caster, caster, heal);
    }

    /// Lands the on-hit damage whose delay has run out.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.carry_stacks(ctx, player);
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

    /// Feral stacks survive the Radiant upgrade, clamped to the successor's own
    /// ceiling in case the config gives the two variants different caps.
    ///
    /// The host of game 0.6.2 never calls these two (`crate::upgrade_carry`);
    /// `carry_stacks` does the carrying, and they stay for a host that does.
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

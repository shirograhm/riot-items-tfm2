use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, is_monster, item_counts, upgrade_carry, ItemMeta, Transmutation,
    TransmutationNumbers,
};

// Transmutation: Killing monsters grants 5 Ability Haste and 30 bonus health
// regeneration for 5 seconds. Additionally, monsters that damage you burn for
// 10% of their maximum health as true damage over 3 seconds, capped at 150
// damage per second. (Philosopher's Medallion has it too: `crate::Transmutation`.)
//
// Cinderhulk: Gain 1% maximum health for each champion takedown and monster
// killed, up to 15% (Radiant: up to 25%).

// The upgrade line the stacks are noted under (`crate::upgrade_carry`), so
// they follow the carrier into the Radiant item.
const BASE_KEY: &str = "philosophers_stone";

// The maximum health Cinderhulk has granted, worn by the carrier.
const CINDERHULK_BUFF: &str = "philosophers_stone_cinderhulk";

#[derive(Clone, Debug)]
pub struct PhilosophersStone {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    defence: i32,
    magic_resistance: i32,
    effect_skill_cooldown_mult: i32,
    effect_bonus_hp_regen: i32,
    effect_duration_seconds: f64,
    effect_hp_percent_damage: f64,
    effect_burn_duration_seconds: f64,
    effect_burn_damage_cap: usize,
    effect_stack_hp_mult: i32,
    effect_max_stacks: usize,
    // Non-vital stats (internals)
    transmutation: Transmutation,
    cinderhulk_stacks: usize,
    // What carries the stacks into an upgrade.
    carry: upgrade_carry::StackCarry,
}

impl PhilosophersStone {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                BASE_KEY,
                &["bamis_cinder", "philosophers_medallion"],
                &["radiant_philosophers_stone"],
            ),
            price: 750,
            hp: 350,
            defence: 15,
            magic_resistance: 25,
            effect_skill_cooldown_mult: 5,
            effect_bonus_hp_regen: 30,
            effect_duration_seconds: 5.0,
            effect_hp_percent_damage: 10.0,
            effect_burn_duration_seconds: 3.0,
            effect_burn_damage_cap: 150,
            effect_stack_hp_mult: 1,
            effect_max_stacks: 15,
            // Non-vital stats (internals)
            transmutation: Transmutation::default(),
            cinderhulk_stacks: 0,
            carry: upgrade_carry::StackCarry::default(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_philosophers_stone", &[BASE_KEY]),
            price: 1050,
            hp: 450,
            defence: 30,
            magic_resistance: 45,
            effect_skill_cooldown_mult: 5,
            effect_bonus_hp_regen: 30,
            effect_duration_seconds: 5.0,
            effect_hp_percent_damage: 10.0,
            effect_burn_duration_seconds: 3.0,
            effect_burn_damage_cap: 150,
            effect_stack_hp_mult: 1,
            effect_max_stacks: 25,
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
                effect_skill_cooldown_mult,
                effect_bonus_hp_regen,
                effect_duration_seconds,
                effect_hp_percent_damage,
                effect_burn_duration_seconds,
                effect_burn_damage_cap,
                effect_stack_hp_mult,
                effect_max_stacks
            ]
        );
        self
    }

    fn transmutation_numbers(&self) -> TransmutationNumbers {
        TransmutationNumbers {
            haste: self.effect_skill_cooldown_mult,
            hp_regen: self.effect_bonus_hp_regen,
            seconds: self.effect_duration_seconds,
            burn_percent: self.effect_hp_percent_damage,
            burn_seconds: self.effect_burn_duration_seconds,
            burn_cap_per_second: self.effect_burn_damage_cap,
        }
    }

    // Cinderhulk: one more capped step of maximum health. The buff carries the
    // step since same-name buffs stack, and swapping a total would dip maximum
    // health mid-fight.
    fn grow(&mut self, ctx: &mut StableSim<'_>, entity: usize) {
        if self.cinderhulk_stacks >= self.effect_max_stacks {
            return;
        }
        self.cinderhulk_stacks += 1;
        ctx.add_buff(
            entity,
            &BuffV1 {
                hp_mult: self.effect_stack_hp_mult,
                ..BuffV1::named(CINDERHULK_BUFF)
            },
        );
    }

    // Keeps the stacks where an upgrade can find them
    // (`upgrade_carry::StackCarry`). Only the counter moves; the health already
    // gained is on the champion as `cinderhulk` buffs, re-applied on respawn.
    fn carry_stacks(&mut self, ctx: &StableSim<'_>, player: usize) {
        self.carry.carry(
            BASE_KEY,
            &self.meta,
            ctx,
            player,
            &mut self.cinderhulk_stacks,
            self.effect_max_stacks,
        );
    }
}

impl Default for PhilosophersStone {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for PhilosophersStone {
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

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.transmutation.clear();
        self.carry_stacks(ctx, player);

        // Cinderhulk is permanent, so the health earned so far is re-applied
        // each spawn from the banked count. Same-name buffs stack and this one
        // carries the whole total, so the previous life's copies go first.
        if self.cinderhulk_stacks == 0 {
            return;
        }
        let Some(champion) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| c.id())
        else {
            return;
        };
        ctx.entity_remove_buff(champion, CINDERHULK_BUFF);
        ctx.add_buff(
            champion,
            &BuffV1 {
                hp_mult: self.cinderhulk_stacks as i32 * self.effect_stack_hp_mult,
                ..BuffV1::named(CINDERHULK_BUFF)
            },
        );
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.carry_stacks(ctx, player);
        // The maximum health Cinderhulk has granted, in percent, for the
        // number on the item's icon.
        let gained = self.cinderhulk_stacks as i32 * self.effect_stack_hp_mult;
        item_counts::note(ctx, player, self.meta.key, gained.max(0) as u64);
        self.transmutation.update(ctx, player);
    }

    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        _entity: usize,
        attacker: usize,
        damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let numbers = self.transmutation_numbers();
        self.transmutation.on_damaged(ctx, attacker, damage, &numbers);
    }

    fn on_kill(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        _player: usize,
        entity: usize,
        victim: usize,
    ) {
        let numbers = self.transmutation_numbers();
        self.transmutation.on_kill(ctx, entity, victim, &numbers);
        let Some(victim_ref) = ctx.get_entity(victim) else {
            return;
        };
        // Champion and monster kills grow; minions do not, which is what
        // `is_monster` rules out on top of champions and towers.
        if victim_ref.is_champion() || is_monster(&victim_ref) {
            self.grow(ctx, entity);
        }
    }

    fn on_assist(&mut self, ctx: &mut StableSim<'_>, _player: usize, entity: usize) {
        self.grow(ctx, entity);
    }

    // Cinderhulk stacks survive the Radiant upgrade, clamped to the successor's
    // cap. The host (0.6.2) never calls these two (`crate::upgrade_carry`);
    // `carry_stacks` does the carrying.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.cinderhulk_stacks as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.cinderhulk_stacks = (carry as usize).min(self.effect_max_stacks);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Defense,
            ItemTagV1::MagicResistance,
            ItemTagV1::CooltimeReduce,
            ItemTagV1::HpRegen,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}

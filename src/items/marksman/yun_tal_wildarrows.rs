use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, item_counts, ticks, upgrade_carry, ItemMeta};

// The upgrade line the stacks are noted under (`crate::upgrade_carry`), so
// they follow the carrier into the Radiant item.
const BASE_KEY: &str = "yun_tal_wildarrows";

#[derive(Clone, Debug)]
pub struct YunTalWildarrows {
    meta: ItemMeta,
    yun_tal_practice_buff: &'static str,
    yun_tal_flurry_cooldown_buff: &'static str,
    yun_tal_flurry_buff: &'static str,
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    effect_stack_crit_chance: i32,
    effect_max_stacks: usize,
    // Not read from the config: the tooltip spells the count out ("Every
    // third basic attack").
    effect_attack_interval: usize,
    effect_flurry_attack_speed_mult: i32,
    effect_duration_seconds: f64,
    effect_cooldown_seconds: f64,
    accumulated_stacks: usize,
    // Basic attacks counted toward the next Practice stack.
    attack_count: usize,
    // What carries the stacks into an upgrade.
    carry: upgrade_carry::StackCarry,
    // The tick the last basic attack was counted on.
    counted_at: Option<usize>,
}

impl YunTalWildarrows {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                BASE_KEY,
                &["noonquiver"],
                &["radiant_yun_tal_wildarrows"],
            ),
            yun_tal_practice_buff: "yun_tal_practice",
            yun_tal_flurry_cooldown_buff: "yun_tal_flurry_cooldown",
            yun_tal_flurry_buff: "yun_tal_flurry",
            price: 750,
            attack: 35,
            attack_speed_mult: 20,
            effect_stack_crit_chance: 1,
            effect_max_stacks: 25,
            effect_attack_interval: 3,
            effect_flurry_attack_speed_mult: 30,
            effect_duration_seconds: 6.0,
            effect_cooldown_seconds: 15.0,
            // Non-vital stats (internals)
            accumulated_stacks: 0,
            attack_count: 0,
            carry: upgrade_carry::StackCarry::default(),
            counted_at: None,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_yun_tal_wildarrows", &[BASE_KEY]),
            yun_tal_practice_buff: "yun_tal_practice",
            yun_tal_flurry_cooldown_buff: "yun_tal_flurry_cooldown",
            yun_tal_flurry_buff: "yun_tal_flurry",
            price: 1100,
            attack: 40,
            attack_speed_mult: 50,
            effect_stack_crit_chance: 1,
            effect_max_stacks: 25,
            effect_attack_interval: 3,
            effect_flurry_attack_speed_mult: 30,
            effect_duration_seconds: 6.0,
            effect_cooldown_seconds: 15.0,
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
                effect_stack_crit_chance,
                effect_max_stacks,
                effect_flurry_attack_speed_mult,
                effect_duration_seconds,
                effect_cooldown_seconds
            ]
        );
        self
    }

    // Crit chance Practice grants at full stacks, for Smart Builds.
    pub fn max_passive_crit(&self) -> i32 {
        self.effect_stack_crit_chance * self.effect_max_stacks as i32
    }

    // Keeps the stacks where an upgrade can find them
    // (`upgrade_carry::StackCarry`). Only the counter moves; the crit already
    // earned is on the champion as `yun_tal_practice` buffs, re-applied on
    // respawn.
    fn carry_stacks(&mut self, ctx: &StableSim<'_>, player: usize) {
        self.carry.carry(
            BASE_KEY,
            &self.meta,
            ctx,
            player,
            &mut self.accumulated_stacks,
            self.effect_max_stacks,
        );
    }
}

impl Default for YunTalWildarrows {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for YunTalWildarrows {
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
            ..Default::default()
        }
    }

    // Practice: permanent crit chance earned so far is re-applied each spawn.
    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.carry_stacks(ctx, player);
        if self.accumulated_stacks == 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        let champion_id = champion_ref.id();
        // Same-name buffs stack and this carries the whole banked total, so the
        // previous life's copies are removed first, or every respawn would add
        // another total past the cap.
        ctx.entity_remove_buff(champion_id, self.yun_tal_practice_buff);
        ctx.add_buff(
            champion_id,
            &BuffV1 {
                crit_chance: self.accumulated_stacks as i32 * self.effect_stack_crit_chance,
                ..BuffV1::named(self.yun_tal_practice_buff)
            },
        );
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        _target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let is_flurry_on_cooldown = has_buff(&caster_ref, self.yun_tal_flurry_cooldown_buff);

        // Practice: every third base attack grants permanent crit chance,
        // capped. Attacks are counted, not hits: an attack's several hits all
        // land on one tick, and two attacks never do.
        let tick = ctx.tick();
        if self.accumulated_stacks < self.effect_max_stacks && self.counted_at != Some(tick) {
            self.counted_at = Some(tick);
            self.attack_count += 1;
            if self.attack_count >= self.effect_attack_interval {
                self.attack_count = 0;
                ctx.add_buff(
                    caster,
                    &BuffV1 {
                        crit_chance: self.effect_stack_crit_chance,
                        ..BuffV1::named(self.yun_tal_practice_buff)
                    },
                );
                self.accumulated_stacks += 1;
            }
        }

        // Flurry: on attack, gain a burst of attack speed on an internal cooldown.
        if !is_flurry_on_cooldown {
            ctx.add_buff(
                caster,
                &BuffV1 {
                    attack_speed_mult: self.effect_flurry_attack_speed_mult,
                    ..BuffV1::timed(
                        self.yun_tal_flurry_buff,
                        ticks(self.effect_duration_seconds),
                    )
                },
            );
            ctx.add_buff(
                caster,
                &BuffV1::timed(
                    self.yun_tal_flurry_cooldown_buff,
                    ticks(self.effect_cooldown_seconds),
                ),
            );
        }
    }

    // The attack hook is not told which player it runs for, so the stacks are
    // noted, and taken over by a new Radiant instance, from here.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.carry_stacks(ctx, player);
        // The crit chance Practice has granted, in percent, for the number
        // on the item's icon.
        let gained = self.accumulated_stacks as i32 * self.effect_stack_crit_chance;
        item_counts::note(ctx, player, self.meta.key, gained.max(0) as u64);
    }

    // Practice stacks are permanent, so they cross the Radiant upgrade; only
    // the counter moves (see `carry_stacks`). The host (0.6.2) never calls
    // these two (`crate::upgrade_carry`), and they stay for a host that does.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.accumulated_stacks as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.accumulated_stacks = (carry as usize).min(self.effect_max_stacks);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::AttackSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

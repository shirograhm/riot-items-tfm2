use std::collections::HashMap;

use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, percent_of, refresh_buff, ticks, ItemMeta, ProcQueue, DISTANCE_UNITS_PER_RANGE,
    TICKS_PER_SECOND,
};

// Ironheart, League's Colossal Consumption: an enemy champion who stays within
// `effect_max_distance` of the carrier for `effect_charge_seconds` is marked,
// and the carrier's next basic attack against them deals bonus damage and banks
// part of it as permanent health. Each enemy champion has their own charge,
// mark and `effect_cooldown_seconds` cooldown, so one fight can pay out on
// several of them. Leaving range before the charge completes starts it over.
// A mark lasts until it is spent, until either side dies, or until the enemy
// has spent `effect_duration_seconds` out of range in one go; one that falls
// off that way costs no cooldown.
//
// The pictures are League's stack and trigger VFX. The charge and the mark are
// drawn by two statless buffs on the enemy, bound in the `view_buffs` table of
// `view/effects.view_effects` and both from `effects/heartsteel_mark`: an orb
// up and to the right of them, a dark swirling ring for the first half of the
// charge, then the same ring with a yellow-green core lit, then a bright pink
// orb pulsing inside it until the attack lands. The attack bursts the orb: a
// pink flash where it was, with sharp light-blue and pink spikes shooting out of
// it (`effects/heartsteel_trigger`).

/// The Ironheart proc sound: `sound/sfx/riot_heartsteel_ironheart.sound_info`,
/// mapped into `asset/base/sound/sfx` by `mod.override_info`, which is where
/// sound names are looked up. The clip is `sfx/lol-heartsteel.mp3` turned up
/// 2 dB with its peaks limited to -1 dBFS, its tail trimmed, and faded in and
/// out; the first cut, 3 dB down, was too quiet in game.
const IRONHEART_SFX: &str = "riot_heartsteel_ironheart";
/// Over an enemy champion while Ironheart charges on them (tag `charge`, drawn
/// for the default 1.5 second charge).
const CHARGE_BUFF: &str = "riot_heartsteel_charge";
/// Over an enemy champion Ironheart has charged on (tag `ready`).
const MARK_BUFF: &str = "riot_heartsteel_mark";
/// The burst on the enemy when a mark is spent (a `view_effects` animation).
const TRIGGER_EFFECT: &str = "riot_heartsteel_trigger";
/// The charge buff outlasts the charge by this much, so it always comes off on
/// purpose rather than running out a tick early.
const CHARGE_BUFF_GRACE_TICKS: usize = 30;
/// The mark is refreshed once a second, so it never lapses while it is up, and
/// is gone half a second after a missed refresh: the item left.
const MARK_BUFF_TICKS: usize = 90;

#[derive(Clone, Debug)]
pub struct Heartsteel {
    meta: ItemMeta,
    stack_buff: &'static str,
    price: usize,
    hp: i32,
    effect_bonus_flat_damage: usize,
    effect_caster_hp_percent_damage: f64,
    effect_bonus_hp_percent_of_damage: f64,
    effect_max_distance: usize,
    effect_charge_seconds: f64,
    effect_duration_seconds: f64,
    effect_cooldown_seconds: f64,
    accumulated_bonus_hp: i32,
    /// Ironheart on each enemy champion, by their player id: the player keeps
    /// it across a respawn, their champion entity may not.
    targets: HashMap<usize, Target>,
    procs: ProcQueue,
}

impl Heartsteel {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "heartsteel",
                &["ring_of_reincarnation"],
                &["radiant_heartsteel"],
            ),
            stack_buff: "heartsteel_stack",
            price: 750,
            hp: 250,
            effect_bonus_flat_damage: 70,
            effect_caster_hp_percent_damage: 6.0,
            effect_bonus_hp_percent_of_damage: 10.0,
            effect_max_distance: 70,
            effect_charge_seconds: 1.5,
            effect_duration_seconds: 8.0,
            effect_cooldown_seconds: 30.0,
            // Non-vital stats (internals)
            accumulated_bonus_hp: 0,
            targets: HashMap::new(),
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_heartsteel", &["heartsteel"]),
            stack_buff: "heartsteel_stack",
            price: 1050,
            hp: 400,
            effect_bonus_flat_damage: 70,
            effect_caster_hp_percent_damage: 6.0,
            effect_bonus_hp_percent_of_damage: 10.0,
            effect_max_distance: 70,
            effect_charge_seconds: 1.5,
            effect_duration_seconds: 8.0,
            effect_cooldown_seconds: 30.0,
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
                effect_bonus_flat_damage,
                effect_caster_hp_percent_damage,
                effect_bonus_hp_percent_of_damage,
                effect_max_distance,
                effect_charge_seconds,
                effect_duration_seconds,
                effect_cooldown_seconds
            ]
        );
        self
    }

    /// Ends every charge and mark: the carrier died or respawned. The
    /// cooldowns keep running.
    fn drop_charges(&mut self, ctx: &mut StableSim<'_>) {
        for target in self.targets.values_mut() {
            if target.charge > 0 {
                ctx.entity_remove_buff(target.entity, CHARGE_BUFF);
            }
            if target.marked {
                ctx.entity_remove_buff(target.entity, MARK_BUFF);
            }
            target.charge = 0;
            target.marked = false;
            target.away = 0;
        }
    }
}

impl Default for Heartsteel {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Heartsteel {
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
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.procs.clear();
        self.drop_charges(ctx);

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        let champion_id = champion_ref.id();
        // Same-name buffs stack and this carries the whole banked total, so
        // the previous life's copies go before the new one lands. Without the
        // remove, every respawn added another full total on top of the ones
        // already worn and the cap stopped meaning anything.
        ctx.entity_remove_buff(champion_id, self.stack_buff);
        ctx.add_buff(
            champion_id,
            &BuffV1 {
                hp: self.accumulated_bonus_hp,
                ..BuffV1::named(self.stack_buff)
            },
        );
    }

    /// Spends a mark: a basic attack against a marked enemy champion.
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
        let Some(max_hp) = ctx.get_entity(caster).map(|c| c.hp().1) else {
            return;
        };
        let cooldown = ticks(self.effect_cooldown_seconds);
        let Some(marked) = self
            .targets
            .values_mut()
            .find(|t| t.marked && t.entity == target)
        else {
            return;
        };
        marked.marked = false;
        marked.away = 0;
        marked.cooldown = cooldown;
        ctx.entity_remove_buff(target, MARK_BUFF);

        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(max_hp, self.effect_caster_hp_percent_damage);
        let bonus_hp = percent_of(bonus_damage, self.effect_bonus_hp_percent_of_damage) as i32;
        // The banked health is priced off the damage this swing earned and is
        // granted with the swing; only the damage number waits.
        self.procs.push_physical(ctx, target, bonus_damage);
        ctx.play_sfx(IRONHEART_SFX, caster, &InputTargetV1::target(target));
        ctx.play_view_effect(
            TRIGGER_EFFECT,
            caster,
            &InputTargetV1::target(target),
            0,
            0,
            0,
        );
        ctx.add_buff(
            caster,
            &BuffV1 {
                hp: bonus_hp,
                ..BuffV1::named(self.stack_buff)
            },
        );
        self.accumulated_bonus_hp += bonus_hp;
    }

    /// Lands the Ironheart damage whose delay has run out, then charges, marks
    /// and cools down every enemy champion.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);

        let Some(team) = ctx.get_player(player).map(|p| p.team()) else {
            return;
        };
        let carrier = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| c.id());
        // (player id, champion entity, alive) for every enemy champion.
        let mut enemies = Vec::new();
        for index in 0..ctx.player_count() {
            let Some(enemy) = ctx.player_at(index) else {
                continue;
            };
            if enemy.team() == team {
                continue;
            }
            if let Some(champion) = enemy.champion() {
                enemies.push((enemy.id(), champion.id(), champion.is_alive()));
            }
        }

        let reach = (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        let charge_ticks = ticks(self.effect_charge_seconds).max(1);
        let away_ticks = ticks(self.effect_duration_seconds).max(1);
        let refresh = ctx.tick() % TICKS_PER_SECOND as usize == 0;
        for (id, entity, alive) in enemies {
            let target = self.targets.entry(id).or_default();
            target.entity = entity;
            target.cooldown = target.cooldown.saturating_sub(1);
            // Death on either side ends a charge and a mark.
            let Some(carrier_id) = carrier.filter(|_| alive) else {
                if target.charge > 0 {
                    ctx.entity_remove_buff(entity, CHARGE_BUFF);
                }
                if target.marked {
                    ctx.entity_remove_buff(entity, MARK_BUFF);
                }
                target.charge = 0;
                target.marked = false;
                target.away = 0;
                continue;
            };
            if target.marked {
                // Too long out of range and the mark falls off, with no
                // cooldown: the next time they come close it charges again.
                if ctx.distance_sq(carrier_id, entity) > reach * reach {
                    target.away += 1;
                    if target.away >= away_ticks {
                        target.marked = false;
                        target.away = 0;
                        ctx.entity_remove_buff(entity, MARK_BUFF);
                        continue;
                    }
                } else {
                    target.away = 0;
                }
                if refresh {
                    refresh_buff(
                        ctx,
                        entity,
                        MARK_BUFF,
                        &BuffV1::timed(MARK_BUFF, MARK_BUFF_TICKS),
                    );
                }
                continue;
            }
            if target.cooldown > 0 {
                continue;
            }
            if ctx.distance_sq(carrier_id, entity) > reach * reach {
                if target.charge > 0 {
                    target.charge = 0;
                    ctx.entity_remove_buff(entity, CHARGE_BUFF);
                }
                continue;
            }
            if target.charge == 0 {
                refresh_buff(
                    ctx,
                    entity,
                    CHARGE_BUFF,
                    &BuffV1::timed(CHARGE_BUFF, charge_ticks + CHARGE_BUFF_GRACE_TICKS),
                );
            }
            target.charge += 1;
            if target.charge >= charge_ticks {
                target.charge = 0;
                target.marked = true;
                target.away = 0;
                ctx.entity_remove_buff(entity, CHARGE_BUFF);
                refresh_buff(
                    ctx,
                    entity,
                    MARK_BUFF,
                    &BuffV1::timed(MARK_BUFF, MARK_BUFF_TICKS),
                );
            }
        }
    }

    /// Colossal Consumption's banked HP is permanent, so it crosses the Radiant
    /// upgrade. Only the counter moves: the HP already granted this life is
    /// sitting on the champion as `heartsteel_stack` buffs, and `on_spawn`
    /// re-applies it from the carried total on the next respawn. The cast round
    /// trips exactly for every `i32`, so the total needs no clamping.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.accumulated_bonus_hp as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.accumulated_bonus_hp = carry as i32;
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::MyHpPercentDamage]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}

/// Ironheart's hold on one enemy champion.
#[derive(Clone, Copy, Debug, Default)]
struct Target {
    /// Their champion entity, as of the last update.
    entity: usize,
    /// Ticks spent in range so far; they are marked at `effect_charge_seconds`.
    charge: usize,
    /// Charged: the carrier's next basic attack against them procs.
    marked: bool,
    /// Ticks in a row they have been out of range while marked; the mark falls
    /// off at `effect_duration_seconds`.
    away: usize,
    /// Ticks left before Ironheart can charge on them again.
    cooldown: usize,
}

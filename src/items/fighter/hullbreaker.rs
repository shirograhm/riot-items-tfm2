use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, is_monster, percent_of, sized_range, ItemMeta, ProcQueue, AURA_DURATION_TICKS,
    AURA_REFRESH_TICKS, DISTANCE_UNITS_PER_RANGE,
};

// Skipper: Every fifth basic attack against champions and monsters deals bonus
// physical damage, increased against turrets.
//
// Boarding Party: Allied minions nearby gain armor and magic resistance.

// One name for both tiers, so two carriers near the same wave replace each
// other's bonus instead of stacking it.
const BOARDING_PARTY_BUFF: &str = "hullbreaker_boarding_party";

#[derive(Clone, Debug)]
pub struct Hullbreaker {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    hp: i32,
    move_speed_mult: i32,
    effect_attack_interval: usize,
    effect_ad_percent_damage: f64,
    effect_caster_hp_percent_damage: f64,
    effect_tower_ad_percent_damage: f64,
    effect_tower_caster_hp_percent_damage: f64,
    effect_melee_distance: usize,
    effect_ranged_percent: f64,
    effect_bonus_defence: i32,
    effect_bonus_magic_resistance: i32,
    effect_max_distance: usize,
    // Non-vital stats (internals)
    attack_count: usize,
    refresh_cooldown: usize,
    procs: ProcQueue,
}

impl Hullbreaker {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "hullbreaker",
                &["winged_moonplate", "phage"],
                &["radiant_hullbreaker"],
            ),
            price: 800,
            attack: 20,
            hp: 200,
            move_speed_mult: 4,
            effect_attack_interval: 5,
            effect_ad_percent_damage: 80.0,
            effect_caster_hp_percent_damage: 5.0,
            effect_tower_ad_percent_damage: 200.0,
            effect_tower_caster_hp_percent_damage: 10.0,
            effect_melee_distance: 35,
            effect_ranged_percent: 70.0,
            effect_bonus_defence: 10,
            effect_bonus_magic_resistance: 20,
            effect_max_distance: 100,
            // Non-vital stats (internals)
            attack_count: 0,
            refresh_cooldown: 0,
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_hullbreaker", &["hullbreaker"]),
            price: 1000,
            attack: 35,
            hp: 300,
            move_speed_mult: 4,
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
                hp,
                move_speed_mult,
                effect_attack_interval,
                effect_ad_percent_damage,
                effect_caster_hp_percent_damage,
                effect_tower_ad_percent_damage,
                effect_tower_caster_hp_percent_damage,
                effect_melee_distance,
                effect_ranged_percent,
                effect_bonus_defence,
                effect_bonus_magic_resistance,
                effect_max_distance
            ]
        );
        self
    }

    // Boarding Party, refreshed on the shared aura cycle: every allied minion
    // in range has its bonus replaced, and one that walks out keeps it until
    // the buff runs out.
    fn boarding_party(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if self.refresh_cooldown > 0 {
            self.refresh_cooldown -= 1;
            return;
        }
        self.refresh_cooldown = AURA_REFRESH_TICKS;

        let Some((carrier, team)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| (c.id(), c.team()))
        else {
            return;
        };
        let range = sized_range(ctx, carrier, self.effect_max_distance);
        let range_sq = range * range;

        let minions: Vec<usize> = (0..ctx.entity_count())
            .filter_map(|index| ctx.entity_at(index))
            .filter(|e| e.is_minion() && e.is_alive() && e.team() == team)
            .map(|e| e.id())
            .collect();
        for id in minions {
            if ctx.distance_sq(carrier, id) > range_sq {
                continue;
            }
            ctx.entity_remove_buff(id, BOARDING_PARTY_BUFF);
            ctx.add_buff(
                id,
                &BuffV1 {
                    defence: self.effect_bonus_defence,
                    magic_resistance: self.effect_bonus_magic_resistance,
                    ..BuffV1::timed(BOARDING_PARTY_BUFF, AURA_DURATION_TICKS)
                },
            );
        }
    }
}

impl Default for Hullbreaker {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Hullbreaker {
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
            hp: self.hp,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.attack_count = 0;
        self.refresh_cooldown = 0;
        self.procs.clear();
    }

    // Skipper. Attacks on champions and monsters count toward the fifth; an
    // attack on a turret never counts, but it can be the fifth, and then it
    // takes the turret damage. Minions do neither.
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
        let Some((counts, is_tower)) = ctx
            .get_entity(target)
            .map(|t| (t.is_champion() || is_monster(&t), t.is_tower()))
        else {
            return;
        };
        if !counts && !is_tower {
            return;
        }
        if self.attack_count + 1 < self.effect_attack_interval {
            if counts {
                self.attack_count += 1;
            }
            return;
        }

        let Some((attack, max_hp)) = ctx.get_entity(caster).map(|c| (c.stat().attack, c.hp().1))
        else {
            return;
        };
        let (ad_percent, hp_percent) = if is_tower {
            (
                self.effect_tower_ad_percent_damage,
                self.effect_tower_caster_hp_percent_damage,
            )
        } else {
            (
                self.effect_ad_percent_damage,
                self.effect_caster_hp_percent_damage,
            )
        };
        let mut bonus = percent_of(attack, ad_percent) + percent_of(max_hp, hp_percent);
        let reach = (self.effect_melee_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        if ctx.distance_sq(caster, target) > reach * reach {
            bonus = percent_of(bonus, self.effect_ranged_percent);
        }
        self.procs.push_physical(ctx, target, bonus);
        self.attack_count = 0;
    }

    // Lands the Skipper hits whose delay has run out, and keeps Boarding
    // Party on the minions around the carrier.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
        self.boarding_party(ctx, player);
    }

    // Skipper's count survives the Radiant upgrade rather than resetting.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.attack_count as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.attack_count = (carry as usize).min(self.effect_attack_interval.saturating_sub(1));
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::Hp, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

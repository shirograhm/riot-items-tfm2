use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, ticks, ItemMeta, DISTANCE_UNITS_PER_RANGE};

// Zeke's Convergence (registered under its old key, `zekes_herald`).
//
// Cryocombustion: Gain 15 Ultimate Ability Haste.
//
// Frostfire Tempest: Upon casting your ultimate ability, summon a storm of flame
// and ice around you for 4 seconds. The storm deals 30 magic damage per second to
// nearby enemies and applies a 20% slow.

/// The storm hits four times a second.
const TEMPEST_TICK_SECONDS: f64 = 0.25;
/// Shared by both variants: the slow is a state on the target, and two carriers'
/// storms refresh one slow rather than stacking two.
const SLOW_BUFF: &str = "zekes_convergence_slow";
/// A little longer than one storm tick, so a target inside the storm is never
/// between slows; it wears off shortly after leaving the storm or the storm
/// ending.
const SLOW_GRACE_TICKS: usize = 10;
/// Statless marker on the carrier for as long as the storm lasts. It is the
/// `view_buffs` binding in `view/effects.view_effects` that draws the storm, so
/// the art follows her and goes when she dies (buffs do not survive death).
const STORM_BUFF: &str = "zekes_convergence_storm";

#[derive(Clone, Debug)]
pub struct ZekesHerald {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    defence: i32,
    magic_resistance: i32,
    skill_cooldown_mult: i32,
    ult_cooldown_mult: i32,
    effect_duration_seconds: f64,
    effect_bonus_magic_damage: usize,
    effect_slow_amount: i32,
    effect_max_distance: usize,
    // Non-vital stats (internals)
    /// Ult cooldown seen on the previous tick. There is no "cast ultimate" hook,
    /// so a cast is read off the cooldown jumping up.
    last_ult_cooldown: Option<usize>,
    storm_ticks_left: usize,
    until_next_tick: usize,
    /// Storm ticks dealt so far, so per-tick damage can alternate to hit the
    /// per-second total exactly (30 per second is 7.5 per tick: 7, 8, 7, 8).
    storm_ticks_dealt: usize,
}

impl ZekesHerald {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "zekes_herald",
                &["aegis_of_the_legion"],
                &["radiant_zekes_herald"],
            ),
            price: 550,
            hp: 100,
            defence: 20,
            magic_resistance: 30,
            skill_cooldown_mult: 10,
            ult_cooldown_mult: 15,
            effect_duration_seconds: 4.0,
            effect_bonus_magic_damage: 30,
            effect_slow_amount: 30,
            effect_max_distance: 50,
            // Non-vital stats (internals)
            last_ult_cooldown: None,
            storm_ticks_left: 0,
            until_next_tick: 0,
            storm_ticks_dealt: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_zekes_herald", &["zekes_herald"]),
            price: 750,
            hp: 150,
            defence: 30,
            magic_resistance: 40,
            skill_cooldown_mult: 15,
            ult_cooldown_mult: 15,
            effect_duration_seconds: 4.0,
            effect_bonus_magic_damage: 30,
            effect_slow_amount: 30,
            effect_max_distance: 50,
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
                skill_cooldown_mult,
                ult_cooldown_mult,
                effect_duration_seconds,
                effect_bonus_magic_damage,
                effect_slow_amount,
                effect_max_distance
            ]
        );
        self
    }

    /// Damage for the next storm tick: the running total rounded down, minus
    /// what has already been dealt.
    fn next_tick_damage(&mut self) -> usize {
        let per_tick = self.effect_bonus_magic_damage as f64 * TEMPEST_TICK_SECONDS;
        let dealt = (per_tick * self.storm_ticks_dealt as f64).floor() as usize;
        self.storm_ticks_dealt += 1;
        let total = (per_tick * self.storm_ticks_dealt as f64).floor() as usize;
        total - dealt
    }

    fn storm_tick(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        let Some((caster, caster_team)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| (c.id(), c.team()))
        else {
            // The storm is around the carrier; it ends with her.
            self.storm_ticks_left = 0;
            return;
        };

        let range = (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        let range_sq = range * range;
        // Any enemy unit but a turret, the way Bami's Cinder picks its targets.
        let targets: Vec<usize> = (0..ctx.entity_count())
            .filter_map(|index| ctx.entity_at(index))
            .filter(|e| e.is_alive() && !e.is_tower() && e.team() != caster_team)
            .map(|e| e.id())
            .filter(|&id| ctx.distance_sq(caster, id) <= range_sq)
            .collect();

        let damage = self.next_tick_damage();
        let slow = BuffV1 {
            move_speed_mult: -self.effect_slow_amount,
            ..BuffV1::timed(SLOW_BUFF, ticks(TEMPEST_TICK_SECONDS) + SLOW_GRACE_TICKS)
        };
        for target in targets {
            if damage > 0 {
                ctx.deal_damage(caster, target, 0, damage, AttackTypeV1::Item);
            }
            refresh_buff(ctx, target, SLOW_BUFF, &slow);
        }
    }
}

impl Default for ZekesHerald {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for ZekesHerald {
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
            skill_cooldown_mult: self.skill_cooldown_mult,
            ult_cooldown_mult: self.ult_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.last_ult_cooldown = None;
        self.storm_ticks_left = 0;
        self.until_next_tick = 0;
    }

    // Frostfire Tempest. A cast shows up as the ult cooldown going *up* between
    // two ticks: it only ever counts down otherwise. The first reading after a
    // spawn is just a baseline.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        let ult_cooldown = ctx
            .get_player(player)
            .and_then(|p| p.cooldowns())
            .map(|(_, _, _, ult)| ult);
        if let (Some(now), Some(before)) = (ult_cooldown, self.last_ult_cooldown) {
            if now > before {
                self.storm_ticks_left = ticks(self.effect_duration_seconds);
                self.until_next_tick = 0;
                self.storm_ticks_dealt = 0;
                let carrier = ctx
                    .get_player(player)
                    .and_then(|p| p.champion())
                    .map(|c| c.id());
                if let Some(carrier) = carrier {
                    refresh_buff(
                        ctx,
                        carrier,
                        STORM_BUFF,
                        &BuffV1::timed(STORM_BUFF, self.storm_ticks_left),
                    );
                }
            }
        }
        self.last_ult_cooldown = ult_cooldown;

        if self.storm_ticks_left == 0 {
            return;
        }
        if self.until_next_tick == 0 {
            self.storm_tick(ctx, player);
            self.until_next_tick = ticks(TEMPEST_TICK_SECONDS).max(1);
        }
        self.until_next_tick -= 1;
        self.storm_ticks_left = self.storm_ticks_left.saturating_sub(1);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Defense,
            ItemTagV1::MagicResistance,
            ItemTagV1::CooltimeReduce,
            ItemTagV1::DotDamage,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}

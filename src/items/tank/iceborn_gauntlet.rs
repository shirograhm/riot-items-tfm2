use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, ticks, ItemMeta, Spellblade, DISTANCE_UNITS_PER_RANGE};

// Spellblade: Using an Ability causes your next basic attack within 10 seconds
// to deal 30 - 85 (based on level) bonus physical damage on-hit and creates a
// frost zone under the target for 2 seconds (1.5 second cooldown, starting
// after using the empowered attack). Enemies within the zone are slowed by 20%.
//
// The damage is Sheen's, so the upgrade keeps what the component did. The
// zone is a spot and a timer, like Hollow Radiance's eruption: no unit stands
// for it, so it stays where the target was hit and nothing can attack it.

/// A zone slows what stands in it four times a second.
const SLOW_TICK_SECONDS: f64 = 0.25;
/// Shared by both variants and by every zone: the slow is a state on the
/// target, so two zones refresh one slow rather than stacking two.
const SLOW_BUFF: &str = "iceborn_gauntlet_slow";
/// A little longer than one slow tick, so an enemy inside a zone is never
/// between slows; it wears off shortly after leaving the zone or the zone
/// melting. The slow has no duration of its own: it is for being in the zone.
const SLOW_GRACE_TICKS: usize = 10;

/// The zone's picture, in three plays of one sheet (`effects/frost_zone`): it
/// spreads, it lies there, it melts. Bound in `view/effects.view_effects`.
const ZONE_FORM_EFFECT: &str = "riot_frost_zone_form";
const ZONE_HOLD_EFFECT: &str = "riot_frost_zone_hold";
const ZONE_MELT_EFFECT: &str = "riot_frost_zone_melt";
/// How long one play lasts, in ticks: each is five frames of 0.1 s. A zone's
/// first play spreads, its last melts and the ones between hold, so the
/// picture covers whatever the config makes the zone's duration, to within one
/// play.
const ZONE_PLAY_TICKS: usize = 30;

/// A frost zone lying in the world.
#[derive(Clone, Copy, Debug)]
struct FrostZone {
    x: u64,
    y: u64,
    /// Ticks until it melts away.
    remaining: usize,
    /// Ticks until it slows again. Zero as it forms, so it slows at once.
    until_slow: usize,
    /// Ticks until its picture is played again.
    until_play: usize,
    /// Whether its picture has been played at all: the first play spreads.
    formed: bool,
}

#[derive(Clone, Debug)]
pub struct IcebornGauntlet {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    defence: i32,
    attack_speed_mult: i32,
    skill_cooldown_mult: i32,
    effect_min_bonus_damage: usize,
    effect_max_bonus_damage: usize,
    effect_cooldown_seconds: f64,
    effect_duration_seconds: f64,
    effect_slow_amount: i32,
    effect_max_distance: usize,
    // Non-vital stats (internals)
    spellblade: Spellblade,
    /// Every zone this carrier has lying in the world. The empowered attack
    /// lays one; the rest happens in `update`.
    zones: Vec<FrostZone>,
}

impl IcebornGauntlet {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("iceborn_gauntlet", &["sheen"], &["radiant_iceborn_gauntlet"]),
            price: 750,
            hp: 150,
            defence: 15,
            attack_speed_mult: 20,
            skill_cooldown_mult: 10,
            effect_min_bonus_damage: 30,
            effect_max_bonus_damage: 85,
            effect_cooldown_seconds: 1.5,
            effect_duration_seconds: 2.0,
            effect_slow_amount: 20,
            effect_max_distance: 30,
            // Non-vital stats (internals)
            spellblade: Spellblade::default(),
            zones: Vec::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_iceborn_gauntlet", &["iceborn_gauntlet"]),
            price: 1000,
            hp: 250,
            defence: 25,
            attack_speed_mult: 25,
            skill_cooldown_mult: 15,
            effect_min_bonus_damage: 30,
            effect_max_bonus_damage: 85,
            effect_cooldown_seconds: 1.5,
            effect_duration_seconds: 2.0,
            effect_slow_amount: 20,
            effect_max_distance: 30,
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
                attack_speed_mult,
                skill_cooldown_mult,
                effect_min_bonus_damage,
                effect_max_bonus_damage,
                effect_cooldown_seconds,
                effect_duration_seconds,
                effect_slow_amount,
                effect_max_distance
            ]
        );
        self
    }

    // Bonus damage scales linearly from min (level 1) to max (level 12), the
    // way Sheen's does.
    fn spellblade_damage(&self, level: usize) -> usize {
        let per_level = (self
            .effect_max_bonus_damage
            .saturating_sub(self.effect_min_bonus_damage) as f64
            / 11.0)
            .round() as usize;
        self.effect_min_bonus_damage + level.saturating_sub(1) * per_level
    }

    /// Lays a frost zone where `target` stands. It stays on that spot for its
    /// whole life, whatever the target does next.
    fn lay_zone(&mut self, ctx: &StableSim<'_>, target: usize) {
        let Some((x, y)) = ctx.get_entity(target).map(|target| target.pos()) else {
            return;
        };
        // What the host answers for a unit it cannot place.
        if (x, y) == (0, 0) {
            return;
        }
        self.zones.push(FrostZone {
            x,
            y,
            remaining: ticks(self.effect_duration_seconds),
            until_slow: 0,
            until_play: 0,
            formed: false,
        });
    }

    /// Enemy units (not turrets) within `range` of a spot.
    fn enemies_near(
        ctx: &StableSim<'_>,
        team: usize,
        (x, y): (u64, u64),
        range: usize,
    ) -> Vec<usize> {
        let range = (range * DISTANCE_UNITS_PER_RANGE) as i128;
        let range_sq = range * range;
        (0..ctx.entity_count())
            .filter_map(|index| ctx.entity_at(index))
            .filter(|e| e.is_alive() && !e.is_tower() && e.team() != team)
            .filter(|e| {
                let (ex, ey) = e.pos();
                let dx = ex as i128 - x as i128;
                let dy = ey as i128 - y as i128;
                dx * dx + dy * dy <= range_sq
            })
            .map(|e| e.id())
            .collect()
    }

    /// Runs every zone lying in the world: its picture, its slow, its timer.
    fn run_zones(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if self.zones.is_empty() {
            return;
        }
        // A zone outlives its carrier: it only ever melts.
        let Some((carrier, team)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| (c.id(), c.team()))
        else {
            return;
        };

        // At least a tick apart, whatever the constants become.
        let slow_every = ticks(SLOW_TICK_SECONDS).max(1);
        let slow = BuffV1 {
            move_speed_mult: -self.effect_slow_amount,
            ..BuffV1::timed(SLOW_BUFF, slow_every + SLOW_GRACE_TICKS)
        };
        let range = self.effect_max_distance;

        for zone in &mut self.zones {
            if zone.until_play == 0 {
                zone.until_play = ZONE_PLAY_TICKS;
                let effect = if !zone.formed {
                    ZONE_FORM_EFFECT
                } else if zone.remaining <= ZONE_PLAY_TICKS {
                    ZONE_MELT_EFFECT
                } else {
                    ZONE_HOLD_EFFECT
                };
                zone.formed = true;
                ctx.play_view_effect(effect, carrier, &InputTargetV1::pos(zone.x, zone.y), 0, 0, 0);
            }
            zone.until_play -= 1;

            if zone.until_slow == 0 {
                zone.until_slow = slow_every;
                for target in Self::enemies_near(ctx, team, (zone.x, zone.y), range) {
                    refresh_buff(ctx, target, SLOW_BUFF, &slow);
                }
            }
            zone.until_slow -= 1;

            zone.remaining = zone.remaining.saturating_sub(1);
        }
        self.zones.retain(|zone| zone.remaining > 0);
    }
}

impl Default for IcebornGauntlet {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for IcebornGauntlet {
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
            attack_speed_mult: self.attack_speed_mult,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.spellblade.reset();
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        damage: &mut usize,
        damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if !self.spellblade.is_ready() || attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let bonus_damage = self.spellblade_damage(caster_ref.level());

        Spellblade::on_hit_physical(ctx, caster, target, damage, damage_type, bonus_damage);
        self.spellblade
            .spend(ctx, caster, target, self.effect_cooldown_seconds);
        self.lay_zone(ctx, target);
    }

    /// Watches for the cast that readies Spellblade, and runs the zones.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.spellblade.update(ctx, player);
        self.run_zones(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Defense,
            ItemTagV1::AttackSpeed,
            ItemTagV1::CooltimeReduce,
            ItemTagV1::MoveSpeed,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Defense
    }
}

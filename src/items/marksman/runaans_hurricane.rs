use mod_api_stable::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use crate::config::ItemConfig;
use crate::{apply_config, is_monster, percent_of, ItemMeta, ProcQueue, DISTANCE_UNITS_PER_RANGE};

// Wind's Fury: Basic attacks fire bolts at up to 2 additional enemies near the
// target, each dealing bonus physical damage that can critically strike.
//
// The bolts are real projectiles: `spawn_projectile` flies each one from the
// carrier to its target, and the damage lands when it arrives, through the
// native effect the bolt carries (`RunaansBolt`, registered in `lib.rs`, one
// per tier). That effect is handed a deterministic seed, which is what the
// crit roll uses. The damage goes out as `Item`, so it applies no on-hit
// effects -- this item's own included, which would otherwise fire bolts
// forever.

/// The `view_projectiles` name in `view/effects.view_effects` that draws a
/// bolt in flight (`effects/runaans_bolt`).
const BOLT_PROJECTILE: &str = "riot_runaans_bolt";
/// World units per tick. The base game's own ranged attacks fly at 4200-4300;
/// the bolts go about half again as fast, which read as too slow at 4300.
const BOLT_SPEED: u64 = 6500;

#[derive(Clone, Debug)]
pub struct RunaansHurricane {
    meta: ItemMeta,
    /// The native effect this tier's bolts carry.
    bolt_hit: &'static str,
    price: usize,
    attack_speed_mult: i32,
    crit_chance: i32,
    move_speed_mult: i32,
    effect_ad_percent_damage: f64,
    effect_max_targets: usize,
    effect_max_distance: usize,
    // Non-vital stats (internals)
    procs: ProcQueue,
}

impl RunaansHurricane {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "runaans_hurricane",
                &["twin_stormblade", "scouts_slingshot"],
                &["radiant_runaans_hurricane"],
            ),
            bolt_hit: "riot_runaans_bolt_hit",
            price: 700,
            attack_speed_mult: 40,
            crit_chance: 20,
            move_speed_mult: 5,
            effect_ad_percent_damage: 50.0,
            effect_max_targets: 2,
            effect_max_distance: 50,
            // Non-vital stats (internals)
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_runaans_hurricane", &["runaans_hurricane"]),
            bolt_hit: "riot_radiant_runaans_bolt_hit",
            price: 950,
            attack_speed_mult: 70,
            crit_chance: 25,
            move_speed_mult: 5,
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
                attack_speed_mult,
                crit_chance,
                move_speed_mult,
                effect_ad_percent_damage,
                effect_max_targets,
                effect_max_distance
            ]
        );
        self
    }

    /// The native effect this tier's bolts land through, for `lib.rs` to
    /// register under [`RunaansHurricane::bolt_hit_name`].
    pub fn bolt_hit(&self) -> RunaansBolt {
        RunaansBolt {
            ad_percent: self.effect_ad_percent_damage,
        }
    }

    pub fn bolt_hit_name(&self) -> &'static str {
        self.bolt_hit
    }

    /// Up to `effect_max_targets` enemies within range of `target`, champions
    /// first, nearest first. Turrets are never picked.
    fn bolt_targets(&self, ctx: &StableSim<'_>, team: usize, target: usize) -> Vec<usize> {
        let range = (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        let range_sq = range * range;
        let mut candidates: Vec<(bool, u64, usize)> = (0..ctx.entity_count())
            .filter_map(|index| ctx.entity_at(index))
            .filter(|e| {
                e.id() != target
                    && e.team() != team
                    && e.is_alive()
                    && e.is_targetable()
                    && (e.is_champion() || e.is_minion() || is_monster(e))
            })
            .map(|e| (e.is_champion(), e.id()))
            .filter_map(|(champion, id)| {
                let dist = ctx.distance_sq(target, id);
                (dist <= range_sq).then_some((!champion, dist, id))
            })
            .collect();
        candidates.sort_unstable();
        candidates
            .into_iter()
            .take(self.effect_max_targets)
            .map(|(_, _, id)| id)
            .collect()
    }
}

impl Default for RunaansHurricane {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for RunaansHurricane {
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
            attack_speed_mult: self.attack_speed_mult,
            crit_chance: self.crit_chance,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
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
        if ctx.get_entity(target).map_or(true, |t| t.is_tower()) {
            return;
        }
        let Some((team, (x, y), attack)) = ctx
            .get_entity(caster)
            .map(|c| (c.team(), c.pos(), c.stat().attack))
        else {
            return;
        };

        for id in self.bolt_targets(ctx, team, target) {
            let spec = ProjectileSpawnV1 {
                caster_id: caster,
                team,
                x,
                y,
                speed: BOLT_SPEED,
                target_id: id,
                attack_type: AttackTypeV1::Item.code(),
                ..ProjectileSpawnV1::default()
            };
            // A host without projectiles still gets the damage, straight away
            // and without the crit roll.
            if !ctx.spawn_projectile(BOLT_PROJECTILE, self.bolt_hit, &spec) {
                let damage = percent_of(attack, self.effect_ad_percent_damage);
                self.procs.push_physical(ctx, id, damage);
            }
        }
    }

    /// Lands fallback bolts whose delay has run out.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::AttackSpeed, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}

/// Lands one Wind's Fury bolt: a share of the carrier's AD as physical damage,
/// doubled on a critical strike rolled against their crit chance.
#[derive(Clone, Debug)]
pub struct RunaansBolt {
    ad_percent: f64,
}

impl StableEffectType for RunaansBolt {
    fn apply(
        &self,
        sim: &mut StableSim<'_>,
        rng_seed: u64,
        caster_id: usize,
        input: InputTargetV1,
    ) {
        let target = input.target_id;
        if !sim.get_entity(target).is_some_and(|t| t.is_alive()) {
            return;
        }
        let Some(stat) = sim.get_entity(caster_id).map(|c| c.stat()) else {
            return;
        };
        let mut damage = percent_of(stat.attack, self.ad_percent);
        let mut rng = StdRng::seed_from_u64(rng_seed);
        if rng.random::<f64>() < stat.crit_chance as f64 / 100.0 {
            damage *= 2;
        }
        sim.deal_damage(caster_id, target, damage, 0, AttackTypeV1::Item);
    }

    fn expected_damage(&self, caster_stat: &StatV1) -> (usize, usize) {
        (percent_of(caster_stat.attack, self.ad_percent), 0)
    }
}

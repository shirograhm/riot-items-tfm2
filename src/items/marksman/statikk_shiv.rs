use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, is_monster, percent_of, Energized, ItemMeta, ProcQueue, DISTANCE_UNITS_PER_RANGE,
};

// Electrospark: when fully Energized, the next basic attack deals bonus magic
// damage to its target and releases chain lightning that jumps on to up to
// `effect_max_targets` more enemies, each hop within range of the last one,
// champions first.
//
// The whole chain is worked out when the charged swing lands, and every hop
// is a real projectile flying from the enemy it jumps off to the next one
// (`spawn_projectile`), so the segments draw together as one chain. The damage
// lands on arrival through the native effect the hop carries (`StatikkSpark`,
// registered in `lib.rs`, one per tier). It goes out as `Item`, so the chain
// applies no on-hit effects -- this item's own included. Every enemy the
// chain hits, the first one included, is left crackling for a moment
// (`SHOCK_EFFECT`), since the hop itself vanishes the instant it lands.

// The `view_projectiles` name in `view/effects.view_effects` that draws one
// hop of the chain (`effects/statikk_spark`).
const SPARK_PROJECTILE: &str = "riot_statikk_spark";
// The `view_effects` name that crackles on each enemy the chain hits
// (`effects/statikk_shock`, 0.4 s, following the unit).
const SHOCK_EFFECT: &str = "riot_statikk_shock";
// A hop's hit circle. The default is one champion wide (10000), and enemies
// in a fight stand close enough that a hop that size touched its target
// almost as soon as it spawned, before it was ever drawn.
const SPARK_RADIUS: u64 = 1_000;
// How long every hop is in the air, whatever its length (0.1 s). A fixed
// speed would still make the short hops between bunched-up enemies vanish.
const HOP_TICKS: u64 = 6;
// Floor for a hop with almost no distance left to cover.
const MIN_SPARK_SPEED: u64 = 500;

#[derive(Clone, Debug)]
pub struct StatikkShiv {
    meta: ItemMeta,
    // The native effect this tier's chain hops carry.
    spark_hit: &'static str,
    price: usize,
    attack: i32,
    magic_power: i32,
    attack_speed_mult: i32,
    move_speed_mult: i32,
    effect_max_stacks: usize,
    effect_bonus_magic_damage: usize,
    effect_minion_percent: f64,
    effect_max_targets: usize,
    effect_max_distance: usize,
    // Non-vital stats (internals)
    energized: Energized,
    procs: ProcQueue,
}

impl StatikkShiv {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "statikk_shiv",
                &["scouts_slingshot"],
                &["radiant_statikk_shiv"],
            ),
            spark_hit: "riot_statikk_spark_hit",
            price: 700,
            attack: 15,
            magic_power: 15,
            attack_speed_mult: 30,
            move_speed_mult: 4,
            effect_max_stacks: 100,
            effect_bonus_magic_damage: 60,
            effect_minion_percent: 150.0,
            effect_max_targets: 4,
            effect_max_distance: 50,
            // Non-vital stats (internals)
            energized: Energized::default(),
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_statikk_shiv", &["statikk_shiv"]),
            spark_hit: "riot_radiant_statikk_spark_hit",
            price: 1000,
            attack: 25,
            magic_power: 25,
            attack_speed_mult: 50,
            move_speed_mult: 4,
            effect_max_stacks: 100,
            effect_bonus_magic_damage: 80,
            effect_minion_percent: 150.0,
            effect_max_targets: 5,
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
                attack,
                magic_power,
                attack_speed_mult,
                move_speed_mult,
                effect_max_stacks,
                effect_bonus_magic_damage,
                effect_minion_percent,
                effect_max_targets,
                effect_max_distance
            ]
        );
        self
    }

    // The native effect this tier's chain hops land through, for `lib.rs` to
    // register under [`StatikkShiv::spark_hit_name`].
    pub fn spark_hit(&self) -> StatikkSpark {
        StatikkSpark {
            damage: self.effect_bonus_magic_damage,
            minion_percent: self.effect_minion_percent,
        }
    }

    pub fn spark_hit_name(&self) -> &'static str {
        self.spark_hit
    }

    fn damage_against(&self, champion: bool) -> usize {
        spark_damage(
            self.effect_bonus_magic_damage,
            self.effect_minion_percent,
            champion,
        )
    }

    // The chain's hops after `target`, as `(from, to)` pairs: each jump goes
    // to an enemy within range of the one before it that the chain has not
    // hit yet, every champion in range before any other unit, nearest first
    // within each. Turrets are never picked. Stops early when nothing is in
    // range of the last enemy hit.
    fn chain_hops(&self, ctx: &StableSim<'_>, team: usize, target: usize) -> Vec<(usize, usize)> {
        let range = (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        let range_sq = range * range;
        let mut candidates: Vec<(bool, usize)> = (0..ctx.entity_count())
            .filter_map(|index| ctx.entity_at(index))
            .filter(|e| {
                e.id() != target
                    && e.team() != team
                    && e.is_alive()
                    && e.is_targetable()
                    && (e.is_champion() || e.is_minion() || is_monster(e))
            })
            .map(|e| (e.is_champion(), e.id()))
            .collect();

        let mut hops = Vec::new();
        let mut from = target;
        while hops.len() < self.effect_max_targets {
            let next = candidates
                .iter()
                .enumerate()
                .filter_map(|(index, &(champion, id))| {
                    let dist = ctx.distance_sq(from, id);
                    (dist <= range_sq).then_some((!champion, dist, index))
                })
                .min();
            let Some((_, _, index)) = next else {
                break;
            };
            let (_, to) = candidates.swap_remove(index);
            hops.push((from, to));
            from = to;
        }
        hops
    }

    // Hits `target` with the charged swing's bonus damage and sends the
    // chain on from it.
    fn electrospark(&mut self, ctx: &mut StableSim<'_>, caster: usize, target: usize) {
        let Some(team) = ctx.get_entity(caster).map(|c| c.team()) else {
            return;
        };
        let Some(champion) = ctx.get_entity(target).map(|t| t.is_champion()) else {
            return;
        };
        let damage = self.damage_against(champion);
        self.procs.push_magic(ctx, target, damage);
        play_shock(ctx, caster, target);

        for (from, to) in self.chain_hops(ctx, team, target) {
            let Some((x, y)) = ctx.get_entity(from).map(|e| e.pos()) else {
                continue;
            };
            let spec = ProjectileSpawnV1 {
                caster_id: caster,
                team,
                x,
                y,
                radius: SPARK_RADIUS,
                speed: hop_speed(ctx, from, to),
                target_id: to,
                attack_type: AttackTypeV1::Item.code(),
                ..ProjectileSpawnV1::default()
            };
            // A host without projectiles still gets the damage, straight away.
            if !ctx.spawn_projectile(SPARK_PROJECTILE, self.spark_hit, &spec) {
                let champion = ctx.get_entity(to).is_some_and(|t| t.is_champion());
                let damage = self.damage_against(champion);
                self.procs.push_magic(ctx, to, damage);
                play_shock(ctx, caster, to);
            }
        }
    }
}

impl Default for StatikkShiv {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for StatikkShiv {
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
            magic_power: self.magic_power,
            attack_speed_mult: self.attack_speed_mult,
            move_speed_mult: self.move_speed_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.energized.reset();
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
        // Base attacks only: the chain lands as `Item` damage, which must not
        // spend or build the meter.
        if attack_type != AttackTypeV1::BaseAttack {
            return;
        }
        if ctx.get_entity(target).map_or(true, |t| t.is_tower()) {
            return;
        }

        if self
            .energized
            .is_charged(ctx, caster, self.effect_max_stacks)
        {
            self.energized.spend(ctx);
            self.electrospark(ctx, caster, target);
        }

        self.energized.basic_attack(self.effect_max_stacks);
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
        self.energized.update(ctx, player, self.effect_max_stacks);
    }

    // The Energized meter carries across the Radiant upgrade, so buying it
    // mid-fight does not throw away a nearly full bar.
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.energized.stacks() as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.energized
                .set_stacks((carry as usize).min(self.effect_max_stacks));
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Ad,
            ItemTagV1::Ap,
            ItemTagV1::AttackSpeed,
            ItemTagV1::MoveSpeed,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::AttackSpeed
    }
}

// Electrospark's damage against one unit: the flat amount on champions,
// scaled by `minion_percent` on minions and monsters.
fn spark_damage(damage: usize, minion_percent: f64, champion: bool) -> usize {
    if champion {
        damage
    } else {
        percent_of(damage, minion_percent)
    }
}

// The speed that keeps a hop from `from` to `to` in the air for `HOP_TICKS`:
// the distance left once the hit circles touch, spread over the flight, plus
// the target's own move speed. Without that, a short hop could fly slower
// than its target runs and trail behind an enemy moving away. Move speed is
// in the same units per tick as projectile speed (vanilla champions move at
// 900; their basic attacks fly at 4200-4300).
fn hop_speed(ctx: &StableSim<'_>, from: usize, to: usize) -> u64 {
    let distance = (ctx.distance_sq(from, to) as f64).sqrt() as u64;
    let (radius, move_speed) = ctx
        .get_entity(to)
        .map_or((0, 0), |t| (t.radius() as u64, t.stat().move_speed as u64));
    let travel = distance.saturating_sub(SPARK_RADIUS + radius);
    (travel / HOP_TICKS).max(MIN_SPARK_SPEED) + move_speed
}

// Leaves `target` crackling after a hit. Anchored on the unit, so like every
// on-hit view effect here it uses none of `range`/`radius`/`time`.
fn play_shock(sim: &mut StableSim<'_>, caster: usize, target: usize) {
    sim.play_view_effect(
        SHOCK_EFFECT,
        caster,
        &InputTargetV1::target(target),
        0,
        0,
        0,
    );
}

// Lands one hop of Electrospark's chain lightning as magic damage.
#[derive(Clone, Debug)]
pub struct StatikkSpark {
    damage: usize,
    minion_percent: f64,
}

impl StableEffectType for StatikkSpark {
    fn apply(
        &self,
        sim: &mut StableSim<'_>,
        _rng_seed: u64,
        caster_id: usize,
        input: InputTargetV1,
    ) {
        let target = input.target_id;
        let Some(champion) = sim
            .get_entity(target)
            .filter(|t| t.is_alive())
            .map(|t| t.is_champion())
        else {
            return;
        };
        let damage = spark_damage(self.damage, self.minion_percent, champion);
        // Before the damage, so a hop that kills still leaves its crackle.
        play_shock(sim, caster_id, target);
        sim.deal_damage(caster_id, target, 0, damage, AttackTypeV1::Item);
    }

    fn expected_damage(&self, _caster_stat: &StatV1) -> (usize, usize) {
        (0, self.damage)
    }
}

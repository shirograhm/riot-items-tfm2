use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ItemMeta, ProcQueue, DISTANCE_UNITS_PER_RANGE};

// Cleave, scaled on the carrier's maximum health instead of Tiamat's Attack
// Damage: basic attacks hit their target for a share of it and every enemy
// behind the target for a larger share. The tower exclusion and the ranged
// falloff are Tiamat's and Ravenous Hydra's, so the three read as one Cleave,
// but where theirs splashes a circle around the target, this one splashes the
// wave League draws behind it.
//
// The wave is a wedge cut square at both ends: its short side runs across the
// target and it widens to a long, flat front `effect_max_distance` further
// on, directly away from the carrier. The splash hits every enemy whose body
// overlaps it, so it lands exactly where the picture is. View effects cannot
// be turned to face a direction, but projectiles are (their art faces +x), so
// the picture is a `Linear` projectile that passes through everything and
// does nothing; its art is drawn for the default reach of 35. The short side
// runs through the centre of its frame, so the projectile sits on the target
// and only creeps forward: it moves just so the engine knows which way to face
// it and when to remove it, and the burst itself is the animation. The damage
// still lands with the swing. With the carrier and the target on the same spot
// there is no behind, and the splash falls back to a circle of the same reach.
// A host without projectiles gets Tiamat's swing effect instead of the wave.

/// The `view_projectiles` name in `view/effects.view_effects` that draws the
/// wedge (`effects/titanic_hydra_wave`).
const WAVE_PROJECTILE: &str = "riot_titanic_hydra_wave";
/// How long the wedge stays up (0.3 s): the first five frames of its
/// animation. The sixth holds longer, so a late removal never loops it.
const WAVE_TICKS: u64 = 18;
/// How far the wedge creeps each tick, in world units (1.8 px in all).
const WAVE_DRIFT: u64 = 100;
/// Half the width of the wave's short side, across the target, in range.
const WAVE_SHORT_HALF_WIDTH: usize = 12;
/// How much each side of the wave widens per range it reaches past the
/// target: 12 either side at the target becomes 29.5 at the default 35.
const WAVE_SPREAD: f64 = 0.5;

#[derive(Clone, Debug)]
pub struct TitanicHydra {
    meta: ItemMeta,
    cleave_effect: &'static str,
    price: usize,
    attack: i32,
    hp: i32,
    effect_caster_hp_percent_damage: f64,
    effect_splash_caster_hp_percent: f64,
    effect_max_distance: usize,
    effect_melee_distance: usize,
    effect_ranged_percent: f64,
    // Non-vital stats (internals)
    procs: ProcQueue,
}

impl TitanicHydra {
    /// The native effect the wedge carries, for `lib.rs` to register.
    pub const WAVE_HIT: &'static str = "riot_titanic_hydra_wave_hit";

    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("titanic_hydra", &["tiamat"], &["radiant_titanic_hydra"]),
            cleave_effect: "riot_ravenous_hydra_cleave",
            price: 750,
            attack: 25,
            hp: 200,
            effect_caster_hp_percent_damage: 1.0,
            effect_splash_caster_hp_percent: 3.0,
            effect_max_distance: 35,
            effect_melee_distance: 35,
            effect_ranged_percent: 50.0,
            // Non-vital stats (internals)
            procs: ProcQueue::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_titanic_hydra", &["titanic_hydra"]),
            price: 1000,
            attack: 40,
            hp: 300,
            effect_caster_hp_percent_damage: 1.5,
            effect_splash_caster_hp_percent: 4.0,
            effect_max_distance: 35,
            effect_melee_distance: 35,
            effect_ranged_percent: 50.0,
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
                effect_caster_hp_percent_damage,
                effect_splash_caster_hp_percent,
                effect_max_distance,
                effect_melee_distance,
                effect_ranged_percent
            ]
        );
        self
    }

    /// The way the wave points: from the carrier through `target`, as a unit
    /// vector. None when the two stand on the same spot.
    fn away_from_carrier(ctx: &StableSim<'_>, caster: usize, target: usize) -> Option<(f64, f64)> {
        let (cx, cy) = ctx.get_entity(caster)?.pos();
        let (tx, ty) = ctx.get_entity(target)?.pos();
        let (dx, dy) = (tx as f64 - cx as f64, ty as f64 - cy as f64);
        let length = (dx * dx + dy * dy).sqrt();
        (length >= 1.0).then(|| (dx / length, dy / length))
    }

    /// The enemies the Cleave splashes, towers excepted: every one other than
    /// `target` whose body overlaps the wave pointing `away` from the carrier,
    /// or without a direction, every one within the wave's reach of `target`.
    fn splash_targets(
        &self,
        ctx: &StableSim<'_>,
        caster_team: usize,
        target: usize,
        away: Option<(f64, f64)>,
    ) -> Vec<usize> {
        let reach = (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        let short_half_width = (WAVE_SHORT_HALF_WIDTH * DISTANCE_UNITS_PER_RANGE) as f64;
        let Some((tx, ty)) = ctx.get_entity(target).map(|t| t.pos()) else {
            return Vec::new();
        };

        let mut splashed = Vec::new();
        for index in 0..ctx.entity_count() {
            let Some(entity_ref) = ctx.entity_at(index) else {
                continue;
            };
            let id = entity_ref.id();
            if id == target {
                continue;
            }
            // Towers are enemy entities too, and Cleave is not meant for them.
            if !entity_ref.is_alive() || entity_ref.is_tower() || entity_ref.team() == caster_team {
                continue;
            }
            let inside = match away {
                Some((ux, uy)) => {
                    // How far past the target along the wave's middle line,
                    // and how far off that line to either side.
                    let (ex, ey) = entity_ref.pos();
                    let (dx, dy) = (ex as f64 - tx as f64, ey as f64 - ty as f64);
                    let along = dx * ux + dy * uy;
                    let aside = (dx * uy - dy * ux).abs();
                    let body = entity_ref.radius() as f64;
                    let half_width =
                        short_half_width + WAVE_SPREAD * along.clamp(0.0, reach as f64);
                    along >= -body && along <= reach as f64 + body && aside <= half_width + body
                }
                None => ctx.distance_sq(target, id) <= reach * reach,
            };
            if inside {
                splashed.push(id);
            }
        }
        splashed
    }

    /// Bursts the wave out of `target` along `away`, the unit direction from
    /// the carrier through it. False when there is no projectile to draw it
    /// with.
    fn throw_wedge(
        &self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        team: usize,
        target: usize,
        (ux, uy): (f64, f64),
    ) -> bool {
        let Some((tx, ty)) = ctx.get_entity(target).map(|t| t.pos()) else {
            return false;
        };
        let drift = (WAVE_DRIFT * WAVE_TICKS) as f64;
        let spec = ProjectileSpawnV1 {
            caster_id: caster,
            team,
            x: tx,
            y: ty,
            radius: 1_000,
            speed: WAVE_DRIFT,
            move_kind: ProjectileMoveKindV1::Linear.code(),
            target_x: (tx as f64 + ux * drift).max(0.0) as u64,
            target_y: (ty as f64 + uy * drift).max(0.0) as u64,
            penetrate: true,
            attack_type: AttackTypeV1::Item.code(),
            ..ProjectileSpawnV1::default()
        };
        ctx.spawn_projectile(WAVE_PROJECTILE, Self::WAVE_HIT, &spec)
    }
}

impl Default for TitanicHydra {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for TitanicHydra {
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
        damage: &mut usize,
        damage_type: DamageTypeV1,
        attack_type: AttackTypeV1,
        is_crit: bool,
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
        let Some((caster_team, max_hp)) = ctx.get_entity(caster).map(|c| (c.team(), c.hp().1))
        else {
            return;
        };
        let mut on_hit = percent_of(max_hp, self.effect_caster_hp_percent_damage);
        let mut splash = percent_of(max_hp, self.effect_splash_caster_hp_percent);

        let reach = (self.effect_melee_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        if ctx.distance_sq(caster, target) > reach * reach {
            on_hit = percent_of(on_hit, self.effect_ranged_percent);
            splash = percent_of(splash, self.effect_ranged_percent);
        }

        self.procs
            .on_hit_physical(ctx, target, damage, damage_type, is_crit, on_hit);
        if splash == 0 {
            return;
        }

        let away = Self::away_from_carrier(ctx, caster, target);
        let splashed = self.splash_targets(ctx, caster_team, target, away);
        let drawn =
            away.is_some_and(|away| self.throw_wedge(ctx, caster, caster_team, target, away));
        if !drawn {
            ctx.play_view_effect(
                self.cleave_effect,
                caster,
                &InputTargetV1::target(target),
                0,
                0,
                0,
            );
        }

        for id in splashed {
            ctx.deal_damage(caster, id, splash, 0, AttackTypeV1::Item);
        }
    }

    /// Lands the on-hit damage whose delay has run out.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.procs.update(ctx, player);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::Hp, ItemTagV1::MyHpPercentDamage]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

/// What the Cleave wedge does to whatever it passes through: nothing. The
/// wedge is only the picture; `spawn_projectile` still needs an effect for
/// it to carry.
#[derive(Clone, Debug)]
pub struct TitanicWave;

impl StableEffectType for TitanicWave {
    fn apply(
        &self,
        _sim: &mut StableSim<'_>,
        _rng_seed: u64,
        _caster_id: usize,
        _input: InputTargetV1,
    ) {
    }
}

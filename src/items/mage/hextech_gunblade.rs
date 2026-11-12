use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, refresh_buff, ticks, ItemMeta, DISTANCE_UNITS_PER_RANGE};

// Lightning Bolt's slow, shared by both tiers.
const SLOW_BUFF: &str = "hextech_gunblade_slow";
// The burst on the shocked champion (`effects/gunblade_impact`, a
// `view_effects` Animation, 0.4 s, following the unit).
const IMPACT_EFFECT: &str = "riot_gunblade_impact";
// The beam from the carrier to the target, after League's: segments of
// `effects/gunblade_beam` laid end to end along the line, each a near-still
// projectile (projectiles are the only pictures that turn to face a
// direction). Three `view_projectiles` bind the sheet's three crackle
// variants, taken in turn so a long beam does not repeat.
const BEAM_PROJECTILES: [&str; 3] = [
    "riot_gunblade_beam_a",
    "riot_gunblade_beam_b",
    "riot_gunblade_beam_c",
];
// A segment's length in the art, in range (1 range = 1 px). Segments are
// spaced at most this far apart, so they meet or overlap a little.
const BEAM_SEGMENT: usize = 24;
// At most this many segments: 12 x 24 px is far past any attack range.
const BEAM_MAX_SEGMENTS: usize = 12;
// How long a segment lives, matching its art's 6 x 0.05 s, and how far it
// creeps along the beam meanwhile (100 x 18 = 1.8 px, Titanic Hydra's wave).
const BEAM_TICKS: u64 = 18;
const BEAM_DRIFT: u64 = 100;

#[derive(Clone, Debug)]
pub struct HextechGunblade {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    magic_power: i32,
    vamp: i32,
    effect_hp_percent_threshold: f64,
    effect_bonus_flat_damage: usize,
    effect_ap_percent_damage: f64,
    effect_slow_amount: i32,
    effect_slow_seconds: f64,
    effect_cooldown_seconds: f64,
    // Non-vital stats (internals)
    cooldown: usize,
}

impl HextechGunblade {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "hextech_gunblade",
                &["ruinous_blade", "hextech_alternator"],
                &["radiant_hextech_gunblade"],
            ),
            price: 750,
            attack: 25,
            magic_power: 50,
            vamp: 10,
            effect_hp_percent_threshold: 50.0,
            effect_bonus_flat_damage: 120,
            effect_ap_percent_damage: 20.0,
            effect_slow_amount: 25,
            effect_slow_seconds: 1.5,
            effect_cooldown_seconds: 40.0,
            // Non-vital stats (internals)
            cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_hextech_gunblade", &["hextech_gunblade"]),
            price: 1050,
            attack: 45,
            magic_power: 75,
            vamp: 15,
            effect_bonus_flat_damage: 180,
            effect_ap_percent_damage: 30.0,
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
                vamp,
                effect_hp_percent_threshold,
                effect_bonus_flat_damage,
                effect_ap_percent_damage,
                effect_slow_amount,
                effect_slow_seconds,
                effect_cooldown_seconds
            ]
        );
        self
    }
}

impl Default for HextechGunblade {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for HextechGunblade {
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
            vamp: self.vamp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.cooldown = 0;
    }

    // Lightning Bolt: the carrier's own damage (a basic attack or an Ability,
    // damage over time included) to an enemy champion already under the
    // health threshold shocks it. The bolt lands as an `Item` hit, which this
    // does not answer to.
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
        let own_damage = matches!(
            attack_type,
            AttackTypeV1::BaseAttack
                | AttackTypeV1::Skill
                | AttackTypeV1::Dot
                | AttackTypeV1::DotIgnoreShield
        );
        if !own_damage || self.cooldown > 0 {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }
        let (current_hp, max_hp) = target_ref.hp();
        if current_hp == 0 || current_hp >= percent_of(max_hp, self.effect_hp_percent_threshold) {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let bolt = self.effect_bonus_flat_damage
            + percent_of(caster_ref.stat().magic_power, self.effect_ap_percent_damage);

        self.cooldown = ticks(self.effect_cooldown_seconds);
        ctx.deal_damage(caster, target, 0, bolt, AttackTypeV1::Item);
        refresh_buff(
            ctx,
            target,
            SLOW_BUFF,
            &BuffV1 {
                move_speed_mult: -self.effect_slow_amount,
                ..BuffV1::timed(SLOW_BUFF, ticks(self.effect_slow_seconds))
            },
        );
        draw_beam(ctx, caster, target);
        ctx.play_view_effect(
            IMPACT_EFFECT,
            caster,
            &InputTargetV1::target(target),
            0,
            0,
            0,
        );
    }

    fn update(&mut self, _ctx: &mut StableSim<'_>, _rng_seed: u64, _player: usize) {
        self.cooldown = self.cooldown.saturating_sub(1);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::Ap, ItemTagV1::Vamp]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}

// Lays Lightning Bolt's beam from the edge of the carrier's body to the
// target: as few segments as cover the gap, spaced evenly, each centred on
// its share of the line and creeping a hair along it so it is drawn facing
// the target. Nothing is drawn where the target stands inside the carrier.
fn draw_beam(ctx: &mut StableSim<'_>, caster: usize, target: usize) {
    let Some((team, (cx, cy), body)) = ctx
        .get_entity(caster)
        .map(|c| (c.team(), c.pos(), c.radius() as f64))
    else {
        return;
    };
    let Some((tx, ty)) = ctx.get_entity(target).map(|t| t.pos()) else {
        return;
    };
    let (dx, dy) = (tx as f64 - cx as f64, ty as f64 - cy as f64);
    let distance = dx.hypot(dy);
    let length = distance - body;
    if length <= 0.0 {
        return;
    }
    let (ux, uy) = (dx / distance, dy / distance);
    let segment = (BEAM_SEGMENT * DISTANCE_UNITS_PER_RANGE) as f64;
    let count = ((length / segment).ceil() as usize).clamp(1, BEAM_MAX_SEGMENTS);
    let spacing = length / count as f64;
    let drift = (BEAM_DRIFT * BEAM_TICKS) as f64;
    for i in 0..count {
        let along = body + spacing * (i as f64 + 0.5);
        let (x, y) = (cx as f64 + ux * along, cy as f64 + uy * along);
        let spec = ProjectileSpawnV1 {
            caster_id: caster,
            team,
            x: x.max(0.0) as u64,
            y: y.max(0.0) as u64,
            radius: 1_000,
            speed: BEAM_DRIFT,
            move_kind: ProjectileMoveKindV1::Linear.code(),
            target_x: (x + ux * drift).max(0.0) as u64,
            target_y: (y + uy * drift).max(0.0) as u64,
            penetrate: true,
            attack_type: AttackTypeV1::Item.code(),
            ..ProjectileSpawnV1::default()
        };
        ctx.spawn_projectile(
            BEAM_PROJECTILES[i % BEAM_PROJECTILES.len()],
            GunbladeBeam::HIT,
            &spec,
        );
    }
}

// What a beam segment does to whatever it passes through: nothing. The beam
// is only the picture (the bolt's damage has landed already);
// `spawn_projectile` still needs an effect for it to carry.
#[derive(Clone, Debug)]
pub struct GunbladeBeam;

impl GunbladeBeam {
    pub const HIT: &'static str = "riot_gunblade_beam_hit";
}

impl StableEffectType for GunbladeBeam {
    fn apply(
        &self,
        _sim: &mut StableSim<'_>,
        _rng_seed: u64,
        _caster_id: usize,
        _input: InputTargetV1,
    ) {
    }
}

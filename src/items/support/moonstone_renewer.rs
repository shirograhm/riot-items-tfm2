use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, HealWatch, ItemMeta, DISTANCE_UNITS_PER_RANGE};

/// The `view_projectiles` name in `view/effects.view_effects` that draws the
/// wisp flying from the ally who was healed to the ally the heal chains to
/// (`effects/moonstone_wisp`).
const WISP_PROJECTILE: &str = "riot_moonstone_wisp";
/// The wisp's hit circle: small, so it only lands on the ally it was sent to.
const WISP_RADIUS: u64 = 1_000;
/// How long the wisp is in the air, whatever the distance (1/6 s).
const WISP_TICKS: u64 = 10;
/// The slowest the wisp flies, in world units per tick: two allies standing
/// together still get a visible flight rather than a hop.
const WISP_MIN_SPEED: u64 = 500;

/// Moonstone Renewer — Starlit Grace, a share of every heal the carrier gives
/// an ally passed on to another ally near them.
///
/// # Where the amounts come from
///
/// The host says that an ally-targeted skill was cast and at whom, never what
/// it did. So the heal is read back afterwards from the carrier's own healing
/// statistics ([`HealWatch`]): the host credits the carrier with the health
/// they really restored to another champion, so both who was healed and by how
/// much are the host's own figures. A skill cast on the carrier that heals
/// around them counts as well.
///
/// Shields are not chained. Riot's item does, but the host credits nothing as
/// a shield is given, so one could only be guessed at.
///
/// # The wisp is only a picture
///
/// The chained heal lands at once and a wisp is sent after it from the ally
/// who was healed. It could not carry the heal: a projectile has no payload,
/// so the amount would have to be worked out again on arrival, and a heal that
/// lands later would be read back as one of the carrier's to chain.
///
/// # A chained heal does not chain again
///
/// It is given in the carrier's name, so the host credits it like any other;
/// the watch is read again straight after, or it would come back next tick as
/// a heal to chain.
///
/// The passive is the same on base and Radiant; Radiant buys the stat line.
#[derive(Clone, Debug)]
pub struct MoonstoneRenewer {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    hp_regen: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_heal_chain_percent: f64,
    effect_max_distance: usize,
    // Non-vital stats (internals)
    heals: HealWatch,
}

impl MoonstoneRenewer {
    /// The native effect the wisp carries, registered in `lib.rs`.
    pub const WISP_HIT: &'static str = "riot_moonstone_wisp_hit";

    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "moonstone_renewer",
                &["bandleglass_mirror"],
                &["radiant_moonstone_renewer"],
            ),
            price: 550,
            hp: 150,
            hp_regen: 2,
            magic_power: 20,
            skill_cooldown_mult: 15,
            effect_heal_chain_percent: 35.0,
            effect_max_distance: 100,
            // Non-vital stats (internals)
            heals: HealWatch::default(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_moonstone_renewer", &["moonstone_renewer"]),
            price: 750,
            hp: 250,
            hp_regen: 3,
            magic_power: 30,
            skill_cooldown_mult: 20,
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
                hp_regen,
                magic_power,
                skill_cooldown_mult,
                effect_heal_chain_percent,
                effect_max_distance
            ]
        );
        self
    }

    /// Who a heal on `from` chains to: the nearest living allied champion
    /// within range of `from`, the carrier and `from` aside. The range is
    /// measured from the ally, not the carrier, so the carrier's size does not
    /// stretch it.
    fn chain_target(&self, ctx: &StableSim<'_>, caster: usize, from: usize) -> Option<usize> {
        let caster_team = ctx.get_entity(caster)?.team();
        let range = (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        let range_sq = range * range;

        let mut best: Option<(usize, u64)> = None;
        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            if id == caster || id == from {
                continue;
            }
            let Some(ally_ref) = ctx.get_entity(id) else {
                continue;
            };
            if !ally_ref.is_alive() || !ally_ref.is_champion() || ally_ref.team() != caster_team {
                continue;
            }
            let distance = ctx.distance_sq(from, id);
            if distance > range_sq {
                continue;
            }
            if best.is_none_or(|(_, best_distance)| distance < best_distance) {
                best = Some((id, distance));
            }
        }
        best.map(|(id, _)| id)
    }

    /// Sends the wisp from `from` to `to`. Its speed is Statikk Shiv's spark
    /// timing: the gap between the two hit circles spread over `WISP_TICKS`,
    /// plus the ally's own move speed, so one running away is still caught.
    fn send_wisp(ctx: &mut StableSim<'_>, caster: usize, from: usize, to: usize) {
        let Some(team) = ctx.get_entity(caster).map(|c| c.team()) else {
            return;
        };
        let Some((x, y)) = ctx.get_entity(from).map(|f| f.pos()) else {
            return;
        };
        let distance = (ctx.distance_sq(from, to) as f64).sqrt() as u64;
        let (radius, move_speed) = ctx
            .get_entity(to)
            .map_or((0, 0), |t| (t.radius() as u64, t.stat().move_speed as u64));
        let travel = distance.saturating_sub(WISP_RADIUS + radius);
        let spec = ProjectileSpawnV1 {
            caster_id: caster,
            team,
            x,
            y,
            radius: WISP_RADIUS,
            speed: (travel / WISP_TICKS).max(WISP_MIN_SPEED) + move_speed,
            move_kind: ProjectileMoveKindV1::Target.code(),
            target_id: to,
            attack_type: AttackTypeV1::Item.code(),
            casting_target: CastingTargetV1::AllyChampion.code(),
            ..ProjectileSpawnV1::default()
        };
        ctx.spawn_projectile(WISP_PROJECTILE, Self::WISP_HIT, &spec);
    }
}

impl Default for MoonstoneRenewer {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for MoonstoneRenewer {
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
            hp_regen: self.hp_regen,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.heals.close();
    }

    // Starlit Grace. `is_ally` marks an ally-targeted skill — a heal, shield or
    // buff — and nothing has landed yet when this fires, so the heal is watched
    // for. Whoever the skill was cast on, the carrier included: a heal cast on
    // the carrier can reach the allies around them.
    fn on_skill_hit(
        &mut self,
        ctx: &mut StableSim<'_>,
        _rng_seed: u64,
        caster: usize,
        _target: usize,
        is_ally: bool,
    ) {
        if is_ally {
            self.heals.open(ctx, caster);
        }
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        let healed = self.heals.poll(ctx);
        if healed.is_empty() {
            return;
        }
        let Some(caster) = ctx
            .get_player(player)
            .and_then(|player_ref| player_ref.champion())
            .map(|champion_ref| champion_ref.id())
        else {
            return;
        };

        for heal in healed {
            let amount = percent_of(heal.amount, self.effect_heal_chain_percent);
            if amount == 0 {
                continue;
            }
            if let Some(other) = self.chain_target(ctx, caster, heal.ally) {
                ctx.heal(caster, other, amount);
                Self::send_wisp(ctx, caster, heal.ally, other);
            }
        }
        self.heals.resync(ctx);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::HpRegen,
            ItemTagV1::Ap,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}

/// What the wisp does when it reaches the ally: nothing. The heal has landed
/// already; `spawn_projectile` still needs an effect for the wisp to carry.
#[derive(Clone, Debug)]
pub struct MoonstoneWisp;

impl StableEffectType for MoonstoneWisp {
    fn apply(
        &self,
        _sim: &mut StableSim<'_>,
        _rng_seed: u64,
        _caster_id: usize,
        _input: InputTargetV1,
    ) {
    }
}

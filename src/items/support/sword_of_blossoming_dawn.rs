use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, sized_range, ItemMeta};

// The `view_projectiles` name in `view/effects.view_effects` that draws the
// wisp flying from the carrier to the ally Peppermint heals
// (`effects/blossoming_dawn_wisp`).
const WISP_PROJECTILE: &str = "riot_blossoming_dawn_wisp";
// The wisp's hit circle: small, so it only lands on the ally it was sent to.
const WISP_RADIUS: u64 = 1_000;
// How long the wisp takes to reach the ally, whatever the distance (1/6 s).
const WISP_TICKS: u64 = 10;
// The slowest the wisp flies, in world units per tick: an ally right next to
// the carrier still gets a visible flight rather than a hop.
const WISP_MIN_SPEED: u64 = 500;

#[derive(Clone, Debug)]
pub struct SwordOfBlossomingDawn {
    meta: ItemMeta,
    // The name this tier's [`BlossomingDawnWisp`] is registered under.
    wisp_hit: &'static str,
    price: usize,
    attack_speed_mult: i32,
    hp: i32,
    magic_power: i32,
    skill_cooldown_mult: i32,
    effect_min_heal: usize,
    effect_max_heal: usize,
    effect_ad_percent_heal: f64,
    effect_ap_percent_heal: f64,
    effect_max_distance: usize,
}

impl SwordOfBlossomingDawn {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "sword_of_blossoming_dawn",
                &["forbidden_idol"],
                &["radiant_sword_of_blossoming_dawn"],
            ),
            wisp_hit: "riot_blossoming_dawn_wisp_hit",
            price: 500,
            attack_speed_mult: 20,
            hp: 100,
            magic_power: 20,
            skill_cooldown_mult: 10,
            effect_min_heal: 15,
            effect_max_heal: 60,
            effect_ad_percent_heal: 7.0,
            effect_ap_percent_heal: 7.0,
            effect_max_distance: 100,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant(
                "radiant_sword_of_blossoming_dawn",
                &["sword_of_blossoming_dawn"],
            ),
            wisp_hit: "riot_radiant_blossoming_dawn_wisp_hit",
            price: 850,
            attack_speed_mult: 35,
            hp: 150,
            magic_power: 35,
            skill_cooldown_mult: 15,
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
                hp,
                magic_power,
                skill_cooldown_mult,
                effect_min_heal,
                effect_max_heal,
                effect_ad_percent_heal,
                effect_ap_percent_heal,
                effect_max_distance
            ]
        );
        self
    }

    // This tier's Peppermint heal, as the effect the wisp carries to the ally.
    // For `lib.rs` to register under [`Self::wisp_hit_name`].
    pub fn wisp_hit(&self) -> BlossomingDawnWisp {
        BlossomingDawnWisp {
            min_heal: self.effect_min_heal,
            max_heal: self.effect_max_heal,
            ad_percent: self.effect_ad_percent_heal,
            ap_percent: self.effect_ap_percent_heal,
        }
    }

    pub fn wisp_hit_name(&self) -> &'static str {
        self.wisp_hit
    }

    // Sends the wisp, and with it the heal, from the carrier to `ally`. False
    // when there is no projectile to send it with. Its speed is Statikk Shiv's
    // spark timing: the gap between the two hit circles spread over
    // `WISP_TICKS`, plus the ally's own move speed, so one running away is
    // still caught.
    fn send_wisp(&self, ctx: &mut StableSim<'_>, caster: usize, team: usize, ally: usize) -> bool {
        let Some((x, y)) = ctx.get_entity(caster).map(|c| c.pos()) else {
            return false;
        };
        let distance = (ctx.distance_sq(caster, ally) as f64).sqrt() as u64;
        let (radius, move_speed) = ctx
            .get_entity(ally)
            .map_or((0, 0), |a| (a.radius() as u64, a.stat().move_speed as u64));
        let travel = distance.saturating_sub(WISP_RADIUS + radius);
        let spec = ProjectileSpawnV1 {
            caster_id: caster,
            team,
            x,
            y,
            radius: WISP_RADIUS,
            speed: (travel / WISP_TICKS).max(WISP_MIN_SPEED) + move_speed,
            move_kind: ProjectileMoveKindV1::Target.code(),
            target_id: ally,
            attack_type: AttackTypeV1::Item.code(),
            casting_target: CastingTargetV1::AllyChampion.code(),
            ..ProjectileSpawnV1::default()
        };
        ctx.spawn_projectile(WISP_PROJECTILE, self.wisp_hit, &spec)
    }
}

impl Default for SwordOfBlossomingDawn {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for SwordOfBlossomingDawn {
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
            hp: self.hp,
            magic_power: self.magic_power,
            skill_cooldown_mult: self.skill_cooldown_mult,
            ..Default::default()
        }
    }

    // Basic attacks heal the most wounded ally in range. "Most wounded" is the
    // lowest fraction of maximum health rather than the lowest number, so a
    // chipped tank does not outrank a nearly dead carry; distance only breaks
    // ties. The carrier is not a candidate, matching the other ally-facing
    // items here (`locket_of_the_iron_solari`). The heal rides a wisp to the
    // ally and lands when it arrives.
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
        let Some((level, attack, magic_power, caster_team)) =
            ctx.get_entity(caster).map(|caster_ref| {
                let stat = caster_ref.stat();
                (
                    caster_ref.level(),
                    stat.attack,
                    stat.magic_power,
                    caster_ref.team(),
                )
            })
        else {
            return;
        };

        let range = sized_range(ctx, caster, self.effect_max_distance);
        let range_sq = range * range;

        let mut best: Option<(usize, f64, u64)> = None;
        for index in 0..ctx.champion_count() {
            let id = ctx.champion_id_at(index);
            if id == caster {
                continue;
            }
            let Some(ally_ref) = ctx.get_entity(id) else {
                continue;
            };
            if !ally_ref.is_alive() || ally_ref.team() != caster_team {
                continue;
            }
            let distance = ctx.distance_sq(caster, id);
            if distance > range_sq {
                continue;
            }
            let (current, max) = ally_ref.hp();
            if max == 0 {
                continue;
            }
            let wounded = current as f64 / max as f64;
            let better = best.is_none_or(|(_, best_wounded, best_distance)| {
                wounded < best_wounded || (wounded == best_wounded && distance < best_distance)
            });
            if better {
                best = Some((id, wounded, distance));
            }
        }

        let Some((ally, _, _)) = best else {
            return;
        };
        // With no wisp to carry it, the heal lands at once.
        if !self.send_wisp(ctx, caster, caster_team, ally) {
            let heal = self.wisp_hit().amount(level, attack, magic_power);
            ctx.heal(caster, ally, heal);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![
            ItemTagV1::Hp,
            ItemTagV1::Ap,
            ItemTagV1::AttackSpeed,
            ItemTagV1::CooltimeReduce,
        ]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}

// Peppermint's heal, carried by the wisp: when it reaches the ally, it heals
// them by the carrier's heal as it stands then. One per tier, with that tier's
// numbers, since a projectile carries no payload of its own.
#[derive(Clone, Debug)]
pub struct BlossomingDawnWisp {
    min_heal: usize,
    max_heal: usize,
    ad_percent: f64,
    ap_percent: f64,
}

impl BlossomingDawnWisp {
    // Level 1 pays `min_heal` and level 12 pays `max_heal`, the same
    // eleven-step ramp `bloodsong` uses for Spellblade, plus shares of the
    // carrier's Attack Damage and Ability Power.
    fn amount(&self, level: usize, attack: usize, magic_power: usize) -> usize {
        let per_level = ((self.max_heal - self.min_heal) as f64 / 11.0).round() as usize;
        self.min_heal
            + level.saturating_sub(1) * per_level
            + percent_of(attack, self.ad_percent)
            + percent_of(magic_power, self.ap_percent)
    }
}

impl StableEffectType for BlossomingDawnWisp {
    fn apply(
        &self,
        sim: &mut StableSim<'_>,
        _rng_seed: u64,
        caster_id: usize,
        input: InputTargetV1,
    ) {
        let ally = input.target_id;
        if !sim.get_entity(ally).is_some_and(|a| a.is_alive()) {
            return;
        }
        let Some(heal) = sim.get_entity(caster_id).map(|carrier| {
            let stat = carrier.stat();
            self.amount(carrier.level(), stat.attack, stat.magic_power)
        }) else {
            return;
        };
        sim.heal(caster_id, ally, heal);
    }
}

use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, refresh_buff, sized_range, ticks, ItemMeta, SharedRiches};

/// Celestial Opposition — what World Atlas grows into for a support that
/// stands in front and is hit: Blessing of the Mountain, and the gold the
/// Atlas line pays ([`SharedRiches`]).
///
/// # Blessing of the Mountain
///
/// Damage from an enemy champion makes the carrier Blessed: for a short while
/// everything it takes is cut by a share. The hit that brings the blessing on
/// has already landed, so it is the ones after it that are cut. When Blessed
/// runs out the carrier lets a shockwave go, which slows every enemy unit
/// around it, and only then does the cooldown start.
///
/// A carrier that dies Blessed lets nothing go: the blessing ends with it and
/// the cooldown starts there.
#[derive(Clone, Debug)]
pub struct CelestialOpposition {
    meta: ItemMeta,
    price: usize,
    hp: i32,
    hp_regen: i32,
    effect_damaged_reduce: usize,
    effect_duration_seconds: f64,
    effect_max_distance: usize,
    effect_slow_amount: i32,
    effect_slow_seconds: f64,
    effect_cooldown_seconds: f64,
    effect_bonus_gold: usize,
    effect_gold_interval_seconds: f64,
    // Non-vital stats (internals)
    /// Ticks of Blessed left; zero when the carrier is not Blessed.
    blessed: usize,
    /// Ticks until Blessing of the Mountain can come on again.
    cooldown: usize,
    riches: SharedRiches,
}

/// Blessed: the cut in damage taken, and the name the `view_buffs` binding in
/// `view/effects.view_effects` draws the golden shield under
/// (`effects/celestial_shield`). Shared by both variants.
const BLESSED_BUFF: &str = "celestial_opposition_blessed";
/// The shockwave's slow. One name for both variants and every carrier, so two
/// shockwaves refresh one slow on a target rather than stacking two.
const SLOW_BUFF: &str = "celestial_opposition_slow";
/// The shield breaking, played on the carrier (`effects/celestial_shatter`).
/// It is the shockwave's picture too: the pieces fly out as far as the slow
/// reaches, so there is the one burst and not two. Bound in
/// `view/effects.view_effects`.
const SHATTER_EFFECT: &str = "riot_celestial_shatter";

impl CelestialOpposition {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "celestial_opposition",
                &["runic_compass"],
                &["radiant_celestial_opposition"],
            ),
            price: 550,
            hp: 200,
            hp_regen: 4,
            effect_damaged_reduce: 15,
            effect_duration_seconds: 2.0,
            effect_max_distance: 50,
            effect_slow_amount: 50,
            effect_slow_seconds: 1.5,
            effect_cooldown_seconds: 18.0,
            effect_bonus_gold: 4,
            effect_gold_interval_seconds: 5.0,
            // Non-vital stats (internals)
            blessed: 0,
            cooldown: 0,
            riches: SharedRiches::default(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_celestial_opposition", &["celestial_opposition"]),
            price: 750,
            hp: 300,
            hp_regen: 5,
            // Blessing of the Mountain itself is unchanged — Radiant buys the
            // stat line only.
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
                effect_damaged_reduce,
                effect_duration_seconds,
                effect_max_distance,
                effect_slow_amount,
                effect_slow_seconds,
                effect_cooldown_seconds,
                effect_bonus_gold,
                effect_gold_interval_seconds
            ]
        );
        self
    }

    /// Counts Blessed down, and ends it: with the shockwave when it runs out,
    /// with nothing when the carrier died under it. Either way the cooldown
    /// starts there.
    fn run_blessed(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        let Some((carrier, team, alive)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| (c.id(), c.team(), c.is_alive()))
        else {
            self.blessed = 0;
            return;
        };
        if alive {
            self.blessed -= 1;
            if self.blessed > 0 {
                return;
            }
        }
        self.blessed = 0;
        self.cooldown = ticks(self.effect_cooldown_seconds);
        // The buff is timed to run out on this tick anyway; taking it off here
        // has the shield gone exactly as it is seen to break.
        ctx.entity_remove_buff(carrier, BLESSED_BUFF);
        if alive {
            self.shockwave(ctx, carrier, team);
        }
    }

    /// Slows every enemy unit (not turrets) within range of the carrier, the
    /// range stretched by the carrier's size ([`sized_range`]).
    fn shockwave(&self, ctx: &mut StableSim<'_>, carrier: usize, team: usize) {
        let reach = sized_range(ctx, carrier, self.effect_max_distance);
        let reach_sq = reach * reach;
        let targets: Vec<usize> = (0..ctx.entity_count())
            .filter_map(|index| ctx.entity_at(index))
            .filter(|e| e.is_alive() && !e.is_tower() && e.team() != team)
            .map(|e| e.id())
            .filter(|&id| ctx.distance_sq(carrier, id) <= reach_sq)
            .collect();

        let slow = BuffV1 {
            move_speed_mult: -self.effect_slow_amount,
            ..BuffV1::timed(SLOW_BUFF, ticks(self.effect_slow_seconds))
        };
        for target in targets {
            refresh_buff(ctx, target, SLOW_BUFF, &slow);
        }

        ctx.play_view_effect(
            SHATTER_EFFECT,
            carrier,
            &InputTargetV1::target(carrier),
            0,
            0,
            0,
        );
    }
}

impl Default for CelestialOpposition {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for CelestialOpposition {
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
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.blessed = 0;
        self.cooldown = 0;
    }

    /// Blessing of the Mountain's trigger: damage from an enemy champion, off
    /// cooldown and not already Blessed. A hit that killed the carrier
    /// blesses nobody.
    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        attacker: usize,
        damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        if self.blessed > 0 || self.cooldown > 0 || damage == 0 {
            return;
        }
        let Some(team) = ctx
            .get_entity(entity)
            .filter(|carrier| carrier.is_alive())
            .map(|carrier| carrier.team())
        else {
            return;
        };
        if !ctx
            .get_entity(attacker)
            .is_some_and(|attacker| attacker.is_champion() && attacker.team() != team)
        {
            return;
        }

        self.blessed = ticks(self.effect_duration_seconds).max(1);
        refresh_buff(
            ctx,
            entity,
            BLESSED_BUFF,
            &BuffV1 {
                damaged_reduce: self.effect_damaged_reduce,
                ..BuffV1::timed(BLESSED_BUFF, self.blessed)
            },
        );
    }

    /// Runs the cooldown and Blessed, and pays Shared Riches.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        self.cooldown = self.cooldown.saturating_sub(1);
        if self.blessed > 0 {
            self.run_blessed(ctx, player);
        }
        self.riches.update(
            ctx,
            player,
            self.effect_bonus_gold,
            self.effect_gold_interval_seconds,
        );
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::HpRegen, ItemTagV1::MoveSpeed]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Support
    }
}

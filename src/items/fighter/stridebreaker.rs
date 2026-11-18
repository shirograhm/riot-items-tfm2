use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, cleave_targets, percent_of, refresh_buff, ticks, ItemMeta,
    DISTANCE_UNITS_PER_RANGE,
};

// The shockwave's slow: one buff per enemy, refreshed by a later shockwave
// rather than stacked.
const SLOW_BUFF: &str = "stridebreaker_slow";
// A circular slash round the champion the shockwave centres on, out to its
// 35 range, drawn under it (docs/vfx/make_stridebreaker_shockwave.py).
const SHOCKWAVE_EFFECT: &str = "riot_stridebreaker_shockwave";
// The shockwave's reach round the champion, in range units. Fixed, not in the
// config: the effect is drawn at exactly this size.
const SHOCKWAVE_RANGE: usize = 35;

// Stridebreaker: Tiamat's Cleave, and Breaking Shockwave made a passive (no
// League active): damaging an enemy champion at or under the health threshold
// sends a shockwave out around them that deals % AD physical damage and slows
// every enemy within [`SHOCKWAVE_RANGE`] of them, on a cooldown.
#[derive(Clone, Debug)]
pub struct Stridebreaker {
    meta: ItemMeta,
    cleave_effect: &'static str,
    price: usize,
    attack: i32,
    attack_speed_mult: i32,
    hp: i32,
    effect_ad_percent_damage: f64,
    effect_max_distance: usize,
    effect_melee_distance: usize,
    effect_ranged_percent: f64,
    effect_hp_percent_threshold: f64,
    effect_shockwave_ad_percent_damage: f64,
    effect_slow_amount: i32,
    effect_slow_seconds: f64,
    effect_cooldown_seconds: f64,
    cooldown: usize,
}

impl Stridebreaker {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "stridebreaker",
                &["phage", "tiamat"],
                &["radiant_stridebreaker"],
            ),
            cleave_effect: "riot_ravenous_hydra_cleave",
            price: 750,
            attack: 25,
            attack_speed_mult: 10,
            hp: 200,
            effect_ad_percent_damage: 20.0,
            effect_max_distance: 35,
            effect_melee_distance: 35,
            effect_ranged_percent: 50.0,
            effect_hp_percent_threshold: 50.0,
            effect_shockwave_ad_percent_damage: 80.0,
            effect_slow_amount: 35,
            effect_slow_seconds: 3.0,
            effect_cooldown_seconds: 15.0,
            cooldown: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_stridebreaker", &["stridebreaker"]),
            price: 1000,
            attack: 40,
            attack_speed_mult: 15,
            hp: 300,
            effect_ad_percent_damage: 30.0,
            effect_shockwave_ad_percent_damage: 120.0,
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
                attack_speed_mult,
                hp,
                effect_ad_percent_damage,
                effect_max_distance,
                effect_melee_distance,
                effect_ranged_percent,
                effect_hp_percent_threshold,
                effect_shockwave_ad_percent_damage,
                effect_slow_amount,
                effect_slow_seconds,
                effect_cooldown_seconds
            ]
        );
        self
    }

    fn cleave(&self, ctx: &mut StableSim<'_>, caster: usize, target: usize) {
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let caster_team = caster_ref.team();
        let mut splash = percent_of(caster_ref.stat().attack, self.effect_ad_percent_damage);

        let reach = (self.effect_melee_distance * DISTANCE_UNITS_PER_RANGE) as u64;
        if ctx.distance_sq(caster, target) > reach * reach {
            splash = percent_of(splash, self.effect_ranged_percent);
        }
        if splash == 0 {
            return;
        }

        let splashed = cleave_targets(ctx, caster_team, target, self.effect_max_distance);
        ctx.play_view_effect(
            self.cleave_effect,
            caster,
            &InputTargetV1::target(target),
            0,
            0,
            0,
        );
        for id in splashed {
            ctx.deal_damage(caster, id, splash, 0, AttackTypeV1::Item);
        }
    }

    // Breaking Shockwave around `target`, an enemy champion the carrier just
    // damaged, when it is at or under the threshold and the cooldown is up.
    fn shockwave(&mut self, ctx: &mut StableSim<'_>, caster: usize, target: usize) {
        if self.cooldown > 0 {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() || !target_ref.is_alive() {
            return;
        }
        let (current_hp, max_hp) = target_ref.hp();
        if current_hp == 0 || current_hp > percent_of(max_hp, self.effect_hp_percent_threshold) {
            return;
        }
        let Some(caster_ref) = ctx.get_entity(caster) else {
            return;
        };
        let caster_team = caster_ref.team();
        let damage = percent_of(
            caster_ref.stat().attack,
            self.effect_shockwave_ad_percent_damage,
        );

        self.cooldown = ticks(self.effect_cooldown_seconds);
        let mut hit = vec![target];
        hit.extend(cleave_targets(
            ctx,
            caster_team,
            target,
            SHOCKWAVE_RANGE,
        ));
        ctx.play_view_effect(
            SHOCKWAVE_EFFECT,
            caster,
            &InputTargetV1::target(target),
            0,
            0,
            0,
        );
        let slow = BuffV1 {
            move_speed_mult: -self.effect_slow_amount,
            ..BuffV1::timed(SLOW_BUFF, ticks(self.effect_slow_seconds))
        };
        for id in hit {
            if damage > 0 {
                ctx.deal_damage(caster, id, damage, 0, AttackTypeV1::Item);
            }
            refresh_buff(ctx, id, SLOW_BUFF, &slow);
        }
    }
}

impl Default for Stridebreaker {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Stridebreaker {
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
            attack_speed_mult: self.attack_speed_mult,
            hp: self.hp,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.cooldown = 0;
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
        // The carrier's own damage only: Cleave's splash and the shockwave are
        // Item damage, so neither sets the shockwave off.
        let own_damage = matches!(
            attack_type,
            AttackTypeV1::BaseAttack
                | AttackTypeV1::Skill
                | AttackTypeV1::Dot
                | AttackTypeV1::DotIgnoreShield
        );
        if !own_damage {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if target_ref.is_tower() {
            return;
        }
        if attack_type == AttackTypeV1::BaseAttack {
            self.cleave(ctx, caster, target);
        }
        self.shockwave(ctx, caster, target);
    }

    fn update(&mut self, _ctx: &mut StableSim<'_>, _rng_seed: u64, _player: usize) {
        self.cooldown = self.cooldown.saturating_sub(1);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::AttackSpeed, ItemTagV1::Hp]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

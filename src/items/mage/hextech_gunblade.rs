use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, refresh_buff, ticks, ItemMeta};

// Lightning Bolt's slow, shared by both tiers.
const SLOW_BUFF: &str = "hextech_gunblade_slow";
// The crackle drawn on the shocked champion: Statikk Shiv's shock
// (`effects/statikk_shock`, 0.4 s, following the unit).
const SHOCK_EFFECT: &str = "riot_statikk_shock";

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
        ctx.play_view_effect(
            SHOCK_EFFECT,
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

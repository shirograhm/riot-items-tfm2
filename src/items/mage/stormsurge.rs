use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, percent_of, ticks, ItemMeta};

/// How far ahead of the damage the bolt is drawn.
///
/// `effects/stormsurge_lightning` is six frames at 0.1s, so at 60 ticks a second
/// each frame is 6 ticks. Three frames of lead puts the damage on the third/
/// fourth frame — where the bolt is fully extended — instead of on frame one,
/// which had the hit landing as the bolt was still only a wisp at the top.
///
/// Retune by frames: this is `frames * 6`, and the frame count and durations are
/// in `effects/stormsurge_lightning#anim.fanim`.
const STRIKE_LEAD_TICKS: usize = 18;

#[derive(Clone, Debug)]
struct Tracked {
    target: usize,
    accumulated: usize,
    window: usize,
    strike: usize,
    /// Whether the bolt for the pending strike has been drawn yet — the strike
    /// countdown alone cannot say, because the tick it fires on is passed
    /// through once and the effect must not repeat on later ticks.
    flashed: bool,
    cooldown: usize,
}

impl Tracked {
    /// Whether this entry still has anything left to do.
    fn live(&self) -> bool {
        self.window > 0 || self.strike > 0 || self.cooldown > 0
    }
}

#[derive(Clone, Debug)]
pub struct Stormsurge {
    meta: ItemMeta,
    strike_effect: &'static str,
    price: usize,
    magic_power: i32,
    move_speed_mult: i32,
    magic_resistance_penetration: usize,
    effect_hp_percent_threshold: f64,
    effect_duration_seconds: f64,
    effect_delay_seconds: f64,
    effect_cooldown_seconds: f64,
    effect_bonus_flat_damage: usize,
    effect_ap_percent_damage: f64,
    // Non-vital stats (internals)
    tracked: Vec<Tracked>,
}

impl Stormsurge {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "stormsurge",
                &["hextech_alternator"],
                &["radiant_stormsurge"],
            ),
            // Our own bolt, bound in `view/effects.view_effects` to
            // `effects/stormsurge_lightning` (tag `strike`).
            //
            // This used to be `lightning_mage_skill2`, which drew nothing: that
            // name is registered as a view *projectile*, and `play_view_effect`
            // only searches the view-effect table. An unknown name is not an
            // error — it silently draws nothing. See the note in `update`.
            strike_effect: "riot_stormsurge_squall",
            price: 1400,
            magic_power: 110,
            move_speed_mult: 5,
            magic_resistance_penetration: 10,
            effect_hp_percent_threshold: 25.0,
            effect_duration_seconds: 2.5,
            effect_delay_seconds: 2.0,
            effect_cooldown_seconds: 30.0,
            effect_bonus_flat_damage: 125,
            effect_ap_percent_damage: 10.0,
            // Non-vital stats (internals)
            tracked: Vec::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_stormsurge", &["stormsurge"]),
            price: 2000,
            magic_power: 200,
            magic_resistance_penetration: 15,
            effect_ap_percent_damage: 15.0,
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
                magic_power,
                move_speed_mult,
                magic_resistance_penetration,
                effect_hp_percent_threshold,
                effect_duration_seconds,
                effect_delay_seconds,
                effect_cooldown_seconds,
                effect_bonus_flat_damage,
                effect_ap_percent_damage
            ]
        );
        self
    }

    /// The entry for `target`, created idle if this is the first hit on it.
    fn entry(&mut self, target: usize) -> &mut Tracked {
        if let Some(index) = self
            .tracked
            .iter()
            .position(|tracked| tracked.target == target)
        {
            return &mut self.tracked[index];
        }
        self.tracked.push(Tracked {
            target,
            accumulated: 0,
            window: 0,
            strike: 0,
            flashed: false,
            cooldown: 0,
        });
        self.tracked
            .last_mut()
            .expect("just pushed an entry for this target")
    }
}

impl Default for Stormsurge {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Stormsurge {
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
            magic_power: self.magic_power,
            move_speed_mult: self.move_speed_mult,
            magic_resistance_penetration: self.magic_resistance_penetration,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.tracked.clear();
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        _caster: usize,
        target: usize,
        damage: &mut usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let dealt = *damage;
        if dealt == 0 {
            return;
        }
        let Some(target_ref) = ctx.get_entity(target) else {
            return;
        };
        if !target_ref.is_champion() {
            return;
        }
        let threshold = percent_of(target_ref.hp().1, self.effect_hp_percent_threshold);

        let window = ticks(self.effect_duration_seconds);
        let delay = ticks(self.effect_delay_seconds);
        let cooldown = ticks(self.effect_cooldown_seconds);

        let tracked = self.entry(target);
        if tracked.cooldown > 0 {
            return;
        }

        if tracked.window == 0 {
            tracked.accumulated = 0;
        }
        tracked.window = window;
        tracked.accumulated = tracked.accumulated.saturating_add(dealt);

        if tracked.accumulated < threshold {
            return;
        }
        tracked.strike = delay;
        tracked.flashed = false;
        tracked.cooldown = cooldown;
        tracked.window = 0;
        tracked.accumulated = 0;
    }

    /// Lands the Squall strikes whose delay has run out.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if self.tracked.is_empty() {
            return;
        }
        let Some(caster) = ctx
            .get_player(player)
            .and_then(|player_ref| player_ref.champion())
            .filter(|champion| champion.is_alive())
            .map(|champion| champion.id())
        else {
            self.tracked.clear();
            return;
        };
        let damage = self.effect_bonus_flat_damage
            + ctx
                .get_entity(caster)
                .map(|caster_ref| {
                    percent_of(caster_ref.stat().magic_power, self.effect_ap_percent_damage)
                })
                .unwrap_or(0);

        let mut flashed = Vec::new();
        let mut struck = Vec::new();
        self.tracked.retain_mut(|tracked| {
            if !ctx
                .get_entity(tracked.target)
                .is_some_and(|target_ref| target_ref.is_alive() && target_ref.is_champion())
            {
                return false;
            }

            tracked.cooldown = tracked.cooldown.saturating_sub(1);
            if tracked.window > 0 {
                tracked.window -= 1;
                if tracked.window == 0 {
                    tracked.accumulated = 0;
                }
            }
            if tracked.strike > 0 {
                tracked.strike -= 1;
                // `<=` rather than `==`, guarded by the flag: a configured
                // `effect_delay_seconds` shorter than the lead never counts down
                // *through* the lead value, and an equality test would silently
                // draw no bolt at all. This way a short delay simply draws it on
                // the first tick it can.
                if !tracked.flashed && tracked.strike <= STRIKE_LEAD_TICKS {
                    tracked.flashed = true;
                    flashed.push(tracked.target);
                }
                if tracked.strike == 0 {
                    struck.push(tracked.target);
                }
            }
            tracked.live()
        });

        // The bolt is drawn `STRIKE_LEAD_TICKS` before the hit, not on it, so
        // the animation is mid-strike when the damage lands rather than only
        // beginning. The damage tick itself is unchanged, so the tooltip's
        // delay still describes when it hurts.
        //
        // It goes through the same frame-event pipeline the game's own
        // `.data_champion` `ViewEffect`s use, so it shows up in replays too, and
        // is skipped silently by the background pre-sims that have no frame
        // recording.
        //
        // `range`/`radius`/`time` are the `.data_champion` `ViewEffect` fields;
        // they steer travelling and area effects, and a bolt anchored on one
        // entity uses none of them, so all three are the documented 0.
        for target in flashed {
            ctx.play_view_effect(
                self.strike_effect,
                caster,
                &InputTargetV1::target(target),
                0,
                0,
                0,
            );
        }
        for target in struck {
            ctx.deal_damage(caster, target, 0, damage, AttackTypeV1::Item);
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        let mut tags = vec![ItemTagV1::Ap, ItemTagV1::MrPenetration];
        if self.move_speed_mult > 0 {
            tags.push(ItemTagV1::MoveSpeed);
        }
        tags
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Magic
    }
}

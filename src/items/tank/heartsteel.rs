use std::collections::HashMap;

use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, item_counts, percent_of, refresh_buff, size_percent, ticks, upgrade_carry,
    Elapsed, ItemMeta, ProcQueue, DISTANCE_UNITS_PER_RANGE, TICKS_PER_SECOND,
};

// Ironheart, League's Colossal Consumption: an enemy champion within
// `effect_max_distance` builds a stage per third of `effect_charge_seconds` in
// range, up to three. At three the carrier's next basic attack on them deals
// bonus damage, banks part of it as permanent health, and starts their
// `effect_cooldown_seconds`; stages and cooldowns are per enemy, linger
// `effect_duration_seconds` each out of range, and clear on either side's
// death.
//
// The range grows with the carrier (`crate::sized_range`), and Goliath grows
// the carrier `effect_size_per_thousand_hp` percent per 1000 maximum health, up
// to `effect_max_size_percent` (`radius_mult`, in whole percents). Each stage
// is a statless `view_buffs` buff drawn from `effects/heartsteel_mark` (ring,
// lit core, pink orb), and the attack bursts it (`effects/heartsteel_trigger`).

// The Ironheart proc sound (`sound/sfx/riot_heartsteel_ironheart.sound_info`,
// mapped by `mod.override_info`): `sfx/lol-heartsteel.mp3` raised 2 dB, limited
// to -1 dBFS, trimmed and faded, since the first cut was too quiet.
const IRONHEART_SFX: &str = "riot_heartsteel_ironheart";
// The picture for each stage, tags `stage1` to `stage3`. The last is the
// charged one: the next basic attack against them procs.
const STAGE_BUFFS: [&str; 3] = [
    "riot_heartsteel_stage1",
    "riot_heartsteel_stage2",
    "riot_heartsteel_stage3",
];
const MAX_STAGES: u8 = 3;
// The stage picture is refreshed once a second, so it never lapses while the
// stage is up, and is gone half a second after a missed refresh: the item left.
const STAGE_BUFF_TICKS: usize = 90;
// The burst where the orb was when the charged attack lands (a `view_effects`
// animation).
const TRIGGER_EFFECT: &str = "riot_heartsteel_trigger";
// Goliath's size on the carrier. One name for both tiers, so the Radiant
// upgrade replaces the base item's instead of adding a second.
const GOLIATH_BUFF: &str = "riot_heartsteel_goliath";
// The upgrade line the banked health is noted under (`crate::upgrade_carry`),
// so it follows the carrier into the Radiant item.
const BASE_KEY: &str = "heartsteel";

#[derive(Clone, Debug)]
pub struct Heartsteel {
    meta: ItemMeta,
    stack_buff: &'static str,
    price: usize,
    hp: i32,
    effect_bonus_flat_damage: usize,
    effect_caster_hp_percent_damage: f64,
    effect_bonus_hp_percent_of_damage: f64,
    effect_max_distance: usize,
    effect_charge_seconds: f64,
    effect_duration_seconds: f64,
    effect_cooldown_seconds: f64,
    effect_size_per_thousand_hp: f64,
    effect_max_size_percent: i32,
    accumulated_bonus_hp: i32,
    // The banked total last noted for an upgrade to take over.
    noted_bonus_hp: i32,
    // Whether this instance has taken over the total of the item it replaced.
    inherited: bool,
    // The size Goliath has on the carrier now, in percent; 0 before the
    // first update of each life.
    goliath_percent: i32,
    // How much bigger than usual the carrier is, in percent, from every source
    // ([`crate::size_percent`]); read once a second and whenever Goliath
    // changes.
    size_percent: u64,
    // Ironheart on each enemy champion, by their player id: the player keeps
    // it across a respawn, their champion entity may not.
    targets: HashMap<usize, Target>,
    procs: ProcQueue,
    // Steps every enemy's cooldown by the time gone by, so they run through
    // the carrier's death.
    clock: Elapsed,
}

impl Heartsteel {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                BASE_KEY,
                &["ring_of_reincarnation"],
                &["radiant_heartsteel"],
            ),
            stack_buff: "heartsteel_stack",
            price: 750,
            hp: 600,
            effect_bonus_flat_damage: 70,
            effect_caster_hp_percent_damage: 6.0,
            effect_bonus_hp_percent_of_damage: 10.0,
            effect_max_distance: 50,
            effect_charge_seconds: 1.5,
            effect_duration_seconds: 1.5,
            effect_cooldown_seconds: 30.0,
            effect_size_per_thousand_hp: 3.0,
            effect_max_size_percent: 30,
            // Non-vital stats (internals)
            accumulated_bonus_hp: 0,
            noted_bonus_hp: 0,
            inherited: false,
            goliath_percent: 0,
            size_percent: 0,
            targets: HashMap::new(),
            procs: ProcQueue::new(),
            clock: Elapsed::default(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_heartsteel", &[BASE_KEY]),
            stack_buff: "heartsteel_stack",
            price: 1050,
            hp: 1000,
            effect_bonus_flat_damage: 70,
            effect_caster_hp_percent_damage: 6.0,
            effect_bonus_hp_percent_of_damage: 10.0,
            effect_max_distance: 50,
            effect_charge_seconds: 1.5,
            effect_duration_seconds: 1.5,
            effect_cooldown_seconds: 30.0,
            effect_size_per_thousand_hp: 3.0,
            effect_max_size_percent: 30,
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
                effect_bonus_flat_damage,
                effect_caster_hp_percent_damage,
                effect_bonus_hp_percent_of_damage,
                effect_max_distance,
                effect_charge_seconds,
                effect_duration_seconds,
                effect_cooldown_seconds,
                effect_size_per_thousand_hp,
                effect_max_size_percent
            ]
        );
        self
    }

    // Goliath: sizes the carrier to their maximum health. The buff is only
    // replaced when the whole percent changes, not every tick; returns
    // whether it was.
    fn goliath(&mut self, ctx: &mut StableSim<'_>, carrier: usize) -> bool {
        let Some(max_hp) = ctx.get_entity(carrier).map(|c| c.hp().1) else {
            return false;
        };
        let percent = ((max_hp as f64 * self.effect_size_per_thousand_hp / 1000.0) as i32)
            .min(self.effect_max_size_percent)
            .max(0);
        if percent == self.goliath_percent {
            return false;
        }
        self.goliath_percent = percent;
        ctx.entity_remove_buff(carrier, GOLIATH_BUFF);
        if percent > 0 {
            ctx.add_buff(
                carrier,
                &BuffV1 {
                    radius_mult: percent,
                    ..BuffV1::named(GOLIATH_BUFF)
                },
            );
        }
        true
    }

    // Keeps the banked total where an upgrade can find it: the Radiant instance
    // takes it over once, and every instance notes it as it grows. The health
    // banked this life is already on the champion as `heartsteel_stack` buffs,
    // re-applied on respawn.
    fn carry_bonus_hp(&mut self, ctx: &StableSim<'_>, player: usize) {
        if !std::mem::replace(&mut self.inherited, true) && self.meta.upgrades_from(BASE_KEY) {
            if let Some((_, total)) = upgrade_carry::latest(BASE_KEY, ctx, player) {
                self.accumulated_bonus_hp = self.accumulated_bonus_hp.max(total as i32);
                self.noted_bonus_hp = self.accumulated_bonus_hp;
            }
        }
        if self.accumulated_bonus_hp != self.noted_bonus_hp {
            self.noted_bonus_hp = self.accumulated_bonus_hp;
            upgrade_carry::note(BASE_KEY, ctx, player, self.accumulated_bonus_hp.max(0) as u64);
        }
    }

    // Clears every enemy's stages: the carrier respawned. The cooldowns keep
    // running.
    fn clear_stages(&mut self, ctx: &mut StableSim<'_>) {
        for target in self.targets.values_mut() {
            set_stage(ctx, target, 0);
            target.progress = 0;
            target.idle = 0;
        }
    }
}

impl Default for Heartsteel {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for Heartsteel {
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
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        self.carry_bonus_hp(ctx, player);
        self.procs.clear();
        self.clear_stages(ctx);

        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        let champion_id = champion_ref.id();
        // Same-name buffs stack and this carries the whole banked total, so the
        // previous life's copies are removed first, or every respawn would add
        // another total past the cap.
        ctx.entity_remove_buff(champion_id, self.stack_buff);
        ctx.add_buff(
            champion_id,
            &BuffV1 {
                hp: self.accumulated_bonus_hp,
                ..BuffV1::named(self.stack_buff)
            },
        );
        // Goliath is sized afresh each life, on the next update.
        ctx.entity_remove_buff(champion_id, GOLIATH_BUFF);
        self.goliath_percent = 0;
    }

    // Spends the charge: a basic attack against an enemy champion at the last
    // stage.
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
        let Some(max_hp) = ctx.get_entity(caster).map(|c| c.hp().1) else {
            return;
        };
        let cooldown = ticks(self.effect_cooldown_seconds);
        let Some(charged) = self
            .targets
            .values_mut()
            .find(|t| t.stages == MAX_STAGES && t.entity == target)
        else {
            return;
        };
        set_stage(ctx, charged, 0);
        charged.progress = 0;
        charged.idle = 0;
        charged.cooldown = cooldown;

        let bonus_damage = self.effect_bonus_flat_damage
            + percent_of(max_hp, self.effect_caster_hp_percent_damage);
        let bonus_hp = percent_of(bonus_damage, self.effect_bonus_hp_percent_of_damage) as i32;
        // The banked health is priced off the damage this swing earned and is
        // granted with the swing.
        self.procs
            .on_hit_physical(ctx, target, damage, damage_type, is_crit, bonus_damage);
        ctx.play_sfx(IRONHEART_SFX, caster, &InputTargetV1::target(target));
        ctx.play_view_effect(
            TRIGGER_EFFECT,
            caster,
            &InputTargetV1::target(target),
            0,
            0,
            0,
        );
        ctx.add_buff(
            caster,
            &BuffV1 {
                hp: bonus_hp,
                ..BuffV1::named(self.stack_buff)
            },
        );
        self.accumulated_bonus_hp += bonus_hp;
    }

    // Lands the Ironheart damage whose delay has run out, keeps Goliath's
    // size in step with the carrier's health, then builds, drops and cools
    // down every enemy champion's stages.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        // One tick, or the whole of a death on the first update after it: no
        // `update` runs for a dead carrier, and the cooldowns are to keep running.
        let gone = self.clock.since_last(ctx);
        self.carry_bonus_hp(ctx, player);
        // The health banked, for the number on the item's icon.
        let banked = self.accumulated_bonus_hp.max(0) as u64;
        item_counts::note(ctx, player, self.meta.key, banked);
        self.procs.update(ctx, player);

        let Some(team) = ctx.get_player(player).map(|p| p.team()) else {
            return;
        };
        let carrier = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| c.id());
        let refresh = ctx.tick() % TICKS_PER_SECOND as usize == 0;
        if let Some(carrier_id) = carrier {
            let resized = self.goliath(ctx, carrier_id);
            if resized || refresh {
                self.size_percent = size_percent(ctx, carrier_id);
            }
        }
        // (player id, champion entity, alive) for every enemy champion.
        let mut enemies = Vec::new();
        for index in 0..ctx.player_count() {
            let Some(enemy) = ctx.player_at(index) else {
                continue;
            };
            if enemy.team() == team {
                continue;
            }
            if let Some(champion) = enemy.champion() {
                enemies.push((enemy.id(), champion.id(), champion.is_alive()));
            }
        }

        // Ironheart reaches as much further as the carrier is bigger.
        let reach = (self.effect_max_distance * DISTANCE_UNITS_PER_RANGE) as u64
            * (100 + self.size_percent)
            / 100;
        let stage_ticks = (ticks(self.effect_charge_seconds) / MAX_STAGES as usize).max(1);
        let linger_ticks = ticks(self.effect_duration_seconds).max(1);
        for (id, entity, alive) in enemies {
            let target = self.targets.entry(id).or_default();
            target.entity = entity;
            target.cooldown = target.cooldown.saturating_sub(gone);
            // Death on either side clears every stage.
            let Some(carrier_id) = carrier.filter(|_| alive) else {
                set_stage(ctx, target, 0);
                target.progress = 0;
                target.idle = 0;
                continue;
            };
            let in_range = ctx.distance_sq(carrier_id, entity) <= reach * reach;
            if in_range && target.cooldown == 0 {
                // Building: a stage for every `stage_ticks` in range.
                target.idle = 0;
                if target.stages < MAX_STAGES {
                    target.progress += 1;
                    if target.progress >= stage_ticks {
                        target.progress = 0;
                        let next = target.stages + 1;
                        set_stage(ctx, target, next);
                    }
                }
            } else {
                // Out of range: part of a stage is lost at once, a whole one
                // lingers `linger_ticks` and then drops, one at a time.
                target.progress = 0;
                if target.stages > 0 {
                    target.idle += 1;
                    if target.idle >= linger_ticks {
                        target.idle = 0;
                        let next = target.stages - 1;
                        set_stage(ctx, target, next);
                    }
                }
            }
            if refresh && target.stages > 0 {
                let name = STAGE_BUFFS[target.stages as usize - 1];
                refresh_buff(ctx, entity, name, &BuffV1::timed(name, STAGE_BUFF_TICKS));
            }
        }
    }

    // Banked HP is permanent, so it crosses the Radiant upgrade; only the
    // counter moves (see `carry_bonus_hp`), and the `i32` cast round-trips
    // exactly. The host (0.6.2) never calls these two (`crate::upgrade_carry`).
    fn on_upgrade(&mut self, next_key: &str) -> u64 {
        if self.meta.upgrades_to(next_key) {
            self.accumulated_bonus_hp as u64
        } else {
            0
        }
    }

    fn on_upgraded_from(&mut self, prev_key: &str, carry: u64) {
        if self.meta.upgrades_from(prev_key) {
            self.accumulated_bonus_hp = carry as i32;
        }
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Hp, ItemTagV1::MyHpPercentDamage]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Hp
    }
}

// Ironheart's hold on one enemy champion.
#[derive(Clone, Copy, Debug, Default)]
struct Target {
    // Their champion entity, as of the last update.
    entity: usize,
    // 0 to `MAX_STAGES`; at the last one the carrier's next basic attack
    // against them procs.
    stages: u8,
    // Ticks in range toward the next stage.
    progress: usize,
    // Ticks out of range since a stage was last gained or dropped; the top
    // one drops at `effect_duration_seconds`.
    idle: usize,
    // Ticks left before Ironheart can build stages on them again.
    cooldown: usize,
}

// Moves `target` to `stage`, swapping the picture over them: the old stage's
// comes off and the new one's goes on. Stage 0 has none.
fn set_stage(ctx: &mut StableSim<'_>, target: &mut Target, stage: u8) {
    if target.stages == stage {
        return;
    }
    if target.stages > 0 {
        ctx.entity_remove_buff(target.entity, STAGE_BUFFS[target.stages as usize - 1]);
    }
    target.stages = stage;
    if stage > 0 {
        let name = STAGE_BUFFS[stage as usize - 1];
        refresh_buff(
            ctx,
            target.entity,
            name,
            &BuffV1::timed(name, STAGE_BUFF_TICKS),
        );
    }
}

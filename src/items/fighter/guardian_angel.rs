use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, percent_of, ticks, upgrade_carry, ItemMeta};

// Rebirth: Upon taking lethal damage, instead resurrect for 4 seconds, healing for
// 40% of your maximum health. During the duration, you are untargetable,
// invulnerable, and unable to act (300 second cooldown).
//
// The heal lands in equal pulses every `effect_heal_interval_seconds` across the
// stasis; `effect_caster_hp_percent_heal` is the total.
//
// There is no "would die" hook, and `on_damaged` runs after the engine has already
// marked the carrier dead: setting HP back from there revives her in the sim, but
// the match view has played the death by then. So the carrier never reaches 0 HP
// while Rebirth is ready: she holds an `undying` buff, and the hit that pins her
// at the HP floor is the "lethal" one.

/// Held while Rebirth is ready. Shared by both tiers, so an upgrade keeps it.
const UNDYING_BUFF: &str = "guardian_angel_undying";
/// Invulnerability for the stasis (and the tick after it, for the last heal
/// pulse): the banish stops targeting, this stops damage-over-time.
const STASIS_BUFF: &str = "guardian_angel_stasis";
/// Wisps for the length of the stasis, then a flash as she gets up. Both are
/// bound in `view/effects.view_effects`; the stasis sheet is drawn 4 seconds long.
const STASIS_EFFECT: &str = "riot_guardian_angel_stasis";
const REVIVE_EFFECT: &str = "riot_guardian_angel_revive";
/// `has_buff` misses a new buff for ~3 ticks; don't re-add inside this window.
const REAPPLY_GUARD_TICKS: usize = 10;
/// The upgrade line Rebirths are noted under (`crate::upgrade_carry`): the
/// cooldown has to follow the carrier into the Radiant item, which otherwise
/// starts ready. Seen in game on 2026-10-02: Rebirth fired again 78 seconds
/// after the base item's.
const BASE_KEY: &str = "guardian_angel";

#[derive(Clone, Debug)]
pub struct GuardianAngel {
    meta: ItemMeta,
    price: usize,
    attack: i32,
    defence: i32,
    effect_caster_hp_percent_heal: f64,
    effect_duration_seconds: f64,
    effect_heal_interval_seconds: f64,
    effect_cooldown_seconds: f64,
    /// Stasis in progress: (carrier entity, next heal pulse tick, last pulse tick).
    stasis: Option<(usize, usize, usize)>,
    /// Sim tick at which Rebirth is available again; 0 = ready.
    ready_at_tick: usize,
    /// Tick the undying buff was last added, for `REAPPLY_GUARD_TICKS`.
    undying_added_tick: Option<usize>,
    /// Whether this instance has looked up the Rebirth of the item it replaced.
    inherited: bool,
}

impl GuardianAngel {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                BASE_KEY,
                &["bf_sword", "steel_sigil"],
                &["radiant_guardian_angel"],
            ),
            price: 750,
            attack: 35,
            defence: 30,
            effect_caster_hp_percent_heal: 40.0,
            effect_duration_seconds: 4.0,
            effect_heal_interval_seconds: 0.25,
            effect_cooldown_seconds: 300.0,
            stasis: None,
            ready_at_tick: 0,
            undying_added_tick: None,
            inherited: false,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_guardian_angel", &[BASE_KEY]),
            price: 1100,
            attack: 50,
            defence: 45,
            effect_caster_hp_percent_heal: 60.0,
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
                defence,
                effect_caster_hp_percent_heal,
                effect_duration_seconds,
                effect_heal_interval_seconds,
                effect_cooldown_seconds
            ]
        );
        self
    }

    fn is_ready(&mut self, tick: usize) -> bool {
        // A deadline further out than one full cooldown is left over from an earlier
        // match (the sim tick restarts at 0), not a live cooldown.
        if self.ready_at_tick > tick + ticks(self.effect_cooldown_seconds) {
            self.ready_at_tick = 0;
            self.undying_added_tick = None;
            self.stasis = None;
        }
        tick >= self.ready_at_tick
    }

    fn interval_ticks(&self) -> usize {
        ticks(self.effect_heal_interval_seconds).max(1)
    }

    /// Share of the total heal that each pulse restores.
    fn pulse_percent(&self) -> f64 {
        let pulses = (ticks(self.effect_duration_seconds) / self.interval_ticks()).max(1);
        self.effect_caster_hp_percent_heal / pulses as f64
    }

    /// Restores HP for every pulse that has come due. Set directly rather than
    /// through `heal`, so healing reduction can't shorten the revive.
    fn pulse_heal(&mut self, ctx: &mut StableSim<'_>, tick: usize) {
        let Some((entity, mut next, last)) = self.stasis else {
            return;
        };
        let interval = self.interval_ticks();
        let mut pulses = 0;
        while next <= tick && next <= last {
            pulses += 1;
            next += interval;
        }
        self.stasis = (next <= last).then_some((entity, next, last));
        if pulses == 0 {
            return;
        }
        let Some((hp, max_hp)) = ctx
            .get_entity(entity)
            .filter(|e| e.is_alive())
            .map(|e| e.hp())
        else {
            self.stasis = None;
            return;
        };
        let heal = percent_of(max_hp, self.pulse_percent()) * pulses;
        ctx.entity_set_hp(entity, (hp + heal).min(max_hp));
    }

    /// A Radiant instance takes over the cooldown of the Rebirth its base item
    /// last fired, once, before it first decides anything.
    fn inherit_cooldown(&mut self, ctx: &StableSim<'_>, player: usize) {
        if std::mem::replace(&mut self.inherited, true) || !self.meta.upgrades_from(BASE_KEY) {
            return;
        }
        if let Some((fired, _)) = upgrade_carry::latest(BASE_KEY, ctx, player) {
            let ready_at = fired + ticks(self.effect_cooldown_seconds);
            self.ready_at_tick = self.ready_at_tick.max(ready_at);
        }
    }

    /// Rebirth itself: she stays at the HP floor and heals back up in pulses
    /// across the stasis.
    fn rebirth(&mut self, ctx: &mut StableSim<'_>, player: usize, entity: usize, tick: usize) {
        upgrade_carry::note(BASE_KEY, ctx, player, 0);
        let stasis = ticks(self.effect_duration_seconds);
        ctx.entity_remove_buff(entity, UNDYING_BUFF);
        ctx.add_buff(
            entity,
            &BuffV1 {
                undying: true,
                damaged_reduce: 100,
                ..BuffV1::timed(STASIS_BUFF, stasis + 1)
            },
        );
        ctx.entity_set_hp(entity, 1);
        ctx.entity_clear_cc(entity);
        ctx.entity_banish(entity, entity, stasis, STASIS_EFFECT, REVIVE_EFFECT);
        self.stasis = Some((entity, tick + self.interval_ticks(), tick + stasis));
        self.ready_at_tick = tick + ticks(self.effect_cooldown_seconds);
        self.undying_added_tick = None;
    }

    /// Settles a carrier the undying buff has pinned (`is_pinned`). While Rebirth
    /// is ready that is its trigger. With Rebirth spent the buff should not be on
    /// her at all, and if it ever is, it would keep her unkillable at the HP floor
    /// for the rest of the cooldown: she dies to that hit after all, there and
    /// then rather than on whichever hit follows the buff's removal.
    ///
    /// Raw damage, because the hit that pinned her has already been through the
    /// attack pipeline once; a second pass would count it, and its procs, again.
    fn settle_pinned(
        &mut self,
        ctx: &mut StableSim<'_>,
        player: usize,
        entity: usize,
        attacker: usize,
    ) {
        self.inherit_cooldown(ctx, player);
        let tick = ctx.tick();
        if self.is_ready(tick) {
            self.rebirth(ctx, player, entity, tick);
            return;
        }
        let Some(carrier) = ctx.get_entity(entity) else {
            return;
        };
        // A stasis in progress is Rebirth at work: she sits at the floor until the
        // first heal pulse, and `has_buff` may still report the buff just removed.
        if self.stasis.is_some() || has_buff(&carrier, STASIS_BUFF) {
            return;
        }
        let lethal = carrier.hp().0.max(1) + carrier.shield();
        ctx.entity_remove_buff(entity, UNDYING_BUFF);
        ctx.deal_damage_raw(attacker, entity, lethal, 0, AttackTypeV1::Item);
    }
}

/// Alive at the HP floor with the undying buff up: the buff has caught a hit that
/// would have killed the carrier.
fn is_pinned(entity: &StableEntity<'_, '_>) -> bool {
    entity.is_alive() && entity.hp().0 <= 1 && has_buff(entity, UNDYING_BUFF)
}

impl Default for GuardianAngel {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for GuardianAngel {
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
            defence: self.defence,
            ..Default::default()
        }
    }

    // Keeps the undying buff on the carrier whenever Rebirth is ready. Buffs are
    // lost on death, so this also restores it after a respawn.
    //
    // A pinned carrier is settled here as well as in `on_damaged`, so she is never
    // left standing at the HP floor: that hook can miss the hit that pinned her,
    // while `has_buff` does not see a new buff yet.
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        let tick = ctx.tick();
        self.inherit_cooldown(ctx, player);
        self.pulse_heal(ctx, tick);
        let Some(champion) = ctx.get_player(player).and_then(|p| p.champion()) else {
            return;
        };
        if !champion.is_alive() {
            return;
        }
        let entity = champion.id();
        if is_pinned(&champion) {
            self.settle_pinned(ctx, player, entity, entity);
            return;
        }
        if !self.is_ready(tick) {
            return;
        }
        if self
            .undying_added_tick
            .is_some_and(|added| tick < added + REAPPLY_GUARD_TICKS)
        {
            return;
        }
        if has_buff(&champion, UNDYING_BUFF) {
            return;
        }
        ctx.add_buff(
            entity,
            &BuffV1 {
                undying: true,
                ..BuffV1::named(UNDYING_BUFF)
            },
        );
        self.undying_added_tick = Some(tick);
    }

    // The engine lowers HP before this runs. With the undying buff up, a hit that
    // would have killed the carrier leaves her at the HP floor instead: that is
    // Rebirth's trigger.
    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        player: usize,
        entity: usize,
        attacker: usize,
        _damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        // Only a hit the undying buff caught: without it she is already dead, and
        // bringing her back from there leaves the death (and `on_dead`) standing.
        if ctx.get_entity(entity).is_some_and(|e| is_pinned(&e)) {
            self.settle_pinned(ctx, player, entity, attacker);
        }
    }

    // The cooldown carries into Radiant Guardian Angel. The host of game 0.6.2
    // never calls these two (`crate::upgrade_carry`); `inherit_cooldown` does the
    // carrying, and they stay for a host that does.
    fn on_upgrade(&mut self, _next_key: &str) -> u64 {
        self.ready_at_tick as u64
    }

    fn on_upgraded_from(&mut self, _prev_key: &str, carry: u64) {
        self.ready_at_tick = carry as usize;
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::Defense]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

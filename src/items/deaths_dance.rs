use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{
    apply_config, count_takedowns, has_buff, mark_enemy_champion, percent_of, ticks, ItemMeta,
};

/// How many times a second Ignore Pain bleeds.
///
/// One number drives two things that must agree: the gate buff lasts
/// `1 / BURN_PROCS_PER_SECOND` seconds, and each proc is allowed
/// `effect_burn_hp_percent_cap / BURN_PROCS_PER_SECOND` of max HP. Splitting
/// them into separate literals is how the cap silently stopped meaning "per
/// second" — `update` runs every tick, so the gate is the only thing setting
/// the rate.
const BURN_PROCS_PER_SECOND: f64 = 5.0;

#[derive(Clone, Debug)]
pub struct DeathsDance {
    meta: ItemMeta,
    // Buff names are namespaced per variant so the base and radiant
    // items keep independent stacks.
    burn_buff: &'static str,
    price: usize,
    attack: i32,
    defence: i32,
    skill_cooldown_mult: i32,
    effect_delayed_damage_percent: f64,
    effect_burn_hp_percent_cap: f64,
    effect_bonus_flat_heal: i32,
    effect_kill_heal_missing_percent: f64,
    accumulated_damage: i32,
    /// Bleed damage dealt but not yet seen come back through `on_damaged`, so
    /// the pool does not refill itself. See [`DeathsDance::on_damaged`].
    self_inflicted_credit: i32,
    last_damaged_by: usize,
    takedown_marks: Vec<(usize, usize)>,
}

impl DeathsDance {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base("deaths_dance", &["steel_sigil"], &["radiant_deaths_dance"]),
            burn_buff: "deaths_dance_burn",
            price: 1450,
            attack: 45,
            defence: 45,
            skill_cooldown_mult: 10,
            effect_delayed_damage_percent: 25.0,
            effect_burn_hp_percent_cap: 5.0,
            effect_bonus_flat_heal: 45,
            effect_kill_heal_missing_percent: 15.0,
            accumulated_damage: 0,
            self_inflicted_credit: 0,
            last_damaged_by: 0,
            takedown_marks: Vec::new(),
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_deaths_dance", &["deaths_dance"]),
            burn_buff: "radiant_deaths_dance_burn",
            price: 2100,
            attack: 75,
            defence: 75,
            effect_bonus_flat_heal: 75,
            effect_kill_heal_missing_percent: 25.0,
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
                skill_cooldown_mult,
                effect_delayed_damage_percent,
                effect_burn_hp_percent_cap,
                effect_bonus_flat_heal,
                effect_kill_heal_missing_percent
            ]
        );
        self
    }

    /// Ratio between damage this champion *takes* and damage that was *aimed*
    /// at it.
    ///
    /// `stat()` puts `effect_delayed_damage_percent` into `damaged_reduce`, so
    /// the game shaves that much off every incoming hit before `on_damaged`
    /// reports it — and off this item's own bleed on the way back out. So it is
    /// needed twice: to recover the pre-reduction figure when storing, and to
    /// pre-pay the reduction when bleeding, so the intended amount lands. At the
    /// default 25% this is 4/3, which is where the bare `4.0 / 3.0` in the
    /// original came from.
    fn mitigation_scale(&self) -> f64 {
        // Reducing 100% of damage would divide by zero, and there is no sensible
        // bleed for a champion that takes nothing, so leave a sliver through.
        100.0 / (100.0 - self.effect_delayed_damage_percent).max(1.0)
    }

    fn defy(&mut self, ctx: &mut StableSim<'_>, player: usize, takedowns: usize) {
        if takedowns == 0 {
            return;
        }
        self.accumulated_damage = 0;
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };
        let hp_max = champion_ref.hp().1;
        let hp_current = champion_ref.hp().0;
        let champion_id = champion_ref.id();
        let missing_hp = hp_max.saturating_sub(hp_current);
        let heal = self.effect_bonus_flat_heal as usize
            + percent_of(missing_hp, self.effect_kill_heal_missing_percent);
        if heal == 0 {
            return;
        }
        for _ in 0..takedowns {
            ctx.heal(champion_id, champion_id, heal);
        }
    }
}

impl Default for DeathsDance {
    fn default() -> Self {
        Self::base()
    }
}

impl StableItem for DeathsDance {
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
            skill_cooldown_mult: self.skill_cooldown_mult,
            damaged_reduce: self.effect_delayed_damage_percent as usize,
            ..Default::default()
        }
    }

    fn on_spawn(&mut self, _ctx: &mut StableSim<'_>, _player: usize) {
        self.accumulated_damage = 0;
        self.self_inflicted_credit = 0;
        self.last_damaged_by = 0;
        self.takedown_marks.clear();
    }

    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        // Defy: heal + cleanse on champion takedowns.
        let takedowns = count_takedowns(&mut self.takedown_marks, ctx);
        self.defy(ctx, player, takedowns);

        // Ignore Pain: bleed the stored damage back over time.
        if self.accumulated_damage <= 0 {
            return;
        }
        let Some(player_ref) = ctx.get_player(player) else {
            return;
        };
        let Some(champion_ref) = player_ref.champion() else {
            return;
        };

        let is_burn_applied = has_buff(&champion_ref, self.burn_buff);
        if is_burn_applied {
            return;
        }

        let entity = champion_ref.id();
        // `update` runs every tick, so the gate buff below is what makes this a
        // rate rather than a per-frame drain: the cap is a per-second figure
        // split across that many procs.
        let per_proc_cap = percent_of(
            champion_ref.hp().1,
            self.effect_burn_hp_percent_cap / BURN_PROCS_PER_SECOND,
        ) as i32;

        let tick_damage = self.accumulated_damage.min(per_proc_cap);
        if tick_damage <= 0 {
            return;
        }
        ctx.add_buff(
            entity,
            &BuffV1::timed(self.burn_buff, ticks(1.0 / BURN_PROCS_PER_SECOND)),
        );
        // Grossed up so that `tick_damage` is what actually lands after this
        // item's own `damaged_reduce` takes its cut. Dealing `tick_damage` flat
        // is what made the real ceiling 3.75% of max HP a second instead of the
        // 5% the config asks for.
        let dealt = (tick_damage as f64 * self.mitigation_scale()).round() as usize;
        // What lands is reported straight back to `on_damaged`; bank it so that
        // it is discounted there rather than refilling the pool.
        self.self_inflicted_credit += tick_damage;
        ctx.deal_damage(self.last_damaged_by, entity, dealt, 0, AttackTypeV1::Item);
        self.accumulated_damage -= tick_damage;
    }

    fn on_damaged(
        &mut self,
        _ctx: &mut StableSim<'_>,
        _player: usize,
        entity: usize,
        attacker: usize,
        damage: usize,
    ) {
        if attacker == entity {
            return;
        }

        // Ignore Pain's own bleed arrives here like any other hit: it is dealt
        // with `last_damaged_by` as the attacker, not the champion itself, so
        // the guard above never catches it. Left alone it re-stores a share of
        // itself and the pool feeds off its own drain.
        //
        // Discounted against a banked credit rather than a "currently bleeding"
        // flag, so it holds whether the engine reports this back inside
        // `deal_damage` or a tick later.
        let mut damage = damage as i32;
        if self.self_inflicted_credit > 0 {
            let discounted = damage.min(self.self_inflicted_credit);
            self.self_inflicted_credit -= discounted;
            damage -= discounted;
        }
        if damage <= 0 {
            return;
        }

        self.accumulated_damage += percent_of(
            (damage as f64 * self.mitigation_scale()).round() as usize,
            self.effect_delayed_damage_percent,
        ) as i32;

        self.last_damaged_by = attacker
    }

    fn on_attack(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        _damage: &mut usize,
        _damage_type: DamageTypeV1,
    ) {
        mark_enemy_champion(&mut self.takedown_marks, ctx, caster, target);
    }

    fn on_skill_hit(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, caster: usize, target: usize) {
        mark_enemy_champion(&mut self.takedown_marks, ctx, caster, target);
    }

    fn tags(&self) -> Vec<ItemTagV1> {
        vec![ItemTagV1::Ad, ItemTagV1::Defense, ItemTagV1::CooltimeReduce]
    }

    fn category(&self) -> ItemCategoryV1 {
        ItemCategoryV1::Ad
    }
}

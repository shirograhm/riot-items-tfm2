use mod_api_stable::*;

use crate::config::ItemConfig;
use crate::{apply_config, has_buff, percent_of, ticks, ItemMeta};

// Rebirth: Upon taking lethal damage, instead resurrect for 4 seconds, healing for
// 40% of your maximum health. During the duration, you are untargetable,
// invulnerable, and unable to act (150 second cooldown).
//
// The heal lands in equal pulses every `effect_heal_interval_seconds` across the
// stasis; `effect_caster_hp_percent_heal` is the total.
//
// There is no "would die" hook, and `on_damaged` runs after the engine has already
// marked the carrier dead: setting HP back from there revives her in the sim, but
// the match view has played the death by then. So the carrier never reaches 0 HP
// while Rebirth is ready: she holds an `undying` buff, and the hit that pins her
// at the HP floor is the "lethal" one.
//
// PROBE BUILD: `DIAGNOSTIC` logs the carrier's state around Rebirth to
// `<mod dir>/logs/guardian_angel_probe.log`; turn it off once confirmed in game.
const DIAGNOSTIC: bool = true;
const WATCH_TICKS: usize = 600;
const WATCH_EVERY: usize = 10;

/// Held while Rebirth is ready. Shared by both tiers, so an upgrade keeps it.
const UNDYING_BUFF: &str = "guardian_angel_undying";
/// Invulnerability for the stasis (and the tick after it, for the last heal
/// pulse): the banish stops targeting, this stops damage-over-time.
const STASIS_BUFF: &str = "guardian_angel_stasis";
/// `has_buff` misses a new buff for ~3 ticks; don't re-add inside this window.
const REAPPLY_GUARD_TICKS: usize = 10;

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
    /// Diagnostic: log the carrier's state every few ticks until this tick.
    watch_until_tick: usize,
    /// Diagnostic: last seen player state, to log every death and respawn.
    was_alive: bool,
    last_deaths: usize,
}

impl GuardianAngel {
    pub fn base() -> Self {
        Self {
            meta: ItemMeta::base(
                "guardian_angel",
                &["bf_sword", "steel_sigil"],
                &["radiant_guardian_angel"],
            ),
            price: 750,
            attack: 35,
            defence: 30,
            effect_caster_hp_percent_heal: 40.0,
            effect_duration_seconds: 4.0,
            effect_heal_interval_seconds: 0.25,
            effect_cooldown_seconds: 150.0,
            stasis: None,
            ready_at_tick: 0,
            undying_added_tick: None,
            watch_until_tick: 0,
            was_alive: true,
            last_deaths: 0,
        }
    }

    pub fn radiant() -> Self {
        Self {
            meta: ItemMeta::radiant("radiant_guardian_angel", &["guardian_angel"]),
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
        self.log(ctx, entity, &format!("heal pulse +{heal}"));
    }

    fn log(&self, ctx: &StableSim<'_>, entity: usize, event: &str) {
        if !DIAGNOSTIC {
            return;
        }
        let state = match ctx.get_entity(entity) {
            Some(e) => {
                let (hp, max) = e.hp();
                let (x, y) = e.pos();
                format!(
                    "{} hp={hp}/{max} alive={} undying={} stasis={} pos=({x},{y})",
                    e.name().unwrap_or_default(),
                    e.is_alive(),
                    has_buff(&e, UNDYING_BUFF),
                    has_buff(&e, STASIS_BUFF)
                )
            }
            None => "entity=None".to_string(),
        };
        let kills = ctx.kill_log_count();
        let last_kill = kills
            .checked_sub(1)
            .and_then(|i| ctx.kill_log_at(i))
            .map(|k| {
                format!(
                    "last_kill(tick={} killer_team={} victim_lane={})",
                    k.tick, k.killer_team, k.killed_position
                )
            })
            .unwrap_or_default();
        let line = format!(
            "[{}] tick={} entity={entity} {event} | {state} | kill_log={kills} {last_kill}",
            self.meta.key,
            ctx.tick()
        );
        let dir = crate::config::mod_dir().join("logs");
        let _ = std::fs::create_dir_all(&dir);
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("guardian_angel_probe.log"))
        {
            let _ = writeln!(f, "{line}");
        }
    }

    fn player_state(ctx: &StableSim<'_>, player: usize) -> String {
        match ctx.get_player(player) {
            Some(p) => format!(
                "player={player} champ={:?} player_alive={} respawn_time={} deaths={}",
                p.champion().map(|c| c.id()),
                p.is_alive(),
                p.respawn_time(),
                p.deaths()
            ),
            None => "player=None".to_string(),
        }
    }

    fn log_diagnostics(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        // Catch every death of the carrier, including ones Rebirth never saw.
        let (alive, deaths, champ) = match ctx.get_player(player) {
            Some(p) => (p.is_alive(), p.deaths(), p.champion().map(|c| c.id())),
            None => return,
        };
        let entity = champ.unwrap_or(usize::MAX);
        if alive != self.was_alive || deaths != self.last_deaths {
            self.log(
                ctx,
                entity,
                &format!(
                    "STATE CHANGE alive {}->{alive} deaths {}->{deaths} | {}",
                    self.was_alive,
                    self.last_deaths,
                    Self::player_state(ctx, player)
                ),
            );
            self.was_alive = alive;
            self.last_deaths = deaths;
        }
        let tick = ctx.tick();
        if tick >= self.watch_until_tick {
            return;
        }
        let remaining = self.watch_until_tick - tick;
        if remaining % WATCH_EVERY == 0 {
            self.log(
                ctx,
                entity,
                &format!("watch -{remaining} | {}", Self::player_state(ctx, player)),
            );
        }
    }
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
    fn update(&mut self, ctx: &mut StableSim<'_>, _rng_seed: u64, player: usize) {
        if DIAGNOSTIC {
            self.log_diagnostics(ctx, player);
        }

        let tick = ctx.tick();
        self.pulse_heal(ctx, tick);
        if !self.is_ready(tick) {
            return;
        }
        if self
            .undying_added_tick
            .is_some_and(|added| tick < added + REAPPLY_GUARD_TICKS)
        {
            return;
        }
        let Some(champion) = ctx.get_player(player).and_then(|p| p.champion()) else {
            return;
        };
        if !champion.is_alive() || has_buff(&champion, UNDYING_BUFF) {
            return;
        }
        let entity = champion.id();
        ctx.add_buff(
            entity,
            &BuffV1 {
                undying: true,
                ..BuffV1::named(UNDYING_BUFF)
            },
        );
        self.undying_added_tick = Some(tick);
        self.log(ctx, entity, "undying buff added");
    }

    // The engine lowers HP before this runs. With the undying buff up, a hit that
    // would have killed the carrier leaves her at the HP floor instead: that is
    // Rebirth's trigger.
    fn on_damaged(
        &mut self,
        ctx: &mut StableSim<'_>,
        player: usize,
        entity: usize,
        _attacker: usize,
        damage: usize,
        _damage_type: DamageTypeV1,
        _attack_type: AttackTypeV1,
        _is_crit: bool,
    ) {
        let Some(entity_ref) = ctx.get_entity(entity) else {
            return;
        };
        let (current_hp, _) = entity_ref.hp();
        let alive = entity_ref.is_alive();
        let guarded = has_buff(&entity_ref, UNDYING_BUFF);
        let tick = ctx.tick();

        if current_hp > 1 && alive {
            if DIAGNOSTIC && tick < self.watch_until_tick {
                self.log(ctx, entity, &format!("on_damaged during watch dmg={damage}"));
            }
            return;
        }

        self.log(
            ctx,
            entity,
            &format!(
                "FLOOR on_damaged dmg={damage} guarded={guarded} ready_at={} | {}",
                self.ready_at_tick,
                Self::player_state(ctx, player)
            ),
        );
        if !alive || !guarded || !self.is_ready(tick) {
            return;
        }

        // She stays at the HP floor and heals back up in pulses across the stasis.
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
        ctx.entity_set_hp(entity, current_hp.max(1));
        ctx.entity_clear_cc(entity);
        let banished = ctx.entity_banish(entity, entity, stasis, "", "");
        self.stasis = Some((entity, tick + self.interval_ticks(), tick + stasis));
        self.ready_at_tick = tick + ticks(self.effect_cooldown_seconds);
        self.undying_added_tick = None;
        self.watch_until_tick = tick + WATCH_TICKS;
        self.log(
            ctx,
            entity,
            &format!(
                "REBIRTH banish={banished} | {}",
                Self::player_state(ctx, player)
            ),
        );
    }

    fn on_spawn(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if !DIAGNOSTIC {
            return;
        }
        let entity = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| c.id())
            .unwrap_or(usize::MAX);
        self.log(
            ctx,
            entity,
            &format!("on_spawn | {}", Self::player_state(ctx, player)),
        );
    }

    fn on_dead(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        if !DIAGNOSTIC {
            return;
        }
        let entity = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .map(|c| c.id())
            .unwrap_or(usize::MAX);
        self.log(
            ctx,
            entity,
            &format!("on_dead | {}", Self::player_state(ctx, player)),
        );
    }

    // The cooldown carries into Radiant Guardian Angel.
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

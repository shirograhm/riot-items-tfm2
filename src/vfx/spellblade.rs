//! Spellblade: using an Ability empowers the next basic attack within
//! `WINDOW_SECONDS`. Shared by Sheen and everything built from it (Trinity
//! Force, Dusk and Dawn, Lich Bane, Essence Reaver, Bloodsong); each item owns
//! its numbers and deals its own bonus damage.
//!
//! No hook reports a cast, so one is read the way Zeke's Convergence reads its
//! ult: an ability's remaining cooldown going *up* between two ticks, since it
//! only ever counts down otherwise. That covers every ability, the ones that
//! hit nothing included. While the empowered attack is up the carrier holds
//! `SPARKS_BUFF`, and the attack that spends it plays a burst on its target.

use std::cell::Cell;
use std::sync::Mutex;

use mod_api_stable::*;

use crate::{has_buff, percent_of, refresh_buff, ticks, TICKS_PER_SECOND};

/// Starts when an empowered attack lands and keeps a cast from readying the
/// next one until it runs out. One name for every Spellblade item, so they
/// share it.
const COOLDOWN_BUFF: &str = "spellblade_cooldown";
/// How long a cast keeps the next basic attack empowered; an unused one is
/// lost. Another cast while it is up restarts it. The tooltips state it as a
/// fixed 10 seconds, so it is not read from config.
const WINDOW_SECONDS: f64 = 10.0;
/// Statless marker on a champion whose next basic attack is empowered. It is
/// the `view_buffs` binding in `view/effects.view_effects` that draws sparks
/// circling the champion's hands (`effects/spellblade_sparks`).
const SPARKS_BUFF: &str = "riot_spellblade";
/// Refreshed once a second while Spellblade is up, so it never lapses between
/// refreshes, and gone half a second after a missed one: the champion died,
/// or the item left.
const SPARKS_TICKS: usize = 90;
/// The `view_effects` burst that plays on the target of the empowered attack
/// (`effects/spellblade_proc`), in the sparks' colours.
const PROC_EFFECT: &str = "riot_spellblade_proc";

/// What a Spellblade item adds to the empowered attack, as a sum of parts that
/// each read one thing off the carrier. Every Spellblade item states its bonus
/// as one of these, so that any of them can work out what the others its
/// carrier holds would deal ([`Spellblade::wins`]).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SpellbladeBonus {
    pub(crate) flat: usize,
    /// Added once for every level past the first.
    pub(crate) per_level: usize,
    pub(crate) ad_percent: f64,
    pub(crate) ap_percent: f64,
    /// Percent of the carrier's critical strike chance, a point for a point
    /// at 100.
    pub(crate) crit_percent: f64,
}

impl SpellbladeBonus {
    /// A bonus that grows evenly from `min` at level 1 to `max` at level 12.
    pub(crate) fn by_level(min: usize, max: usize) -> Self {
        Self {
            flat: min,
            per_level: (max.saturating_sub(min) as f64 / 11.0).round() as usize,
            ..Self::default()
        }
    }

    /// The bonus on `carrier`'s stats as they are now.
    pub(crate) fn of(&self, carrier: &StableEntity<'_, '_>) -> usize {
        let stat = carrier.stat();
        self.flat
            + carrier.level().saturating_sub(1) * self.per_level
            + percent_of(stat.attack, self.ad_percent)
            + percent_of(stat.magic_power, self.ap_percent)
            + percent_of(stat.crit_chance, self.crit_percent)
    }
}

/// Every Spellblade item's bonus by item key, noted once when the mod
/// registers its items.
static BONUSES: Mutex<Vec<(&'static str, SpellbladeBonus)>> = Mutex::new(Vec::new());

thread_local! {
    /// The swing a Spellblade item last went off on, as (tick, champion). Two
    /// copies of one item both rank themselves the strongest, and this is what
    /// tells the second that the first has taken the swing. A champion's
    /// items are reached one after another on a swing, so it is only ever
    /// read within the swing that set it, and every `update` clears it.
    static CLAIMED: Cell<Option<(usize, usize)>> = Cell::new(None);
}

/// The Spellblade items `player` holds and what each adds, in inventory order.
fn held(ctx: &StableSim<'_>, player: usize) -> Vec<(&'static str, SpellbladeBonus)> {
    let Some(keys) = ctx.get_player(player).map(|p| p.item_keys()) else {
        return Vec::new();
    };
    let Ok(bonuses) = BONUSES.lock() else {
        return Vec::new();
    };
    keys.iter()
        .filter_map(|held| bonuses.iter().find(|(key, _)| *key == held.as_str()).copied())
        .collect()
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Spellblade {
    /// The next basic attack is empowered.
    ready: bool,
    /// Ticks left before an unused empowered attack is lost.
    window: usize,
    /// The carrier's remaining ability cooldowns (skill, skill2, ult) last
    /// tick. `None` until the first reading after a spawn, which is only a
    /// baseline.
    last_cooldowns: Option<(usize, usize, usize)>,
    /// The player whose champion carries this, as of the last `update`.
    holder: usize,
    /// The Spellblade items the carrier held when the cast readied this one:
    /// the ones charged along with it, which the strongest is picked among. An
    /// item bought since is not charged yet and has no say.
    charged: Vec<&'static str>,
}

impl Spellblade {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.ready
    }

    /// Notes what the Spellblade item `key` adds to the empowered attack.
    pub(crate) fn note_bonus(key: &'static str, bonus: SpellbladeBonus) {
        if let Ok(mut bonuses) = BONUSES.lock() {
            bonuses.retain(|&(noted, _)| noted != key);
            bonuses.push((key, bonus));
        }
    }

    /// Whether the item `key` is the one whose Spellblade goes off on
    /// `caster`'s swing. A champion that holds several Spellblade items deals
    /// only the strongest one's ([`Spellblade::strongest`]), and one copy's
    /// when it holds that item twice. The others let the swing go, their
    /// empowered attack used with it, which this sees to for an item that is
    /// not the one.
    pub(crate) fn wins(&mut self, ctx: &StableSim<'_>, caster: usize, key: &str) -> bool {
        let swing = (ctx.tick(), caster);
        let wins = self.strongest(ctx, caster).is_none_or(|strongest| strongest == key)
            && CLAIMED.with(|claimed| claimed.replace(Some(swing)) != Some(swing));
        if !wins {
            self.ready = false;
        }
        wins
    }

    /// The Spellblade item that goes off for the champion `caster`: of the
    /// ones charged along with this one that its player still holds, the one
    /// whose bonus is highest on the champion's stats as they are now, and the
    /// earliest in the inventory on a tie. Every charged item asks this on the
    /// same swing and gets the same answer.
    fn strongest(&self, ctx: &StableSim<'_>, caster: usize) -> Option<&'static str> {
        let carrier = ctx.get_entity(caster)?;
        let mut best: Option<(&'static str, usize)> = None;
        for (key, bonus) in held(ctx, self.holder) {
            if !self.charged.contains(&key) {
                continue;
            }
            let amount = bonus.of(&carrier);
            if best.is_none_or(|(_, top)| amount > top) {
                best = Some((key, amount));
            }
        }
        best.map(|(key, _)| key)
    }

    /// Readies Spellblade when the carrier casts, unless the last empowered
    /// attack's cooldown is still running; a cast while it is ready restarts
    /// the window instead. Keeps the sparks up while it is ready and takes
    /// them down when the window runs out. Call once per `update`.
    pub(crate) fn update(&mut self, ctx: &mut StableSim<'_>, player: usize) {
        CLAIMED.with(|claimed| claimed.set(None));
        self.holder = player;
        let cooldowns = ctx
            .get_player(player)
            .and_then(|p| p.cooldowns())
            .map(|(_, skill, skill2, ult)| (skill, skill2, ult));
        let cast = matches!(
            (cooldowns, self.last_cooldowns),
            (Some(now), Some(before))
                if now.0 > before.0 || now.1 > before.1 || now.2 > before.2
        );
        self.last_cooldowns = cooldowns;

        self.window = self.window.saturating_sub(1);
        let expired = self.ready && self.window == 0;
        if expired {
            self.ready = false;
        }

        let Some((champion, cooling_down)) = ctx
            .get_player(player)
            .and_then(|p| p.champion())
            .filter(|c| c.is_alive())
            .map(|c| (c.id(), has_buff(&c, COOLDOWN_BUFF)))
        else {
            return;
        };
        if cast && !cooling_down {
            if !self.ready {
                mark(ctx, champion);
            }
            self.ready = true;
            self.window = ticks(WINDOW_SECONDS);
            self.charged = held(ctx, player).into_iter().map(|(key, _)| key).collect();
        } else if expired {
            ctx.entity_remove_buff(champion, SPARKS_BUFF);
        } else if self.ready && ctx.tick() % TICKS_PER_SECOND as usize == 0 {
            mark(ctx, champion);
        }
    }

    /// Spends the empowered attack: starts the shared cooldown, takes the
    /// sparks off `caster` and plays the burst on `target`. For a Spellblade
    /// item's `on_attack`, alongside its bonus damage.
    pub(crate) fn spend(
        &mut self,
        ctx: &mut StableSim<'_>,
        caster: usize,
        target: usize,
        cooldown_seconds: f64,
    ) {
        self.ready = false;
        ctx.add_buff(
            caster,
            &BuffV1::timed(COOLDOWN_BUFF, ticks(cooldown_seconds)),
        );
        ctx.entity_remove_buff(caster, SPARKS_BUFF);
        ctx.play_view_effect(PROC_EFFECT, caster, &InputTargetV1::target(target), 0, 0, 0);
    }
}

/// Puts up (or keeps up) the sparks on `entity`. Replacing rather than adding
/// keeps one instance however many Spellblade items it holds, and the view does
/// not restart an animation whose buff was replaced in one tick.
fn mark(ctx: &mut StableSim<'_>, entity: usize) {
    refresh_buff(
        ctx,
        entity,
        SPARKS_BUFF,
        &BuffV1::timed(SPARKS_BUFF, SPARKS_TICKS),
    );
}

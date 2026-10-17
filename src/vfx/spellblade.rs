//! Spellblade: using an Ability empowers the next basic attack within
//! `WINDOW_SECONDS`. Shared by Sheen and everything built from it, each item
//! dealing its own bonus.
//!
//! A cast is read as an ability's remaining cooldown going up between ticks.
//! While empowered the carrier holds `SPARKS_BUFF`, and the attack that spends
//! it plays a burst on its target.

use std::cell::Cell;
use std::sync::Mutex;

use mod_api_stable::*;

use crate::{has_buff, percent_of, refresh_buff, ticks, TICKS_PER_SECOND};

// Shared cooldown after an empowered attack lands, during which a cast does not
// ready the next one.
const COOLDOWN_BUFF: &str = "spellblade_cooldown";
// How long a cast keeps the next basic attack empowered; another cast restarts
// it. Fixed at the tooltips' 10 seconds, not read from config.
const WINDOW_SECONDS: f64 = 10.0;
// Statless marker on an empowered champion; its `view_buffs` binding draws
// sparks circling the hands.
const SPARKS_BUFF: &str = "riot_spellblade";
// Refreshed once a second while ready, so it lapses half a second after a
// missed refresh.
const SPARKS_TICKS: usize = 90;
// The `view_effects` burst that plays on the target of the empowered attack
// (`effects/spellblade_proc`), in the sparks' colours.
const PROC_EFFECT: &str = "riot_spellblade_proc";

// What a Spellblade item adds to the empowered attack, stated in parts so any
// item can work out the others' bonus ([`Spellblade::wins`]).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SpellbladeBonus {
    pub(crate) flat: usize,
    // Added once for every level past the first.
    pub(crate) per_level: usize,
    pub(crate) ad_percent: f64,
    pub(crate) ap_percent: f64,
    // Percent of the carrier's critical strike chance (100 is a point for a
    // point).
    pub(crate) crit_percent: f64,
}

impl SpellbladeBonus {
    // A bonus that grows evenly from `min` at level 1 to `max` at level 12.
    pub(crate) fn by_level(min: usize, max: usize) -> Self {
        Self {
            flat: min,
            per_level: (max.saturating_sub(min) as f64 / 11.0).round() as usize,
            ..Self::default()
        }
    }

    // The bonus on `carrier`'s stats as they are now.
    pub(crate) fn of(&self, carrier: &StableEntity<'_, '_>) -> usize {
        let stat = carrier.stat();
        self.flat
            + carrier.level().saturating_sub(1) * self.per_level
            + percent_of(stat.attack, self.ad_percent)
            + percent_of(stat.magic_power, self.ap_percent)
            + percent_of(stat.crit_chance, self.crit_percent)
    }
}

// Every Spellblade item's bonus by item key, noted once when the mod
// registers its items.
static BONUSES: Mutex<Vec<(&'static str, SpellbladeBonus)>> = Mutex::new(Vec::new());

thread_local! {
    // The swing a Spellblade item last went off on, as (tick, champion), so a
    // second copy of the winning item stands down. Cleared every `update`.
    static CLAIMED: Cell<Option<(usize, usize)>> = Cell::new(None);
}

// The Spellblade items `player` holds and what each adds, in inventory order.
fn held(ctx: &StableSim<'_>, player: usize) -> Vec<(&'static str, SpellbladeBonus)> {
    let Some(keys) = ctx.get_player(player).map(|p| p.item_keys()) else {
        return Vec::new();
    };
    let Ok(bonuses) = BONUSES.lock() else {
        return Vec::new();
    };
    keys.iter()
        .filter_map(|held| {
            bonuses
                .iter()
                .find(|(key, _)| *key == held.as_str())
                .copied()
        })
        .collect()
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Spellblade {
    // The next basic attack is empowered.
    ready: bool,
    // Ticks left before an unused empowered attack is lost.
    window: usize,
    // Remaining ability cooldowns (skill, skill2, ult) last tick; `None` until
    // the first reading after a spawn.
    last_cooldowns: Option<(usize, usize, usize)>,
    // The player whose champion carries this, as of the last `update`.
    holder: usize,
    // The Spellblade items held when the cast readied this one; the strongest
    // is picked among them.
    charged: Vec<&'static str>,
}

impl Spellblade {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.ready
    }

    // Notes what the Spellblade item `key` adds to the empowered attack.
    pub(crate) fn note_bonus(key: &'static str, bonus: SpellbladeBonus) {
        if let Ok(mut bonuses) = BONUSES.lock() {
            bonuses.retain(|&(noted, _)| noted != key);
            bonuses.push((key, bonus));
        }
    }

    // Whether item `key` is the one whose Spellblade goes off on `caster`'s
    // swing. Only the strongest fires (one copy if held twice); the others
    // spend their empowered attack with it.
    pub(crate) fn wins(&mut self, ctx: &StableSim<'_>, caster: usize, key: &str) -> bool {
        let swing = (ctx.tick(), caster);
        let wins = self
            .strongest(ctx, caster)
            .is_none_or(|strongest| strongest == key)
            && CLAIMED.with(|claimed| claimed.replace(Some(swing)) != Some(swing));
        if !wins {
            self.ready = false;
        }
        wins
    }

    // The charged item still held whose bonus is highest on the champion's
    // current stats, earliest in the inventory on a tie; every charged item
    // gets the same answer.
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

    // Readies Spellblade on a cast unless its cooldown is running (a cast while
    // ready restarts the window), and keeps the sparks up while ready. Call
    // once per `update`.
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

    // Spends the empowered attack: starts the shared cooldown, takes the
    // sparks off `caster` and plays the burst on `target`. For a Spellblade
    // item's `on_attack`, alongside its bonus damage.
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

// Puts up or keeps up the sparks on `entity`; replacing keeps one instance and
// does not restart the animation.
fn mark(ctx: &mut StableSim<'_>, entity: usize) {
    refresh_buff(
        ctx,
        entity,
        SPARKS_BUFF,
        &BuffV1::timed(SPARKS_BUFF, SPARKS_TICKS),
    );
}

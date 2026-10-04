//! Shared Riches: a little gold at a steady rate for whoever holds the item.
//! World Atlas pays it, and so does everything it grows into (Runic Compass
//! and the finished support items); each item owns its amount and its interval.
//!
//! The gold comes in whether the carrier is alive or not: the tooltip promises
//! gold every so many seconds, not every so many seconds alive. An item cannot
//! keep that promise from its own `update`, which the engine does not run for
//! a dead carrier, so the paying is done from the match hook
//! ([`crate::sunfire::MatchHooks`]), which runs every tick whoever is alive.
//!
//! It goes by the match clock, which leaves nothing to keep per item or per
//! match: an item that pays every five seconds pays each time the clock
//! reaches another five, to every player holding it at that moment. The first
//! payment after a purchase can therefore come sooner than a full interval,
//! and an upgrade does not start the wait over.

use mod_api_stable::StableSim;

use crate::ticks;

/// What one item pays its holder.
#[derive(Clone, Debug)]
struct Rate {
    key: &'static str,
    gold: usize,
    /// Ticks between two payments.
    every: usize,
}

/// The items that pay Shared Riches, each with the rate it was configured
/// with. Filled as they are registered in `lib.rs`.
#[derive(Clone, Debug, Default)]
pub(crate) struct SharedRiches {
    rates: Vec<Rate>,
}

impl SharedRiches {
    /// Notes that the item `key` pays `gold` every `interval_seconds`.
    pub(crate) fn add(&mut self, key: &'static str, (gold, interval_seconds): (usize, f64)) {
        self.rates.push(Rate {
            key,
            gold,
            every: ticks(interval_seconds).max(1),
        });
    }

    /// Pays every player what the items they hold owe them this tick, dead or
    /// alive. Call once per match tick.
    pub(crate) fn pay(&self, sim: &mut StableSim<'_>) {
        let tick = sim.tick();
        let due = |rate: &Rate| rate.gold > 0 && tick % rate.every == 0;
        // Most ticks nothing is due, and nobody's items need looking at.
        if tick == 0 || !self.rates.iter().any(due) {
            return;
        }

        let mut payments = Vec::new();
        for index in 0..sim.player_count() {
            let Some(player) = sim.player_at(index) else {
                continue;
            };
            let keys = player.item_keys();
            // An item held twice pays twice.
            let gold: usize = self
                .rates
                .iter()
                .filter(|rate| due(rate))
                .map(|rate| rate.gold * keys.iter().filter(|key| key.as_str() == rate.key).count())
                .sum();
            if gold > 0 {
                payments.push((player.id(), gold));
            }
        }
        for (player, gold) in payments {
            sim.player_add_gold(player, gold as i64);
        }
    }
}

//! Shared Riches: gold at a steady rate for whoever holds World Atlas or
//! anything it grows into; each item owns its amount and interval.
//!
//! Paid from the match hook ([`crate::sunfire::MatchHooks`]), since an item's
//! `update` does not run for a dead carrier, and by the match clock: each
//! holder is paid whenever the clock reaches another interval, so the first
//! payment can come early and an upgrade does not restart the wait.

use mod_api_stable::StableSim;

use crate::ticks;

// What one item pays its holder.
#[derive(Clone, Debug)]
struct Rate {
    key: &'static str,
    gold: usize,
    // Ticks between two payments.
    every: usize,
}

// The items that pay Shared Riches, each with the rate it was configured
// with. Filled as they are registered in `lib.rs`.
#[derive(Clone, Debug, Default)]
pub(crate) struct SharedRiches {
    rates: Vec<Rate>,
}

impl SharedRiches {
    // Notes that the item `key` pays `gold` every `interval_seconds`.
    pub(crate) fn add(&mut self, key: &'static str, (gold, interval_seconds): (usize, f64)) {
        self.rates.push(Rate {
            key,
            gold,
            every: ticks(interval_seconds).max(1),
        });
    }

    // Pays every player what the items they hold owe them this tick, dead or
    // alive. Call once per match tick.
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

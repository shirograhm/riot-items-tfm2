//! Shared Riches: a little gold at a steady rate for whoever holds the item.
//! World Atlas pays it, and so does everything it grows into (Runic Compass,
//! Bounty of Worlds, Bloodsong); each item owns its amount and its interval.
//!
//! The clock runs whether the carrier is alive or not, and it is not reset by
//! a respawn: the tooltip promises gold every so many seconds, not every so
//! many seconds alive.

use mod_api_stable::StableSim;

use crate::ticks;

#[derive(Clone, Debug, Default)]
pub(crate) struct SharedRiches {
    /// Ticks since the last payment.
    waited: usize,
}

impl SharedRiches {
    /// Pays `gold` to `player` each time `interval_seconds` has gone by. Call
    /// once per `update`.
    pub(crate) fn update(
        &mut self,
        sim: &mut StableSim<'_>,
        player: usize,
        gold: usize,
        interval_seconds: f64,
    ) {
        self.waited += 1;
        if self.waited < ticks(interval_seconds).max(1) {
            return;
        }
        self.waited = 0;
        if gold > 0 {
            sim.player_add_gold(player, gold as i64);
        }
    }
}

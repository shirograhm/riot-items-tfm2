//! How long it has been since an item's last `update`.
//!
//! The engine runs no `update` while the carrier is dead, so a per-`update`
//! countdown pauses through a death. One meant to keep running takes off
//! [`Elapsed::since_last`] instead of one, which includes the whole death on
//! the first `update` after it.

use mod_api_stable::StableSim;

// The match tick of the last `update` that asked.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Elapsed {
    last: Option<usize>,
}

impl Elapsed {
    // Ticks since this was last asked; call once per `update`. One the first
    // time, or when the clock has not moved forward (a copy of the match
    // replayed from earlier).
    pub(crate) fn since_last(&mut self, ctx: &StableSim<'_>) -> usize {
        let now = ctx.tick();
        let gone = match self.last {
            Some(last) if now > last => now - last,
            _ => 1,
        };
        self.last = Some(now);
        gone
    }
}

//! How long it has been since an item's last `update`.
//!
//! The engine runs no `update` for an item whose carrier is dead, so a
//! countdown stepped once per `update` stands still through a death and picks
//! up where it was after the respawn. Most items clear theirs in `on_spawn`
//! and never notice. One that is meant to keep running through a death takes
//! off what [`Elapsed::since_last`] answers instead of one: a single tick from
//! one `update` to the next, and the whole of the death on the first `update`
//! after it.

use mod_api_stable::StableSim;

/// The match tick of the last `update` that asked.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Elapsed {
    last: Option<usize>,
}

impl Elapsed {
    /// Ticks gone by since this was last asked. Call once per `update`. One
    /// tick the first time, and whenever the clock does not read later than it
    /// did (a copy of the match played on from an earlier point), which is the
    /// plain once-per-`update` step.
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

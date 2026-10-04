//! Shared plumbing: config loading, constants, item metadata, the delayed proc
//! queue, the own-team debug log, the carry across an item upgrade, the watches
//! for the heals a carrier lands on allies and for the enemies it immobilizes,
//! the World Atlas line's gold income, and the clock for countdowns that keep
//! running through a death. `lib.rs` re-exports what the rest of the crate
//! reaches for, so these are addressed from the crate root.

pub(crate) mod config;
pub(crate) mod constants;
pub(crate) mod elapsed;
pub(crate) mod heal_watch;
pub(crate) mod immobilize_watch;
pub(crate) mod item_meta;
pub(crate) mod own_team_log;
pub(crate) mod proc_queue;
pub(crate) mod shared_riches;
pub(crate) mod upgrade_carry;

//! Shared plumbing: config loading, constants, item metadata, the delayed proc
//! queue and the own-team debug log. `lib.rs` re-exports what the rest of the
//! crate reaches for, so these are addressed from the crate root.

pub(crate) mod config;
pub(crate) mod constants;
pub(crate) mod item_meta;
pub(crate) mod own_team_log;
pub(crate) mod proc_queue;

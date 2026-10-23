//! Shared plumbing, re-exported from `lib.rs` so the rest of the crate reaches
//! it from the crate root.

pub(crate) mod config;
pub(crate) mod constants;
pub(crate) mod elapsed;
pub(crate) mod extra_stats;
pub(crate) mod game_text;
pub(crate) mod heal_watch;
pub(crate) mod immobilize_watch;
pub(crate) mod item_meta;
pub(crate) mod match_rules;
pub(crate) mod own_team_log;
pub(crate) mod perf;
pub(crate) mod proc_queue;
pub(crate) mod shared_riches;
pub(crate) mod upgrade_carry;

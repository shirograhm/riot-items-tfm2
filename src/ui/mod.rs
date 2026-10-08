//! In-game UI the mod adds or reshapes: the Builds tab on the strategy screen
//! and the solo-rank match history's item row. The Item Stats tab lives with
//! its numbers in `item_stats::ui`. `draft_watch` adds nothing: it reads the
//! draft screen's picks for the Builds tab to use. `match_builds` draws the
//! player's builds in the in-match Check Tactics panel.

pub(crate) mod draft_watch;
pub(crate) mod match_builds;
pub(crate) mod solo_rank_ui;
pub(crate) mod strategy_ui;

//! In-game UI the mod adds or reshapes: the Builds tab on the strategy screen
//! and the solo-rank match history's item row. The Item Stats tab lives with
//! its numbers in `item_stats::ui`. `draft_watch` adds nothing: it reads the
//! draft screen's picks for the Builds tab to use. `match_builds` draws the
//! player's builds in the in-match Check Tactics panel. `patch_settings`
//! adds the Item Patcher Settings row to the game's Options dialog, with the
//! window that row opens, and `options_dialog` finds that dialog for it.

pub(crate) mod draft_watch;
pub(crate) mod match_builds;
mod options_dialog;
pub(crate) mod patch_settings;
pub(crate) mod solo_rank_ui;
pub(crate) mod strategy_ui;

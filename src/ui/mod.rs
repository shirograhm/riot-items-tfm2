//! In-game UI the mod adds or reshapes: the Builds tab on the strategy screen
//! and the solo-rank match history's item row. The Item Stats tab lives with
//! its numbers in `item_stats::ui`. `draft_watch` adds nothing: it reads the
//! draft screen's picks for the Builds tab to use. `match_builds` draws the
//! player's builds in the in-match Check Tactics panel, and `item_spend`
//! has the item tooltips of a match show what an item has cost its athlete
//! in all. `item_counts` draws what an item has built up over the match on
//! its icon in the player rows. Two rows are added to the game's Options
//! dialog, both found through `options_dialog`: `options_row`'s Item Rules,
//! and under it `patch_settings`' Item Patcher Settings, with the window
//! that row opens.

pub(crate) mod draft_watch;
pub(crate) mod item_counts;
pub(crate) mod item_spend;
pub(crate) mod match_builds;
mod options_dialog;
pub(crate) mod options_row;
pub(crate) mod patch_settings;
pub(crate) mod solo_rank_ui;
pub(crate) mod strategy_ui;

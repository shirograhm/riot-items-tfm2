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

use std::collections::BTreeSet;
use std::sync::Mutex;

use mod_api_stable::StableClient;

// The paths a screen has registered its UI event handler on, so that each is
// registered once while its node lives (`strategy_ui`, `match_builds`,
// `item_stats::ui`).
//
// `ui_register_path_events` keys a handler by the path *string*, not by the
// node, and the handler lives until process exit. Registering one path twice
// makes one click run the handler twice, and a handler outlives the node it
// was registered for. So a screen registers every path through its set, and
// forgets the set when the screen goes away so that the next visit wires
// itself again. Forgetting it while the screen is up brings the double fire
// back.
pub(crate) struct Registered {
    paths: Mutex<BTreeSet<String>>,
    handler: fn(&mut StableClient<'_>),
}

impl Registered {
    pub(crate) const fn new(handler: fn(&mut StableClient<'_>)) -> Self {
        Self {
            paths: Mutex::new(BTreeSet::new()),
            handler,
        }
    }

    // Registers the handler for `path`, unless it has one already. A path
    // the host refuses is forgotten again, so that a later call retries
    // rather than leaving a dead control behind.
    pub(crate) fn register(&self, ctx: &mut StableClient<'_>, path: &str) {
        let fresh = self
            .paths
            .lock()
            .map(|mut set| set.insert(path.to_string()))
            .unwrap_or(false);
        if !fresh {
            return;
        }
        if !ctx.ui_register_path_events(path, self.handler) {
            if let Ok(mut set) = self.paths.lock() {
                set.remove(path);
            }
        }
    }

    // Forgets every path: the screen, and every node registered on it, is
    // gone.
    pub(crate) fn forget(&self) {
        if let Ok(mut set) = self.paths.lock() {
            set.clear();
        }
    }
}

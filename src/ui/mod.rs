//! In-game UI the mod adds or reshapes: the Builds tab (`strategy_ui`), the
//! solo-rank match history's item row, the Check Tactics builds
//! (`match_builds`), match tooltips (`item_spend`) and icon counts
//! (`item_counts`), and two Options dialog rows (`options_row`,
//! `patch_settings`). The Item Stats tab lives in `item_stats::ui`.

pub(crate) mod draft_watch;
pub(crate) mod item_counts;
pub(crate) mod item_spend;
pub(crate) mod match_builds;
mod name_search;
mod options_dialog;
pub(crate) mod options_row;
pub(crate) mod patch_settings;
pub(crate) mod solo_rank_ui;
pub(crate) mod strategy_ui;

use std::collections::BTreeSet;
use std::sync::Mutex;

use mod_api_stable::StableClient;

// The paths a screen has registered its UI handler on. A handler is keyed by
// the path string and lives until process exit, so registering a path twice
// fires one click twice; each screen registers through its set and forgets it
// when the screen goes away, not before.
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

    // Registers the handler for `path` unless it has one; a path the host
    // refuses is forgotten, so a later call retries.
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

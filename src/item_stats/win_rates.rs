//! Smart Builds' win-rate lean: how each champion has done holding each item,
//! from the champion-and-item results Item Stats folds ([`super::fold`]).
//!
//! A champion's item rate is drawn toward the champion's own rate over all its
//! items until it has [`TRUSTED_GAMES`] games, and says nothing under
//! [`MIN_GAMES`]. The lean is that rate's distance from the champion's own,
//! -1 to 1 at [`FULL_LEAN`]: the same scale as the balance patches' lean
//! (`crate::patches::leaning`), and it rides alongside it in the same places:
//! the stable hook's item scores, the 5th/6th items' network pick, and Smart
//! Builds' choice among stand-ins.
//!
//! Every match is simulated twice (the server's run and the watched one), and
//! a fold between the two must not change the build. So a lineup keeps the
//! table it was first asked with ([`for_lineup`]) until the save changes.
//!
//! Off in multiplayer: each player's save holds its own numbers.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Kill switch for the whole lean.
const ENABLED: bool = true;

// Games under which a champion's item rate says nothing.
const MIN_GAMES: u32 = 10;

// Games at which a champion's item rate counts as much as the champion's own
// rate it is drawn toward. At 30 games it sits halfway between them.
const TRUSTED_GAMES: f64 = 30.0;

// Distance from the champion's own rate, in win rate, at which the lean is
// full (1 or -1).
const FULL_LEAN: f64 = 0.10;

// What the stable hook adds to an item's score at full lean: the balance
// patches' `crate::patches::LEAN_SCORE`, on the scale of `item_build_hook`'s
// bonuses (a mod final gets 0.5).
pub(crate) const LEAN_SCORE: f32 = 0.2;

// What the 5th and 6th items' network pick adds at full lean, as a share of how
// far apart the network puts its candidates: the balance patches'
// `crate::patches::LEAN_SPREAD`.
pub(crate) const LEAN_SPREAD: f32 = 0.25;

// Lineups held before [`FROZEN`] starts over: a few hundred matches.
const FROZEN_MAX: usize = 4096;

// One champion's results: per item key, games and wins with it held, and the
// sums over all its items.
#[derive(Default)]
pub(crate) struct ChampionResults {
    items: HashMap<String, (u32, u32)>,
    games: u32,
    wins: u32,
}

impl ChampionResults {
    pub(crate) fn add(&mut self, item: &str, games: u32, wins: u32) {
        let entry = self.items.entry(item.to_string()).or_default();
        entry.0 += games;
        entry.1 += wins;
        self.games += games;
        self.wins += wins;
    }
}

// Champion key -> results.
pub(crate) type Table = HashMap<String, ChampionResults>;

// The table as of the last fold or load.
static PUBLISHED: Mutex<Option<Arc<Table>>> = Mutex::new(None);

// Canonical lineup -> the table it was first asked with.
static FROZEN: Mutex<Option<HashMap<String, Arc<Table>>>> = Mutex::new(None);

// Replaces the table; from the folds and the save's load.
pub(crate) fn publish(table: Table) {
    let mut guard = PUBLISHED.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(Arc::new(table));
}

// Drops the table and every frozen lineup: another save's numbers, and its
// matches, are not this one's.
pub(crate) fn forget() {
    *PUBLISHED.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *FROZEN.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn published() -> Option<Arc<Table>> {
    PUBLISHED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

fn active() -> bool {
    ENABLED && !crate::multiplayer::active()
}

// The table for the match `side` and `other` play, frozen at its first ask.
// The two sides are sorted into the key, so either team's ask finds the same
// table.
pub(crate) fn for_lineup(side: &[&str], other: &[&str]) -> Option<Arc<Table>> {
    if !active() {
        return None;
    }
    let (a, b) = (side.join(","), other.join(","));
    let key = if a <= b {
        format!("{a}|{b}")
    } else {
        format!("{b}|{a}")
    };
    let mut guard = FROZEN.lock().unwrap_or_else(|e| e.into_inner());
    let frozen = guard.get_or_insert_with(HashMap::new);
    if let Some(table) = frozen.get(&key) {
        return Some(table.clone());
    }
    let table = published()?;
    if frozen.len() >= FROZEN_MAX {
        frozen.clear();
    }
    frozen.insert(key, table.clone());
    Some(table)
}

// The table as it stands, for asks a match identity already freezes (the
// network pick's seat memo).
pub(crate) fn current() -> Option<Arc<Table>> {
    active().then(published).flatten()
}

// `champion`'s lean towards `item` in `table`, -1 to 1; 0 with too few games or
// no table.
pub(crate) fn lean(table: Option<&Table>, champion: &str, item: &str) -> f32 {
    let Some(results) = table.and_then(|table| table.get(champion)) else {
        return 0.0;
    };
    let Some(&(games, wins)) = results.items.get(item) else {
        return 0.0;
    };
    if games < MIN_GAMES || results.games == 0 {
        return 0.0;
    }
    let own = results.wins as f64 / results.games as f64;
    let rate = (wins as f64 + TRUSTED_GAMES * own) / (games as f64 + TRUSTED_GAMES);
    ((rate - own) / FULL_LEAN).clamp(-1.0, 1.0) as f32
}

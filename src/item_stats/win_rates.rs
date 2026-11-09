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
//! In multiplayer the host's numbers decide. Builds are made where a server
//! runs, which is the host's game, so they come from the host's table already.
//! The host also sends its table out with everyone's builds
//! (`crate::multiplayer`, [`wire`]), and a guest's game uses that in place of
//! its own ([`adopt_host`]). That way a guest that did make a build itself
//! would make the host's. (The 5th/6th items' network pick stays off in
//! multiplayer for its own reasons.)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

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

// On a guest's game in a multiplayer session, from the first word of the host:
// the host's table, used in place of [`PUBLISHED`]. `HostTable(None)` is a host
// with no table, so no lean.
struct HostTable(Option<Arc<Table>>);
static HOST: Mutex<Option<HostTable>> = Mutex::new(None);

// One champion's results as the host sends them: its games and wins over all
// its items, and only the items with [`MIN_GAMES`] or more (the rest lean 0).
// Sorted by champion and item, so an unchanged table compares equal and the
// server does not send it out again.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub(crate) struct WireChampion {
    champion: String,
    games: u32,
    wins: u32,
    items: Vec<(String, u32, u32)>,
}

pub(crate) type Wire = Vec<WireChampion>;

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

// The table in force: the host's on a guest that has heard it, else this
// game's own.
fn published() -> Option<Arc<Table>> {
    if let Some(HostTable(host)) = &*HOST.lock().unwrap_or_else(|e| e.into_inner()) {
        return host.clone();
    }
    PUBLISHED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

fn active() -> bool {
    ENABLED
}

// This game's own table for the wire; on the host's server, for every guest.
pub(crate) fn wire() -> Option<Wire> {
    if !ENABLED {
        return None;
    }
    let own = PUBLISHED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()?;
    let mut wire: Wire = own
        .iter()
        .map(|(champion, results)| {
            let mut items: Vec<(String, u32, u32)> = results
                .items
                .iter()
                .filter(|(_, counts)| counts.0 >= MIN_GAMES)
                .map(|(item, &(games, wins))| (item.clone(), games, wins))
                .collect();
            items.sort();
            WireChampion {
                champion: champion.clone(),
                games: results.games,
                wins: results.wins,
                items,
            }
        })
        .collect();
    wire.sort_by(|a, b| a.champion.cmp(&b.champion));
    Some(wire)
}

// The host's table, heard on a guest's game; `None` for a host that sent none.
// The first word drops every lineup frozen on this game's own numbers; after
// that, a lineup keeps the host table it was first asked with, as with
// [`publish`].
pub(crate) fn adopt_host(wire: Option<Wire>) {
    let table = wire.map(|wire| {
        let mut table = Table::new();
        for champion in wire {
            table.insert(
                champion.champion,
                ChampionResults {
                    items: champion
                        .items
                        .into_iter()
                        .map(|(item, games, wins)| (item, (games, wins)))
                        .collect(),
                    games: champion.games,
                    wins: champion.wins,
                },
            );
        }
        Arc::new(table)
    });
    let first = {
        let mut host = HOST.lock().unwrap_or_else(|e| e.into_inner());
        let first = host.is_none();
        *host = Some(HostTable(table));
        first
    };
    if first {
        *FROZEN.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

// The session is over: back to this game's own numbers, and no lineup keeps a
// host table.
pub(crate) fn drop_host() {
    let had = HOST.lock().unwrap_or_else(|e| e.into_inner()).take().is_some();
    if had {
        *FROZEN.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
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

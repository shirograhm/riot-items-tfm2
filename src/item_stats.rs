//! Per-item win/loss totals, folded out of the game's own saved match records.
//!
//! # Why records and not a sim hook
//!
//! The game already persists everything this needs. `MatchReplayData` (record
//! kind [`RecordKindV1::MatchReplay`]) carries `blue_team_win` alongside
//! `blue_team`/`red_team`, and each entry in those is a player with an `items`
//! list — so "which items were on the board and did that side win" is a read,
//! not something that has to be observed as it happens.
//!
//! That matters for two reasons. It covers matches simmed **before** this code
//! ever ran, which a [`StableMatchHook`] never could; and it counts AI-vs-AI
//! league games the player never watched, which is most of the season. The
//! alternative — folding `player.item_keys()` against `score_diff` at
//! `sim.is_end()` — also has to filter `sim_origin()` so that watching your own
//! match live does not count it a second time. Reading the record afterwards has
//! no such trap: one record is one match.
//!
//! # What the record actually looks like
//!
//! Two things about it could not be established offline (the save is compressed,
//! so its field encoding is not greppable). [`diagnose`] settled both on the
//! first run, against a live save:
//!
//! ```text
//! record id=41 is_brief=Some(true) keys=[…, "blue_team", "blue_team_win", …]
//!   blue_team=[{"athlete_id":776,"champion":"test_mod_galio","id":0,
//!               "items":[24,24,24],"position":"Top", …}, …]
//! ```
//!
//! * **`is_brief` does not strip the detail.** That record is flagged brief and
//!   still carries every player, their champion and their items. So the feared
//!   case — most of a season stored with no items — does not happen, and the
//!   fallback of instrumenting the sim is not needed. [`Snapshot::brief`] still
//!   counts records that yield nothing, because "flagged brief" and "has no
//!   items" turned out to be different questions.
//! * **`items` holds numbers, not keys.** They index the item settings document
//!   in declaration order; see [`index_table`], which is what turns 24 back
//!   into `prophet_of_the_abyss`.
//!
//! [`item_key`] still accepts strings and objects, because the numeric form is
//! something this mod inferred from one build rather than something the ABI
//! promises.
//!
//! # Retention
//!
//! Totals live for the process and are rebuilt per session. If the game turns
//! out to prune old match records, this under-counts old seasons and the fix is
//! to accumulate into the save (`save_set_string`, which the mod's server
//! extension can already reach) instead of recomputing. [`sweep`] detects the
//! pruning case — a previously folded id going missing resets everything rather
//! than leaving totals that describe records that no longer exist.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use mod_api_stable::*;
use serde_json::Value;

/// Records folded per [`pump`] call.
///
/// Each one is an FFI round trip plus a JSON parse of a document that holds ten
/// players' full match statistics, and this runs on the UI thread. A season is
/// hundreds of matches, so the whole set cannot be folded in one frame without a
/// visible hitch; at 24 per frame a thousand records take under a second of
/// scrolling-smooth catch-up, and the panel shows its progress while it happens.
const CHUNK: usize = 24;

#[derive(Clone, Copy, Default)]
pub(crate) struct Totals {
    pub games: u32,
    pub wins: u32,
}

impl Totals {
    pub fn losses(&self) -> u32 {
        self.games.saturating_sub(self.wins)
    }

    /// Win rate in percent, or `None` for an item with no games — which is not
    /// the same as 0% and must not print as it.
    pub fn win_rate(&self) -> Option<f64> {
        (self.games > 0).then(|| self.wins as f64 * 100.0 / self.games as f64)
    }
}

#[derive(Default)]
struct Aggregate {
    /// Record ids already folded in. Also the guard against double counting a
    /// record that a later sweep hands back.
    seen: BTreeSet<usize>,
    /// Ids found by the last sweep and not yet folded, newest first so the
    /// table is useful before the scan finishes.
    pending: Vec<usize>,
    /// Patch -> item -> totals.
    ///
    /// Keyed by patch first so the filter is a map lookup rather than a re-scan:
    /// picking one reads its submap, and "All" merges them. The record's
    /// `version` is what a patch is here — it sits beside `seed` in the replay
    /// data, which is what a replay would need to reproduce the balance a match
    /// was played under.
    counts: BTreeMap<String, BTreeMap<String, Totals>>,
    /// Patch -> item -> champion -> times that champion was holding it. Feeds
    /// the "purchased on" column, which is the top few of these by count.
    champions: BTreeMap<String, BTreeMap<String, BTreeMap<String, u32>>>,
    /// Patch -> matches that yielded at least one item.
    matches: BTreeMap<String, u32>,
    /// Records that parsed but held no per-player items.
    brief: u32,
    /// Records that could not be read at all.
    unreadable: u32,
}

static AGG: Mutex<Option<Aggregate>> = Mutex::new(None);

fn with_agg<T>(f: impl FnOnce(&mut Aggregate) -> T) -> Option<T> {
    let mut guard = AGG.lock().ok()?;
    Some(f(guard.get_or_insert_with(Aggregate::default)))
}

/// What the panel draws.
pub(crate) struct Snapshot {
    /// Items in display order — see [`rows`].
    pub rows: Vec<(String, Totals)>,
    pub matches: u32,
    /// Records that parsed but carried no per-player items.
    pub brief: u32,
    /// Records that could not be read at all. Distinct from `brief`: that is
    /// the game choosing not to keep the detail, this is a read that failed,
    /// and while the record format is still an inference the difference is
    /// exactly what tells one from the other.
    pub unreadable: u32,
    /// Records still to fold. Non-zero means the table is incomplete.
    pub pending: usize,
    /// Per item, the champions that bought it most, best first, at most
    /// [`TOP_CHAMPIONS`] of them.
    pub champions: BTreeMap<String, Vec<String>>,
}

/// How many champions the "purchased on" column shows.
///
/// Three, because that is what the vanilla "Most Used Champ" column shows and
/// the cell it borrows its shape from is 132px wide — three 40px slots and two
/// 4px gaps, with nothing left over.
pub(crate) const TOP_CHAMPIONS: usize = 3;

/// Bucket for a record that names no version.
pub(crate) const UNKNOWN_PATCH: &str = "?";

/// Refreshes the list of records to fold.
///
/// Called on entry to the statistics screen rather than per frame: `record_ids`
/// copies the whole id set across the ABI twice (once to size, once to fill),
/// which is not a per-frame cost, and matches are not simmed while the player is
/// standing on this screen anyway.
///
/// A previously folded id that is no longer present means the record set is not
/// the one the totals were built from — a different save was loaded, or the game
/// pruned history. Either way the totals describe records that are gone, so they
/// are dropped and rebuilt rather than carried forward as a mix of two saves.
pub(crate) fn sweep(ctx: &StableClient<'_>) {
    let ids = ctx.record_ids(RecordKindV1::MatchReplay);
    if ids.is_empty() {
        return;
    }
    let present: BTreeSet<usize> = ids.iter().copied().collect();

    let _ = with_agg(|agg| {
        if !agg.seen.is_subset(&present) {
            *agg = Aggregate::default();
        }
        // Newest first: match ids ascend with time, so the most recent season
        // lands in the table first and the scan fills in history behind it.
        let pending: Vec<usize> = ids
            .iter()
            .rev()
            .copied()
            .filter(|id| !agg.seen.contains(id))
            .collect();
        agg.pending = pending;
    });
}

/// Folds up to [`CHUNK`] outstanding records. Returns whether anything changed,
/// which is what tells the panel it needs repainting.
pub(crate) fn pump(ctx: &StableClient<'_>) -> bool {
    // Records name items by index, so folding before the index table exists
    // would file real games under `#24` and leave them there — the totals would
    // be a mix of two key spaces that no later pass could separate. Waiting is
    // free: `prime_catalog` runs first in the same frame.
    let index = index_table();
    if index.is_empty() {
        return false;
    }

    let batch = with_agg(|agg| {
        let take = CHUNK.min(agg.pending.len());
        agg.pending.drain(..take).collect::<Vec<_>>()
    })
    .unwrap_or_default();

    if batch.is_empty() {
        return false;
    }

    for id in batch {
        let record = read_record(ctx, id, &index);
        let _ = with_agg(|agg| {
            agg.seen.insert(id);
            match record {
                Some((patch, sides)) if sides.iter().any(|(players, _)| !players.is_empty()) => {
                    *agg.matches.entry(patch.clone()).or_default() += 1;
                    let counts = agg.counts.entry(patch.clone()).or_default();
                    for (players, won) in &sides {
                        for (_, items) in players {
                            for key in items {
                                let entry = counts.entry(key.clone()).or_default();
                                entry.games += 1;
                                entry.wins += u32::from(*won);
                            }
                        }
                    }
                    let champions = agg.champions.entry(patch).or_default();
                    for (players, _) in &sides {
                        for (champion, items) in players {
                            for key in items {
                                *champions
                                    .entry(key.clone())
                                    .or_default()
                                    .entry(champion.clone())
                                    .or_default() += 1;
                            }
                        }
                    }
                }
                Some(_) => agg.brief += 1,
                None => agg.unreadable += 1,
            }
        });
    }
    true
}

/// The patches seen in the records, newest first.
///
/// Populated from the records themselves rather than from the game's own patch
/// list, which the stable API does not expose. That also makes it exactly the
/// right set: a patch nothing was played on has nothing to filter to.
pub(crate) fn patches() -> Vec<String> {
    with_agg(|agg| {
        let mut out: Vec<String> = agg.counts.keys().cloned().collect();
        out.sort_by(|a, b| b.cmp(a));
        out
    })
    .unwrap_or_default()
}

/// The current table, in key order, for one patch or for all of them.
///
/// Deliberately *not* sorted for display: the column the player picked can be
/// the item's name, which lives in the catalog, so ordering is the UI's job.
/// Key order makes it a stable starting point, which is what keeps equal rows
/// from reshuffling between repaints mid-scan.
pub(crate) fn snapshot(patch: Option<&str>) -> Snapshot {
    with_agg(|agg| {
        // One patch reads its own submap; "All" merges them. Merging here rather
        // than keeping a second running total means there is one set of numbers
        // to be wrong, and it costs a walk of at most items x patches.
        let mut counts: BTreeMap<String, Totals> = BTreeMap::new();
        let mut champions: BTreeMap<String, BTreeMap<String, u32>> = BTreeMap::new();
        let mut matches = 0;

        for (key, per_item) in &agg.counts {
            if patch.is_some_and(|wanted| wanted != key.as_str()) {
                continue;
            }
            for (item, totals) in per_item {
                let entry = counts.entry(item.clone()).or_default();
                entry.games += totals.games;
                entry.wins += totals.wins;
            }
        }
        for (key, per_item) in &agg.champions {
            if patch.is_some_and(|wanted| wanted != key.as_str()) {
                continue;
            }
            for (item, tally) in per_item {
                let entry = champions.entry(item.clone()).or_default();
                for (champion, count) in tally {
                    *entry.entry(champion.clone()).or_default() += count;
                }
            }
        }
        for (key, count) in &agg.matches {
            if patch.is_none_or(|wanted| wanted == key.as_str()) {
                matches += count;
            }
        }

        Snapshot {
            rows: rows(&counts),
            matches,
            brief: agg.brief,
            unreadable: agg.unreadable,
            pending: agg.pending.len(),
            champions: top_champions(&champions),
        }
    })
    .unwrap_or(Snapshot {
        rows: Vec::new(),
        matches: 0,
        brief: 0,
        unreadable: 0,
        pending: 0,
        champions: BTreeMap::new(),
    })
}

/// The most frequent champions per item, best first.
///
/// Ties break on the champion key so the three shown do not swap places between
/// repaints while the scan is still folding records.
fn top_champions(tally: &BTreeMap<String, BTreeMap<String, u32>>) -> BTreeMap<String, Vec<String>> {
    tally
        .iter()
        .map(|(item, champions)| {
            let mut ranked: Vec<(&String, &u32)> = champions.iter().collect();
            ranked.sort_by(|(a_key, a), (b_key, b)| b.cmp(a).then_with(|| a_key.cmp(b_key)));
            let top = ranked
                .into_iter()
                .take(TOP_CHAMPIONS)
                .map(|(champion, _)| champion.clone())
                .collect();
            (item.clone(), top)
        })
        .collect()
}

fn rows(counts: &BTreeMap<String, Totals>) -> Vec<(String, Totals)> {
    counts
        .iter()
        .map(|(key, totals)| (key.clone(), *totals))
        .collect()
}

/// One match as `(item keys, did that side win)`, one entry per side.
///
/// The whole record is fetched in a single call where the host allows it — a
/// match is ten players and three targeted reads per record triples the FFI cost
/// of a scan that is already the expensive part of this module.
fn read_record(
    ctx: &StableClient<'_>,
    id: usize,
    index: &[String],
) -> Option<(String, Vec<(Vec<(String, Vec<String>)>, bool)>)> {
    let record = match ctx
        .record_get_json(RecordKindV1::MatchReplay, id, "")
        .and_then(|json| serde_json::from_str::<Value>(&json).ok())
    {
        Some(record) => record,
        // Older hosts, or a path grammar that does not accept the empty path for
        // this record kind. Named reads say the same thing, and are reassembled
        // into the same shape so that everything below — the diagnostic
        // included — has one case to handle rather than two.
        None => {
            let field = |name: &str| {
                ctx.record_get_json(RecordKindV1::MatchReplay, id, name)
                    .and_then(|json| serde_json::from_str::<Value>(&json).ok())
            };
            serde_json::json!({
                "blue_team_win": field("blue_team_win"),
                "is_brief": field("is_brief"),
                "version": field("version"),
                "blue_team": field("blue_team"),
                "red_team": field("red_team"),
            })
        }
    };

    diagnose(id, &record, index);

    let blue_win = record.get("blue_team_win")?.as_bool()?;

    // A record with no version still counts; it just lands in its own bucket
    // rather than being dropped, and the filter shows it for what it is.
    let patch = record
        .get("version")
        .and_then(Value::as_str)
        .filter(|version| !version.is_empty())
        .unwrap_or(UNKNOWN_PATCH)
        .to_string();

    Some((
        patch,
        vec![
            (team_players(record.get("blue_team"), index), blue_win),
            (team_players(record.get("red_team"), index), !blue_win),
        ],
    ))
}

/// Every player on one side as `(champion, item keys)`, duplicates within a
/// single player collapsed.
///
/// Written as a walk rather than as `team[i].items` because the exact nesting of
/// a side is the game's business and has moved before (see the athlete/champion
/// split in `statistics.ui`). Any object carrying an `items` array is a player;
/// nothing else in a match record has that shape.
fn team_players(team: Option<&Value>, index: &[String]) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let Some(team) = team else {
        return out;
    };
    collect_players(team, 0, &mut out, index);
    out
}

fn collect_players(
    value: &Value,
    depth: usize,
    out: &mut Vec<(String, Vec<String>)>,
    index: &[String],
) {
    if depth > 4 {
        return;
    }
    match value {
        Value::Array(entries) => {
            for entry in entries {
                collect_players(entry, depth + 1, out, index);
            }
        }
        Value::Object(fields) => {
            if let Some(Value::Array(items)) = fields.get("items") {
                // Per player, not per team: two players on a side holding the
                // same item are two games for it, which is what a pick rate
                // means. The same player holding it twice is not.
                let mut held = BTreeSet::new();
                for item in items {
                    if let Some(key) = item_key(item, index) {
                        held.insert(key);
                    }
                }
                let champion = fields
                    .get("champion")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                out.push((champion, held.into_iter().collect()));
                return;
            }
            for field in fields.values() {
                collect_players(field, depth + 1, out, index);
            }
        }
        _ => {}
    }
}

/// One entry of a player's `items` as an item key.
///
/// Records store items as bare numbers, which [`index_table`] turns back into
/// keys. An
/// index past the end of the table keeps its `#`-prefixed raw form rather than
/// being attributed to some other item — a row reading `#207` is a visible bug,
/// a row crediting the wrong item is a silent one.
///
/// The string and object arms are kept because the numeric form is something
/// this mod inferred from one build's records, not something the ABI promises.
/// `null` is an empty slot and yields nothing.
fn item_key(value: &Value, index: &[String]) -> Option<String> {
    match value {
        Value::String(key) if !key.is_empty() => Some(key.clone()),
        Value::Number(id) => {
            let slot = id.as_u64()?;
            Some(
                index
                    .get(slot as usize)
                    .cloned()
                    .unwrap_or_else(|| format!("#{slot}")),
            )
        }
        Value::Object(fields) => fields
            .get("key")
            .or_else(|| fields.get("name"))
            .or_else(|| fields.get("id"))
            .and_then(|inner| item_key(inner, index)),
        _ => None,
    }
}

// -- diagnostics ------------------------------------------------------------

/// Checks the decoded id space against ground truth, once per process.
///
/// [`index_table`] is an inference, and a wrong one would not look wrong: every
/// row would carry a real item's name and the numbers behind it would belong to
/// some other item. So it is checked rather than trusted, against the one thing
/// in this mod that ties a champion to specific item *keys* — `item-builds.json`.
/// The item-build hook forces those keys, so a pinned champion's record ids must
/// decode back to them.
///
/// Prints one line per pinned player found, `MATCH` or `MISMATCH`, plus the raw
/// first record for anything else that needs eyeballing. A `MISMATCH` line names
/// the id, what this decoded it to, and what it should have been — which is
/// enough to correct the table without another guess.
fn diagnose(id: usize, record: &Value, index: &[String]) {
    use std::sync::atomic::{AtomicBool, Ordering};

    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::Relaxed) {
        return;
    }

    let brief = record.get("is_brief").and_then(Value::as_bool);
    diag(&format!(
        "record id={id} is_brief={brief:?} version={:?} index={} entries",
        record.get("version"),
        index.len()
    ));

    for side in ["blue_team", "red_team"] {
        let Some(Value::Array(players)) = record.get(side) else {
            continue;
        };
        for player in players {
            let champion = player
                .get("champion")
                .and_then(Value::as_str)
                .unwrap_or("<none>");
            let Some(Value::Array(items)) = player.get("items") else {
                continue;
            };
            let decoded: Vec<String> = items
                .iter()
                .map(|item| item_key(item, index).unwrap_or_else(|| "<empty>".to_string()))
                .collect();

            if !crate::build_config::has_pins(champion) {
                diag(&format!("  {champion}: {items:?} -> {decoded:?}"));
                continue;
            }

            // Slot order is not guaranteed to line up, so this compares as sets:
            // the question is whether the ids decode to the pinned items at all,
            // not whether they arrived in the pinned order.
            let pinned: Vec<String> = (0..4)
                .filter_map(|slot| crate::build_config::pinned_key(champion, slot))
                .collect();
            let hit = decoded.iter().filter(|key| pinned.contains(key)).count();
            let verdict = if hit > 0 { "MATCH" } else { "MISMATCH" };
            diag(&format!(
                "  {verdict} {champion}: {items:?} -> {decoded:?}; pinned {pinned:?}"
            ));
        }
    }
}

/// Lines [`diag`] will write before it goes quiet.
const DIAG_LINES: u32 = 40;

/// Appends one line to `item-stats.log` beside the DLL, up to [`DIAG_LINES`].
///
/// Shared with [`crate::item_stats_ui`], so the UI half's account of finding the
/// screen and the data half's account of reading a record land in one file in
/// the order they happened — which is the only way to tell "the tab never
/// spawned" from "the tab spawned over an empty table".
///
/// There is no `log` to use instead: `StableHost` is valid only inside the
/// callback that receives it and extensions never get one. The cap is what keeps
/// this from becoming the per-frame spam the strategy-screen probe turned into.
pub(crate) fn diag(line: &str) {
    use std::io::Write;
    use std::sync::atomic::{AtomicU32, Ordering};

    static WRITTEN: AtomicU32 = AtomicU32::new(0);
    if WRITTEN.fetch_add(1, Ordering::Relaxed) >= DIAG_LINES {
        return;
    }
    let Some(path) = crate::config::dll_dir().map(|dir| dir.join("item-stats.log")) else {
        return;
    };
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

// -- item catalog -----------------------------------------------------------

/// Display name and sprite frame for one item key.
#[derive(Clone, Default)]
pub(crate) struct ItemInfo {
    pub name: String,
    /// `rect_tag` into the item sheet, or `None` for an item with no art.
    pub frame: Option<String>,
}

static CATALOG: Mutex<Option<BTreeMap<String, ItemInfo>>> = Mutex::new(None);

/// The game's 30 base items, in the order the item table declares them.
///
/// This is the first half of the id space a record's numeric `items` entry
/// indexes, and it has to be written down because the API will not give it: the
/// host serialises `ItemSetting` with its keys **sorted**, so what
/// `setting_get_json` hands back is alphabetical and its order says nothing
/// about ids. The order below is the one `asset/base/setting/item_setting`
/// declares (readable as plain text inside `bundle.game_data`, and the same
/// order the exe's own key blob lists).
///
/// A game update that adds or reorders base items invalidates this. That shows
/// up as rows named after the wrong item rather than as an error, which is why
/// [`diagnose`] cross-checks it against the pinned builds on every run.
const VANILLA_ORDER: [&str; 30] = [
    "iron_blade",
    "soldiers_longsword",
    "ruinous_blade",
    "conquerors_greatsword",
    "warlords_final_judgement",
    "dagger",
    "wind_dagger",
    "twin_stormblade",
    "thunderclaw",
    "storm_sovereign",
    "steel_armor",
    "gatekeepers_armor",
    "black_knights_heavy_plate",
    "eternal_iron_plate",
    "impregnable_fortress",
    "mystic_cloak",
    "night_hood",
    "dusk_raven",
    "souls_edge",
    "veil_of_annihilation",
    "arcane_crystal",
    "spirit_crystal",
    "staff_of_rapture",
    "angels_fang",
    "prophet_of_the_abyss",
    "vital_orb",
    "hardened_heart",
    "ring_of_reincarnation",
    "hourglass_of_eternity",
    "giants_horn_shard",
];

/// This mod's item keys, in the order `init` registered them — the second half
/// of the id space.
///
/// Recorded as they are registered rather than read back from anywhere, because
/// there is nowhere to read it from: `ItemSetting` contains only the 30 base
/// items (confirmed — the document parses to exactly 30 entries), so a mod's
/// items are absent from the one document that describes items at all.
static REGISTERED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Notes one item key at registration time. Called from `init` for every item
/// the mod adds, in call order.
pub(crate) fn note_registered(key: &str) {
    if let Ok(mut keys) = REGISTERED.lock() {
        keys.push(key.to_string());
    }
}

/// The id space: base items, then this mod's.
///
/// # How this was established
///
/// Records store items as bare numbers (`"items":[24,24,24]`), and three
/// orderings were possible. The observed data rules out the other two:
///
/// * **Alphabetical over everything** would scatter cheap components across the
///   whole range. Instead every id under 30 carries a huge pick count (386, 380,
///   446 over 121 matches) and everything above 120 carries a small one, which
///   is what "base items are a block at the front, and they are the low tiers
///   every build routes through" looks like.
/// * **Settings-document order** cannot be it either: that document arrives
///   sorted and holds only 30 of the 183 items, so it cannot describe ids up to
///   179 at all. Shipping it produced exactly that — 30 named rows and the rest
///   showing `#134`.
///
/// It is still an inference, so [`diagnose`] checks it against ground truth the
/// mod already has: `item-builds.json` pins specific keys to specific champions,
/// and the hook forces them, so a pinned champion's record ids must decode to
/// its pinned keys. A mismatch is printed, loudly, rather than assumed away.
fn index_table() -> Vec<String> {
    let mut table: Vec<String> = VANILLA_ORDER.iter().map(|key| key.to_string()).collect();
    if let Ok(keys) = REGISTERED.lock() {
        table.extend(keys.iter().cloned());
    }
    table
}

/// Builds the item table from the settings document, once.
///
/// Unlike the build editor's list this keeps **every** item, not just finals: a
/// match record holds whatever a player was carrying when it ended, and a board
/// full of half-finished components is a normal way for a game to end. Filtering
/// them out here would silently drop games from the totals.
///
/// Split from [`catalog`] and called from `post_update` because
/// `setting_get_json` does **not** work inside a UI click handler — the trait
/// notes only ui/asset calls are live there, and the build editor already paid
/// for learning that. Building it lazily from the first repaint would mean the
/// first repaint is the one inside the click that opens the tab, which would
/// cache nothing, draw every row under its raw key, and then have no reason to
/// repaint again once the scan had finished.
pub(crate) fn prime_catalog(ctx: &StableClient<'_>) {
    if CATALOG
        .lock()
        .map(|cached| cached.is_some())
        .unwrap_or(false)
    {
        return;
    }

    let mut out = BTreeMap::new();
    if let Some(json) = ctx.setting_get_json(SettingTargetV1::ItemSetting, "") {
        if let Ok(Value::Object(root)) = serde_json::from_str::<Value>(&json) {
            collect_items(ctx, &root, 0, &mut out);
        }
    }

    // An empty result is not cached: it means the document could not be read
    // this frame, which a later frame may well manage.
    if out.is_empty() {
        return;
    }

    // The settings document holds only the base items, so every one of this
    // mod's has to be described from what the mod itself knows: its name from
    // the merged item text, its sprite frame from its own key.
    //
    // The key *is* the frame — `StableItem::icon` returns it verbatim, and all
    // 153 registered keys have a byte-identical tag in the sheet. Resolving it
    // through `item_catalog::icon_frame` instead is wrong here, because that
    // takes a base slug: it strips `radiant_`, and the sheet stores the plain
    // and radiant art of an item as two separate tags, with the gold border
    // being what makes a radiant look radiant. So every one of the mod's 66
    // radiant items drew its own non-radiant twin.
    if let Ok(keys) = REGISTERED.lock() {
        for key in keys.iter() {
            out.entry(key.clone()).or_insert_with(|| ItemInfo {
                name: display_name(ctx, key),
                frame: Some(key.clone()),
            });
        }
    }

    let table = index_table();
    diag(&format!(
        "catalog: {} named, {} ids ({} base + {} mod); 4={:?} 24={:?} 134={:?}",
        out.len(),
        table.len(),
        VANILLA_ORDER.len(),
        table.len() - VANILLA_ORDER.len(),
        table.get(4),
        table.get(24),
        table.get(134),
    ));
    if let Ok(mut cached) = CATALOG.lock() {
        *cached = Some(out);
    }
}

/// The catalog as [`prime_catalog`] last left it, empty until it succeeds.
/// Takes no ctx, so it is safe to call from a click handler.
pub(crate) fn catalog() -> BTreeMap<String, ItemInfo> {
    CATALOG
        .lock()
        .ok()
        .and_then(|cached| cached.clone())
        .unwrap_or_default()
}

fn collect_items(
    ctx: &StableClient<'_>,
    map: &serde_json::Map<String, Value>,
    depth: usize,
    out: &mut BTreeMap<String, ItemInfo>,
) {
    for (key, value) in map {
        let Some(object) = value.as_object() else {
            continue;
        };
        let is_item = object.contains_key("next_tier")
            || object.contains_key("tier")
            || object.contains_key("price");
        if !is_item {
            // Two levels, not one: mod items sit under a per-mod bucket
            // (`mod_items.riot_items_tfm2.collector`).
            if depth < 2 {
                collect_items(ctx, object, depth + 1, out);
            }
            continue;
        }
        out.insert(
            key.clone(),
            ItemInfo {
                name: display_name(ctx, key),
                frame: icon_frame(object, key),
            },
        );
    }
}

/// The item's own name, tier word included.
///
/// The build editor deliberately strips "Radiant" because every row in its list
/// is a final and the prefix distinguishes nothing. Here it distinguishes a
/// great deal: `infinity_edge` and `radiant_infinity_edge` are separate keys
/// with separate win rates, and two rows reading "Infinity Edge" would be a
/// table nobody could act on.
fn display_name(ctx: &StableClient<'_>, key: &str) -> String {
    ctx.i18n(&format!("#asset/base/text/item?{key}.name"))
        .filter(|name| !name.is_empty() && !name.starts_with('#'))
        .unwrap_or_else(|| key.to_string())
}

/// The frame a base item draws from the (mod-overridden) item sheet.
///
/// The settings document's own `icon` is the authority: base items carry a
/// tier-slot name like `t5_0`, which the mod's sheet fills with its reskin of
/// that item — gold border included, since the game's tier-5 items are the ones
/// this mod presents as radiant.
///
/// The fallback is the key itself, never `base_slug`. Stripping `radiant_` picks
/// the plain twin of an item whose radiant art is a separate tag, which is
/// precisely the bug that made 66 items draw as non-radiant.
fn icon_frame(object: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    object
        .get("icon")
        .and_then(Value::as_str)
        .filter(|icon| !icon.is_empty())
        .map(str::to_string)
        .or_else(|| Some(key.to_string()))
}

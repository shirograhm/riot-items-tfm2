//! Per-item win/loss totals over the matches [`crate::item_stats::sim`] captured.
//!
//! # Where the numbers come from
//!
//! The loadouts come from the simulation, not from the match record. The
//! record's per-player `items` is the build the game *assigned*, not what the
//! champion finished holding — proven by its shape: every player carried exactly
//! 3 or 4 items, never fewer, and never a component. `StablePlayer::item_keys`
//! on the last tick is the real end state, and it hands back real item keys
//! rather than numbers indexing a table the API does not expose.
//!
//! So a match is counted once, when a record vouches for it, and the record
//! answers only what the simulation cannot: which patch it was played on, and
//! whether it was a league match at all.
//!
//! # What the record is used for
//!
//! Only two fields: `version`, which is the patch the match was played on and
//! what the patch filter groups by, and `seed`, which joins it to the loadout
//! [`crate::item_stats::sim`] captured from the simulation.
//!
//! Its per-player `items` is deliberately **not** read. That field holds the
//! build the game *assigned*, not what was finished with, and it is stored as
//! bare numbers indexing a table the API does not expose — which previously
//! meant inferring the whole id space from the order items are declared and
//! registered in. Taking the loadout from the sim instead makes both problems
//! disappear at once: real keys, and the real end state.
//!
//! # The other leagues
//!
//! The seed only joins the player's own league: the records of a set played in
//! another one do not seem to carry the simulation's seed (see
//! [`crate::item_stats::sim`]). Those captures are placed by the server, which
//! reads the set's replay record off its match ([`place_captures`]) and takes
//! the same one field from it, `version`. Either way a capture is counted once,
//! and only when a replay record stands behind it.
//!
//! # Why the totals are kept, and not the matches
//!
//! These counters **are** the stored history: a match is folded in once and its
//! loadouts are dropped. The alternative, keeping every match and re-folding them
//! whenever one was added, cost two passes over the whole save — the fold itself
//! and the write that followed it — and both got slower the longer the save was
//! played, which is exactly backwards for a feature that only becomes useful once
//! a lot has been played. What is stored is now proportional to the number of
//! items, patches and lanes, all of which are fixed.
//!
//! # Where they are stored
//!
//! In the save file, under this mod's own namespace — see [`sync`]. They used to
//! be a `totals.json` in `item_stats/<save>/` beside the DLL, which needed the
//! mod to work out *which save is this* on its own; it had no answer, so it
//! fingerprinted saves by their match seeds and named folders after the team.
//! That machinery is gone: data kept inside the save is tied to it by
//! construction, and a save loaded from an earlier point now shows the numbers it
//! had then instead of a future it was rolled back from.
//!
//! Two things follow, both worth knowing. The table reaches disk only when the
//! player saves, so quitting without saving drops the session's matches along
//! with everything else that session. And the namespace answers empty on some
//! frames while it is already writable, which is why nothing folds or writes
//! before a load has been confirmed — see [`LOAD_GRACE`].
//!
//! The trade is that nothing can be recomputed. A column added later starts
//! empty and fills from new matches, where a stored history could have answered
//! it at once — that is how the first-item rate was added retroactively. Worth
//! knowing before adding the next column.
//!
//! Records still cannot be trusted to persist or to keep their ids: the count was
//! seen going 126 -> 28 -> 77 inside one session, so the game prunes and recycles
//! them freely. Re-reading every record on every pass stays safe because a record
//! contributes no numbers, only a patch, and a match can be counted only once —
//! [`crate::item_stats::sim::take`] hands each one over exactly once and remembers
//! that it did.

pub(crate) mod sim;
pub(crate) mod toolbox_tab;
pub(crate) mod ui;

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use mod_api_stable::*;
use serde_json::Value;

/// Records read per [`pump`] call.
///
/// Two fields are wanted from each, but the whole record still crosses the ABI
/// and is parsed — ten players' match statistics included — and this runs on the
/// UI thread. Measured at roughly half a millisecond a record (2026-09-27), so a
/// batch of 24 was a 12-14 ms frame. Most passes now read only the records that
/// are new since the last one (see [`Aggregate::read`]); this bounds the ones
/// that re-read everything.
const CHUNK: usize = 8;

#[derive(Clone, Copy, Default)]
pub(crate) struct Totals {
    pub games: u32,
    pub wins: u32,
    /// Games where this item was the one in the player's **first** item slot.
    ///
    /// "First" is slot order: `StablePlayer::item_keys` enumerates the player's
    /// items by index, and a champion's items are appended as they are
    /// completed, so slot 0 is the item they finished first. That is the closest
    /// thing to a purchase order the simulation exposes — there is no timestamp
    /// on an item — and it is the same order the assigned build is written in.
    pub firsts: u32,
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

    /// Share of this item's buys where it was bought first, in percent.
    ///
    /// Same `None`-for-no-games rule as [`Totals::win_rate`], and for the same
    /// reason: an item nobody has bought has no first-item rate, and printing
    /// 0.0% for it would claim it is never rushed.
    pub fn first_rate(&self) -> Option<f64> {
        (self.games > 0).then(|| self.firsts as f64 * 100.0 / self.games as f64)
    }

    /// Times this item was built per match, in percent: `games` counts one per
    /// player who finished with it, so 200% means two players a match on
    /// average. `None` when no match has been counted, which is no rate at all
    /// rather than 0%.
    ///
    /// `matches` is every match in the patch filter, whatever the lane filter —
    /// with a lane picked this reads "built in that lane, per match".
    pub fn play_rate(&self, matches: u32) -> Option<f64> {
        (matches > 0).then(|| self.games as f64 * 100.0 / matches as f64)
    }
}

#[derive(Default)]
struct Aggregate {
    /// Record ids still to read for their patch, newest first.
    pending: Vec<usize>,
    /// What each record read so far said: its patch and seed, or `None` for one
    /// that cannot be placed.
    ///
    /// A pass reads only the ids missing from here and matches the rest from
    /// memory. Re-reading every record on every pass was the cost that remained
    /// once passes stopped running every frame: most captures never get a
    /// record, so while any waits, each finished match set off a pass through
    /// the whole list. An id that leaves the list is forgotten, since it may
    /// come back holding a different match. One that is pruned and re-used
    /// between two looks at the list would read stale, which is what the
    /// periodic verify pass is for (see [`due_pass`]).
    read: HashMap<usize, Option<(String, u64)>>,
    /// The queued pass has not yet matched captures against [`Self::read`].
    rematch: bool,
    /// Whether the save's counters have been read into this table yet.
    ///
    /// Nothing may be written back before this is true. A read can come back
    /// empty on a frame where `save_can_write` already answers true, and folding
    /// into an empty table and then saving it would overwrite the save's real
    /// history — see [`sync`].
    loaded: bool,
    /// Patch -> (lane, item) -> totals.
    ///
    /// Keyed by patch first so the filter is a map lookup rather than a re-scan:
    /// picking one reads its submap, and "All" merges them. The record's
    /// `version` is what a patch is here — it sits beside `seed` in the replay
    /// data, which is what a replay would need to reproduce the balance a match
    /// was played under.
    ///
    /// The lane rides in the inner key rather than adding a third level of map,
    /// so both filters are one pass over the same entries and "All" on either
    /// axis is the same merge with one term dropped.
    counts: BTreeMap<String, BTreeMap<(Option<usize>, String), Totals>>,
    /// Patch -> (lane, item) -> champion -> times that champion was holding it.
    /// Feeds the "purchased on" column, which is the top few of these by count.
    champions: BTreeMap<String, BTreeMap<(Option<usize>, String), BTreeMap<String, u32>>>,
    /// Patch -> captured matches.
    matches: BTreeMap<String, u32>,
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
    /// Records still to read. Non-zero means a patch pass is in flight.
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

/// Queues a patch-backfill pass over the match records.
///
/// Record ids are **reused**: the count was observed going 126 -> 28 -> 77
/// inside one session, so the game prunes and recycles them, and "id 12 is
/// already scanned" is not a fact that stays true. That is why an id that
/// leaves the list is forgotten here, and why [`due_pass`] still re-reads
/// everything now and then.
///
/// Re-reading is harmless because a record is read for two fields and nothing
/// is folded from it — the totals come from the captures, which are
/// deduplicated by match seed and cannot be double counted however often a
/// record is re-read.
pub(crate) fn sweep(ctx: &StableClient<'_>) {
    let ids = crate::perf::time(crate::perf::Section::RecordIds, || {
        ctx.record_ids(RecordKindV1::MatchReplay)
    });
    queue_pass(&ids, false);
}

/// [`sweep`] over ids already in hand. `verify` re-reads the records already
/// in [`Aggregate::read`] instead of trusting them.
fn queue_pass(ids: &[usize], verify: bool) {
    if ids.is_empty() {
        return;
    }
    let live: HashSet<usize> = ids.iter().copied().collect();
    let _ = with_agg(|agg| {
        if verify {
            agg.read.clear();
        } else {
            agg.read.retain(|id, _| live.contains(id));
        }
        agg.pending = ids
            .iter()
            .rev()
            .copied()
            .filter(|id| !agg.read.contains_key(id))
            .collect();
        agg.rematch = true;
    });
}

/// Frames between looks at the record list while captures are waiting.
const SWEEP_CHECK_FRAMES: u32 = 30;

/// Frames between passes that re-read every record. A new record can take an id
/// a pruned one left behind between two looks at the list, and then the one
/// [`Aggregate::read`] remembers for it is stale. This catches that, rarely
/// enough that the reads it costs do not matter.
///
/// A verify pass reads every record, and a long save holds thousands (~2000
/// measured, ~0.3 ms each), so it runs every five minutes or so of frames.
const SWEEP_VERIFY_FRAMES: u32 = 18_000;

/// What the records looked like when the last pass was queued.
#[derive(Default)]
struct SweepMark {
    /// [`crate::item_stats::sim::captures`] at the time.
    captures: u64,
    /// The record ids at the time.
    ids: Vec<usize>,
    /// Frames since the id list was last looked at.
    since_check: u32,
    /// Frames since the last verify pass was queued.
    since_verify: u32,
}

static SWEEP_MARK: Mutex<Option<SweepMark>> = Mutex::new(None);

/// The record ids to pass over when a pass could find something the last one
/// did not, or `None` when it could not.
///
/// # Why passes are not queued every frame
///
/// They were, whenever any capture was waiting (measured 2026-09-27). A capture
/// waits until its record is written, which happens when its game day is
/// committed and can be many minutes later, and some never get one at all — so
/// from the first capture on, every frame queued a fresh pass and read the
/// newest [`CHUNK`] records in full: ~10 ms a frame on the main thread, which
/// halved the frame rate for the rest of the session, and took it to ~24 fps
/// with the statistics screen open, whose own pump read the same pass.
///
/// A pass can only find something new if a capture has arrived since the last
/// one, or the record list has changed. The first is an atomic read; the second
/// is one `record_ids` call every [`SWEEP_CHECK_FRAMES`].
///
/// Returns the ids and whether the pass is a verify pass.
fn due_pass(ctx: &StableClient<'_>) -> Option<(Vec<usize>, bool)> {
    let captures = crate::item_stats::sim::captures();
    let mut guard = SWEEP_MARK.lock().ok()?;
    let mark = guard.get_or_insert_with(SweepMark::default);
    mark.since_check = mark.since_check.saturating_add(1);
    mark.since_verify = mark.since_verify.saturating_add(1);
    let new_capture = captures != mark.captures;
    let verify = mark.since_verify >= SWEEP_VERIFY_FRAMES;
    if !new_capture && !verify && mark.since_check < SWEEP_CHECK_FRAMES {
        return None;
    }
    mark.since_check = 0;
    let ids = crate::perf::time(crate::perf::Section::RecordIds, || {
        ctx.record_ids(RecordKindV1::MatchReplay)
    });
    if !new_capture && !verify && ids == mark.ids {
        return None;
    }
    mark.captures = captures;
    mark.ids.clone_from(&ids);
    if verify {
        mark.since_verify = 0;
    }
    Some((ids, verify))
}

/// Reads a bounded batch of records to backfill patches, then re-folds the
/// totals if the captures have changed.
///
/// Records no longer contribute any numbers. They answer one question — which
/// patch was this match played on — and the answer is written onto the capture
/// so it survives the record being pruned.
pub(crate) fn pump(ctx: &StableClient<'_>) -> bool {
    // Nothing folds into a table that has not been read back from the save yet.
    // The fold would be overwritten by the load that follows it, and the capture
    // it consumed is dropped as it is handed over — so the match would be lost
    // rather than merely delayed. `sync` loads first for the same reason; this
    // guard is for the statistics screen, which pumps on its own.
    if !with_agg(|agg| agg.loaded).unwrap_or(false) {
        return false;
    }
    // First the captures the server has found the patch of
    // ([`place_captures`]): there is no record to read for those.
    let mut folded = false;
    for (patch, players) in crate::item_stats::sim::take_placed() {
        fold(&patch, &players);
        folded = true;
    }
    // Every capture has been matched: the rest of the pass would read records
    // for nothing.
    if crate::item_stats::sim::pending() == 0 {
        let _ = with_agg(|agg| {
            agg.pending.clear();
            agg.rematch = false;
        });
        if folded {
            DIRTY.store(true, Ordering::Relaxed);
        }
        return folded;
    }

    // Then the records already read, which need no reading to be matched: a
    // capture often arrives after its record was read for an earlier pass.
    let known: Vec<(String, u64)> = with_agg(|agg| {
        if std::mem::take(&mut agg.rematch) {
            agg.read.values().flatten().cloned().collect()
        } else {
            Vec::new()
        }
    })
    .unwrap_or_default();
    for (patch, seed) in &known {
        if let Some(players) = crate::item_stats::sim::take(*seed) {
            fold(patch, &players);
            folded = true;
        }
    }

    let batch = with_agg(|agg| {
        let take = CHUNK.min(agg.pending.len());
        agg.pending.drain(..take).collect::<Vec<_>>()
    })
    .unwrap_or_default();

    for id in &batch {
        let entry = read_record(ctx, *id);
        let _ = with_agg(|agg| agg.read.insert(*id, entry.clone()));
        let Some((patch, seed)) = entry else {
            continue;
        };
        // A seed with no capture waiting is either a match simmed before
        // capturing began — the loadout it would need does not exist, and the
        // record carries no usable substitute — or one already counted, which
        // `take` declines a second time. Both are nothing to do.
        let Some(players) = crate::item_stats::sim::take(seed) else {
            continue;
        };
        fold(&patch, &players);
        folded = true;
    }

    let finished = with_agg(|agg| agg.pending.is_empty()).unwrap_or(false);

    if folded {
        DIRTY.store(true, Ordering::Relaxed);
        return true;
    }

    !batch.is_empty() && finished
}

/// Folds one vouched match into the running totals.
///
/// Called once per match, ever. The counters it adds to are the stored history,
/// so nothing is recomputed and nothing is walked twice — which is the whole
/// point of keeping numbers rather than matches.
fn fold(patch: &str, players: &[crate::item_stats::sim::CapturedPlayer]) {
    FOLDS.fetch_add(1, Ordering::Relaxed);
    let _ = with_agg(|agg| {
        *agg.matches.entry(patch.to_string()).or_default() += 1;

        let per_item = agg.counts.entry(patch.to_string()).or_default();
        for player in players {
            for (slot, key) in player.items.iter().enumerate() {
                let entry = per_item.entry((player.lane, key.clone())).or_default();
                entry.games += 1;
                entry.wins += u32::from(player.won);
                // Slot order is completion order, so the first slot is the item
                // this player rushed. There is no timestamp on an item; this is
                // the only purchase order the simulation exposes.
                entry.firsts += u32::from(slot == 0);
            }
        }

        let per_champion = agg.champions.entry(patch.to_string()).or_default();
        for player in players {
            // A player whose champion could not be read still counts toward the
            // item's games and wins — the loadout is real — but it must not be
            // tallied as a champion. It used to be, under the empty key, and
            // `top_champions` then ranked it like any other name: on an item
            // bought mostly by champions that were dead at the final tick, the
            // blank outranked every real name and took a column slot that then
            // drew nothing. That is the "played, but no portraits" case.
            if player.champion.is_empty() {
                continue;
            }
            for key in &player.items {
                *per_champion
                    .entry((player.lane, key.clone()))
                    .or_default()
                    .entry(player.champion.clone())
                    .or_default() += 1;
            }
        }
    });
}

/// Matches folded since the game started.
static FOLDS: AtomicU64 = AtomicU64::new(0);

/// How many matches have been folded since the game started. The statistics
/// screen repaints when this moves: most folds happen in [`sync`], not in the
/// screen's own [`pump`], which is all it used to hear from.
pub(crate) fn folds() -> u64 {
    FOLDS.load(Ordering::Relaxed)
}

/// Sets the server looks up in one pass. With [`PLACE_EVERY`] that is 32 a
/// second, so a game day of every league's sets is placed within a few
/// seconds of being recorded, at two small reads a set.
const PLACE_BATCH: usize = 8;

/// The least time between two passes. By the clock, since how often the
/// server ticks is not something this can count on.
const PLACE_EVERY: Duration = Duration::from_millis(250);

/// Lines the test log gets for each way a look-up can end. A set whose record
/// never comes is asked about again on every round.
const PLACINGS_TO_LOG: u32 = 8;

/// What the server's records said about one captured set.
struct Placing {
    /// The match record's `replays`, as the server gave it.
    listed: Option<String>,
    /// The set's replay record.
    replay: Option<u64>,
    /// That record's `version`.
    patch: Option<String>,
}

/// Looks one captured set up in the server's records.
///
/// `RecordKindV1::Match` is the whole match table on the server, and a match
/// record's `replays` holds the replay id of each of its sets (both from the
/// stable API's own notes). A set that has not been recorded yet is simply
/// not in the list, and is asked about again on a later pass.
fn look_up(ctx: &StableServerCtx<'_>, fixture: &sim::Fixture) -> Placing {
    let version = |replay: u64| {
        ctx.record_get_string(RecordKindV1::MatchReplay, replay as usize, "version")
            .filter(|version| !version.is_empty())
    };
    // The simulation's own answer first, where it has one.
    if let Some(replay) = fixture.replay {
        if let Some(patch) = version(replay) {
            return Placing {
                listed: None,
                replay: Some(replay),
                patch: Some(patch),
            };
        }
    }
    let listed = fixture.set.and_then(|(match_id, _)| {
        ctx.record_get_json(RecordKindV1::Match, match_id as usize, "replays")
    });
    let replay = fixture
        .set
        .zip(listed.as_deref())
        .and_then(|((_, set_index), listed)| {
            serde_json::from_str::<Value>(listed)
                .ok()?
                .get(set_index as usize)?
                .as_u64()
        });
    let patch = replay.and_then(version);
    Placing {
        listed,
        replay,
        patch,
    }
}

/// Finds the patch of the captures no record's seed asks for, which is what
/// the sets played outside the player's own league have been, so that
/// [`pump`] can fold them.
///
/// Called from the server's management tick, because the server is where
/// `RecordKindV1::Match` is the whole match table: the client is given views
/// of it by category. A few sets a pass, four passes a second at most, and on
/// the other ticks nothing but a lock and a clock read. No capture is given up
/// on, for the reason none is expired by age: a set's record is written when
/// its game day is committed, however long that takes.
pub(crate) fn place_captures(ctx: &StableServerCtx<'_>) {
    static LAST_PASS: Mutex<Option<Instant>> = Mutex::new(None);
    static LOGGED: [AtomicU32; 4] = [
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
        AtomicU32::new(0),
    ];
    {
        let Ok(mut last) = LAST_PASS.lock() else {
            return;
        };
        if last.is_some_and(|at| at.elapsed() < PLACE_EVERY) {
            return;
        }
        *last = Some(Instant::now());
    }
    for (seed, fixture) in sim::unplaced(PLACE_BATCH) {
        let placing = look_up(ctx, &fixture);
        let (outcome, said) = match (&placing.patch, placing.replay, &placing.listed) {
            (Some(_), _, _) => (0, "placed"),
            (None, Some(_), _) => (1, "its replay record names no version"),
            (None, None, Some(_)) => (2, "the match does not list this set"),
            (None, None, None) => (3, "no match record"),
        };
        if LOGGED[outcome].load(Ordering::Relaxed) < PLACINGS_TO_LOG {
            LOGGED[outcome].fetch_add(1, Ordering::Relaxed);
            crate::match_builds::log("placing", || {
                format!(
                    "{said}: set {:?}, replay named by the simulation {:?}, replays {:?}, replay {:?}, version {:?}",
                    fixture.set, fixture.replay, placing.listed, placing.replay, placing.patch
                )
            });
        }
        if let (Some(replay), Some(patch)) = (placing.replay, placing.patch) {
            sim::place(seed, replay, patch);
        }
    }
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

/// The current table, in key order, for one patch and lane or for all of them.
///
/// Deliberately *not* sorted for display: the column the player picked can be
/// the item's name, which lives in the catalog, so ordering is the UI's job.
/// Key order makes it a stable starting point, which is what keeps equal rows
/// from reshuffling between repaints mid-scan.
pub(crate) fn snapshot(patch: Option<&str>, lane: Option<usize>) -> Snapshot {
    with_agg(|agg| {
        // One patch reads its own submap; "All" merges them. Merging here rather
        // than keeping a second running total means there is one set of numbers
        // to be wrong, and it costs a walk of at most items x patches.
        let mut counts: BTreeMap<String, Totals> = BTreeMap::new();
        let mut champions: BTreeMap<String, BTreeMap<String, u32>> = BTreeMap::new();
        let mut matches = 0;

        // A lane filter keeps only the entries recorded under it. A player the
        // host would not give a lane for is `None`, which no lane selection
        // matches — the same rule the category filter follows for an item with
        // no role, and the reason such a player still shows up under "All".
        let wanted_lane = |recorded: Option<usize>| lane.is_none_or(|want| recorded == Some(want));

        for (key, per_item) in &agg.counts {
            if patch.is_some_and(|wanted| wanted != key.as_str()) {
                continue;
            }
            for ((recorded, item), totals) in per_item {
                if !wanted_lane(*recorded) {
                    continue;
                }
                let entry = counts.entry(item.clone()).or_default();
                entry.games += totals.games;
                entry.wins += totals.wins;
                entry.firsts += totals.firsts;
            }
        }
        for (key, per_item) in &agg.champions {
            if patch.is_some_and(|wanted| wanted != key.as_str()) {
                continue;
            }
            for ((recorded, item), tally) in per_item {
                if !wanted_lane(*recorded) {
                    continue;
                }
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
            pending: agg.pending.len(),
            champions: top_champions(&champions),
        }
    })
    .unwrap_or(Snapshot {
        rows: Vec::new(),
        matches: 0,
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
/// Two fields are wanted — the patch and the join key — but the whole record is
/// fetched in one call where the host allows it, because one round trip beats
/// two and the parse is the same either way.
fn read_record(ctx: &StableClient<'_>, id: usize) -> Option<(String, u64)> {
    // Measured 2026-09-27: a named read ("seed") costs about half a full read,
    // so the two named reads this needs would cost what one full read does.
    // The full read stays first.
    let full = crate::perf::time(crate::perf::Section::RecordRead, || {
        ctx.record_get_json(RecordKindV1::MatchReplay, id, "")
            .and_then(|json| serde_json::from_str::<Value>(&json).ok())
    });
    let record = match full {
        Some(record) => record,
        // Older hosts, or a path grammar that does not accept the empty path for
        // this record kind. Named reads say the same thing, and are reassembled
        // into the same shape so that everything below has one case to handle
        // rather than two.
        None => {
            let field = |name: &str| {
                ctx.record_get_json(RecordKindV1::MatchReplay, id, name)
                    .and_then(|json| serde_json::from_str::<Value>(&json).ok())
            };
            serde_json::json!({
                "version": field("version"),
                "seed": field("seed"),
            })
        }
    };
    // A league match always names its version, so one that does not is not a
    // record this table can place — skipped rather than filed under a catch-all
    // bucket that would only ever collect things that should not be there.
    let patch = record
        .get("version")
        .and_then(Value::as_str)
        .filter(|version| !version.is_empty())?
        .to_string();

    // The seed joins this record to the loadout the simulation captured for the
    // same match. `serde_json` keeps integers as `u64`, so a match seed survives
    // the round trip that an `f64` would round off.
    let seed = record.get("seed").and_then(Value::as_u64)?;
    Some((patch, seed))
}

// -- item catalog -----------------------------------------------------------

/// Display name and sprite frame for one item key.
#[derive(Clone, Default)]
pub(crate) struct ItemInfo {
    pub name: String,
    /// `rect_tag` into the item sheet, or `None` for an item with no art.
    pub frame: Option<String>,
    /// 0..=4, which the tier filter reads as starter/basic/epic/legendary/
    /// radiant. `None` for an item neither the settings document nor this mod
    /// describes.
    pub tier: Option<usize>,
}

static CATALOG: Mutex<Option<BTreeMap<String, ItemInfo>>> = Mutex::new(None);

/// This mod's item keys, in the order `init` registered them — the second half
/// of the id space.
///
/// Recorded as they are registered rather than read back from anywhere, because
/// there is nowhere to read it from: `ItemSetting` contains only the 30 base
/// items (confirmed — the document parses to exactly 30 entries), so a mod's
/// items are absent from the one document that describes items at all.
static REGISTERED: Mutex<Vec<(String, usize)>> = Mutex::new(Vec::new());

/// Notes one item key and its tier at registration time. Called from `init` for
/// every item the mod adds.
///
/// The tier has to come from here because the settings document describes only
/// the game's own items — a mod's are absent from the one place items are
/// described, so `StableItem::tier` at registration is the only source.
pub(crate) fn note_registered(key: &str, tier: usize) {
    if let Ok(mut keys) = REGISTERED.lock() {
        keys.push((key.to_string(), tier));
    }
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
            collect_items(ctx, &root, &mut out);
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
        for (key, tier) in keys.iter() {
            out.entry(key.clone()).or_insert_with(|| ItemInfo {
                name: display_name(ctx, key),
                frame: Some(key.clone()),
                tier: Some(*tier),
            });
        }
    }

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

/// One of the game's own thirty items, by its key in the settings document
/// (the mod draws it as Radiant Luden's Tempest). A settings document that
/// describes it has the game's items in it, not only the ones mods registered.
pub(crate) const A_GAME_ITEM: &str = "prophet_of_the_abyss";

/// The game's finals that give attack, attack speed or ability power (the
/// mod draws them as Radiant Bloodthirster, Phantom Dancer and Luden's
/// Tempest): what [`prime_item_traits`] reports its readings of in the test
/// log, since the rules were found holding blank records of them.
const GAME_DAMAGE_FINALS: [&str; 3] = ["warlords_final_judgement", "storm_sovereign", A_GAME_ITEM];

/// Frames between two tries of [`prime_item_traits`] while the settings
/// document cannot be read: half a second at 60 frames a second.
const TRAITS_RETRY_FRAMES: u32 = 30;

/// Hands the stats of the game's own items to the Smart Builds rules, once.
///
/// Called every client frame, on every screen, until it succeeds. The rules
/// learn this mod's items as `init` registers them, but the game's are
/// described only in the settings document, and until 2026-10-07 that was read
/// by [`prime_catalog`] alone, which runs on the statistics screen. A player
/// who had not opened that screen since launching the game played with rules
/// that knew nothing about the thirty vanilla items: none gave ability power,
/// attack or crit as far as rule 5 and the crit cap could tell. That is how a
/// Hunter and a Dual Blader, both attack-damage champions, came to buy Radiant
/// Luden's Tempest as their 6th item.
///
/// Its own function rather than an earlier [`prime_catalog`]: that one also
/// caches every item's display name, which must not happen on a screen where
/// the item text may not be loaded yet.
pub(crate) fn prime_item_traits(ctx: &StableClient<'_>) {
    static PRIMED: AtomicBool = AtomicBool::new(false);
    static FRAME: AtomicU32 = AtomicU32::new(0);
    if PRIMED.load(Ordering::Relaxed)
        || FRAME.fetch_add(1, Ordering::Relaxed) % TRAITS_RETRY_FRAMES != 0
    {
        return;
    }
    let Some(json) = ctx.setting_get_json(SettingTargetV1::ItemSetting, "") else {
        return;
    };
    let Ok(Value::Object(root)) = serde_json::from_str::<Value>(&json) else {
        return;
    };
    let mut games = false;
    // For the test log: every object read under the key of one of the game's
    // three damage finals, in the order the document has them.
    let mut read: Vec<String> = Vec::new();
    each_item(
        &root,
        0,
        &mut |key: &str, object: &serde_json::Map<String, Value>| {
            // The document's own object, for the log, then the numbers the
            // game runs with ([`merged_item`]).
            let in_document = object;
            let object = merged_item(key, object);
            let stat = |name: &str| item_stat(&object, name) as i32;
            let (crit, attack, speed, power) = (
                stat("crit_chance"),
                stat("attack"),
                stat("attack_speed_mult"),
                stat("magic_power"),
            );
            crate::smart_builds::note_engine_item(key, crit, attack, speed, power);
            if GAME_DAMAGE_FINALS.contains(&key) {
                read.push(format!(
                    "{key}(document: stat block={} stat={:?} price={:?}; merged: crit={crit} attack={attack} speed={speed} power={power} price={:?})",
                    in_document.contains_key("stat"),
                    in_document.get("stat").map(|stat| {
                        ["crit_chance", "attack", "attack_speed_mult", "magic_power"]
                            .map(|name| stat.get(name).cloned().unwrap_or(Value::Null))
                    }),
                    in_document.get("price"),
                    object.get("price"),
                ));
            }
            // Read, and read with what it gives, in the document's own words
            // and not the merged ones: a pass that found it with no ability
            // power there has only seen the placeholder (see below).
            games |= key == A_GAME_ITEM && item_stat(in_document, "magic_power") > 0;
        },
    );
    crate::match_builds::log("traits", || {
        let buckets = match root.get("mod_items") {
            Some(Value::Object(mods)) => format!("{:?}", mods.keys().take(12).collect::<Vec<_>>()),
            Some(Value::Array(mods)) => format!("an array of {}", mods.len()),
            Some(_) => "neither an object nor an array".to_string(),
            None => "absent".to_string(),
        };
        format!(
            "settled={games}; {} keys at the root; mod_items: {buckets}; {} items in the mod's own file; read: {}",
            root.len(),
            game_item_overrides().len(),
            read.join(" ")
        )
    });
    // Settled only once the document describes the game's own items with
    // real numbers, however long that takes. What it holds before that, as
    // the test log caught it (2026-10-08): all thirty keys from the first
    // frame, each with a stat block of zeros and a price of 0, and an empty
    // `mod_items` array. So the keys being there says nothing. Settling on
    // "the key is in it" settled on those placeholders and recorded blanks,
    // which is how rule 5 came to let Luden's Tempest be an AD champion's
    // 6th item and Bloodthirster an AP one's; settling on "something was
    // described", or after two minutes of tries, had done no better. The
    // game's items no longer wait on this at all (`prime_game_items`). What
    // still does is whatever else the document comes to hold, other mods'
    // items among it, which is why the pass goes on until it is real.
    if games {
        PRIMED.store(true, Ordering::Relaxed);
    }
}

/// The mod's own copy of the game's thirty items as it is on disk:
/// `setting/item_setting.item_setting` beside the DLL, which `apply_config.ps1`
/// writes from the player's config and `mod.override_info` has the game merge
/// over its own. By the name each item has in the settings document, which
/// is not always its key (`iron_blade` calls itself `ironsword`). Read once:
/// the game loads it once too. Empty where it cannot be read.
pub(crate) fn game_item_file() -> &'static serde_json::Map<String, Value> {
    static FILE: OnceLock<serde_json::Map<String, Value>> = OnceLock::new();
    FILE.get_or_init(|| {
        let path = crate::config::mod_dir()
            .join("setting")
            .join("item_setting.item_setting");
        match std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(text.trim_start_matches('\u{feff}')).ok())
        {
            Some(Value::Object(root)) => root,
            _ => serde_json::Map::new(),
        }
    })
}

/// [`game_item_file`] by item key.
fn game_item_overrides() -> &'static HashMap<String, serde_json::Map<String, Value>> {
    static OVERRIDES: OnceLock<HashMap<String, serde_json::Map<String, Value>>> = OnceLock::new();
    OVERRIDES.get_or_init(|| {
        let mut items = HashMap::new();
        let mut note = |key: &str, object: &serde_json::Map<String, Value>| {
            items.insert(key.to_string(), object.clone());
        };
        each_item(game_item_file(), 0, &mut note);
        items
    })
}

/// Whether [`sync_server_items`] writes anything. Off, it still reads and
/// reports.
///
/// # What a write does to the server, and why it needs [`lift_mod_items`]
///
/// `setting_set_json` does not change the one item it is given. The host
/// serializes the server's WHOLE item settings to JSON, swaps the fragment
/// in, deserializes the lot, drops the old settings and puts the new ones in
/// their place (0.6.3: handler `0x2dc7670`, out through `0x2e376a0`, back in
/// through `0x2d8b670`). Every mod's items are in that document, under
/// `mod_items`, and the JSON form of one is eight plain fields (the exe's own
/// "struct ModItemEntry with 8 elements": key, icon, price, tier, stat,
/// next_tier, tags, category). What an entry holds besides those is the
/// mod's item itself, the object its hooks are called on, and that does not
/// come back: the rebuilt entry has none, the game takes an item without one
/// for inactive, and the old entry, the one that had it, is dropped.
///
/// 0.11.13 wrote here unguarded. What players saw (2026-10-09): matches the
/// server plays by itself, solo rank first of all, with the game's own items
/// and nothing else. No boots and no jungle item either, which Smart Builds
/// puts in every build it is shown, so those builds were made from a list
/// with none of the mod's items in it. Saving, loading and reinstalling
/// changed nothing, because the write happened again at every server start:
/// a save keeps the stats it is given and not the prices. The test log of a
/// save written to, saved and loaded again (2026-10-09) had Radiant
/// Bloodthirster back at 2000 gold beside the 50 attack of the session
/// before, and 24 of the thirty items to write again. So there is something
/// to write at every load, for good. The match a player watches is played by
/// the client from its own copy, which is why it looked right there, and why
/// the game's log had the two runs of one match disagreeing on all ten
/// players.
///
/// So a write happens with the mod items lifted out of the settings and put
/// back after it, and not at all where they cannot be, which makes it safe
/// however often it happens; and `next_tier` is left as the server has it.
/// Seen in that same log: `24 written, 0 refused; mod items: lifted`, so the
/// host takes a write made while the list is out.
const SYNC_SERVER_ITEMS: bool = true;

/// Whether the server of the save now loaded has been through
/// [`sync_server_items`].
static SERVER_ITEMS_SYNCED: AtomicBool = AtomicBool::new(false);

/// A new server: its item settings have not been looked at.
pub(crate) fn server_started() {
    SERVER_ITEMS_SYNCED.store(false, Ordering::Relaxed);
}

/// Whether the save's item totals have been read, which is also the save's
/// namespace having answered at all (see [`LOAD_GRACE`]).
pub(crate) fn loaded() -> bool {
    with_agg(|agg| agg.loaded).unwrap_or(false)
}

/// Whether two settings values say the same thing, a number being the same
/// whether the host wrote it `50` or `50.0`.
fn same_setting(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => a.as_f64() == b.as_f64(),
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter().all(|(name, value)| {
                    b.get(name).is_some_and(|other| same_setting(value, other))
                })
        }
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_setting(a, b))
        }
        _ => a == b,
    }
}

/// Makes the server's item settings hold the mod's numbers for the game's
/// thirty items: each item of the mod's settings file ([`game_item_file`]) is
/// merged over what the server holds for it and written back where that
/// changes anything. Once for each server, from its start and then from its
/// ticks until its settings could be read.
///
/// The server's settings are the authoritative ones: the stable API calls
/// them the game rule settings, which "affect matches created afterwards",
/// and the client's settings document is its read-only view of them. That
/// view gave the base game's numbers (Radiant Bloodthirster at 2000 gold and
/// 100 attack, where the mod's file says 1000 and 50), on the mod's own
/// tooltip and then on the game's own on the match result screen (the user,
/// 2026-10-08), so the `merge` in `mod.override_info` cannot be relied on to
/// have reached them. Whether matches were run on those numbers too is not
/// known; this says what the server held, in the test log, and from here on
/// it holds the mod's either way.
///
/// A write the server refuses changes nothing there (its document must still
/// deserialize), and is counted.
///
/// Only the numbers are written. An item's `next_tier` is the server's to
/// keep: the file knows the base game's tree alone, and what the game has
/// added to those lists for the mods' items is not this function's to undo
/// (see [`SYNC_SERVER_ITEMS`]).
pub(crate) fn sync_server_items(ctx: &mut StableServerCtx<'_>) {
    if SERVER_ITEMS_SYNCED.load(Ordering::Relaxed) {
        return;
    }
    let (mut read, mut written, mut refused) = (0, 0, 0);
    let mut before: Vec<String> = Vec::new();
    // What there is to write, worked out before any of it is written: the
    // mod items come out of the settings once, for all of it.
    let mut writes: Vec<(&str, String)> = Vec::new();
    for (name, over) in game_item_file() {
        let Some(over) = over.as_object() else {
            continue;
        };
        let held = ctx
            .setting_get_json(SettingTargetV1::ItemSetting, name)
            .and_then(|json| serde_json::from_str::<Value>(&json).ok());
        let Some(Value::Object(held)) = held else {
            continue;
        };
        read += 1;
        if GAME_DAMAGE_FINALS.contains(&name.as_str()) {
            before.push(format!(
                "{name}(price={:?} attack={} speed={} power={})",
                held.get("price"),
                item_stat(&held, "attack"),
                item_stat(&held, "attack_speed_mult"),
                item_stat(&held, "magic_power"),
            ));
        }
        let mut merged = held.clone();
        merge_over(&mut merged, over);
        match held.get("next_tier") {
            Some(next_tier) => merged.insert("next_tier".to_string(), next_tier.clone()),
            None => merged.remove("next_tier"),
        };
        let merged = Value::Object(merged);
        if same_setting(&merged, &Value::Object(held)) {
            continue;
        }
        writes.push((name.as_str(), merged.to_string()));
    }
    // Nothing read: the server has no item settings to show yet, or the
    // mod's file could not be read. The next tick asks again, which costs a
    // file's worth of lookups in an empty map in the second case.
    if read == 0 {
        return;
    }
    SERVER_ITEMS_SYNCED.store(true, Ordering::Relaxed);
    let differed = writes.len();
    let mut mod_items = "untouched";
    if SYNC_SERVER_ITEMS && !writes.is_empty() {
        match lift_mod_items(ctx) {
            Ok(lift) => {
                mod_items = if lift.is_some() {
                    "lifted"
                } else {
                    "none to lift"
                };
                for (name, json) in &writes {
                    if ctx.setting_set_json(SettingTargetV1::ItemSetting, name, json) {
                        written += 1;
                    } else {
                        refused += 1;
                    }
                }
                // Back in, into the settings the writes left behind.
                drop(lift);
            }
            Err(()) => mod_items = "could not be lifted, so nothing was written",
        }
    }
    crate::match_builds::log("server items", || {
        format!(
            "{read} read, {differed} unlike the mod's file, {written} written, {refused} refused; mod items: {mod_items}; held before: {}",
            before.join(" ")
        )
    });
}

/// Takes every mod's items out of the server's item settings for the length
/// of a write there, which would otherwise rebuild each of them without the
/// item it stands for (see [`SYNC_SERVER_ITEMS`]). They go back in when what
/// this returns is dropped.
///
/// Nothing to lift where the server holds no mod items, and then a write
/// harms nothing. An error where it holds some and they could not be taken
/// out: the caller must not write. The list the server itself reports is what
/// the native half checks its reading of the settings against, entry by
/// entry, before it touches them (`tactics::lift_server_mod_items`).
fn lift_mod_items(ctx: &StableServerCtx<'_>) -> Result<Option<crate::tactics::ModItemsLift>, ()> {
    let listed = ctx
        .setting_get_json(SettingTargetV1::ItemSetting, "mod_items")
        .and_then(|json| serde_json::from_str::<Value>(&json).ok());
    let Some(Value::Array(listed)) = listed else {
        return Err(());
    };
    if listed.is_empty() {
        return Ok(None);
    }
    let keys = listed
        .iter()
        .map(|item| item.get("key").and_then(Value::as_str).map(str::to_string))
        .collect::<Option<Vec<String>>>()
        .ok_or(())?;
    crate::tactics::driver::lift_server_mod_items(ctx, &keys)
        .map(Some)
        .ok_or(())
}

/// One of an item's stats from its settings object, as a whole number: the
/// host is free to write `50.0`. Nothing where the object has no such stat.
pub(crate) fn item_stat(object: &serde_json::Map<String, Value>, name: &str) -> i64 {
    object
        .get("stat")
        .and_then(|stat| stat.get(name))
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_f64().map(|value| value as i64))
        })
        .unwrap_or(0)
}

/// Hands the Smart Builds rules the game's own thirty items at start-up,
/// straight from the mod's settings file ([`game_item_overrides`]): all of
/// them, with the numbers the game runs with and the player's config in
/// them.
///
/// The rules used to wait for the client's settings document to describe
/// those items ([`prime_item_traits`]), which it only does around a match,
/// with the base game's numbers, and which twice left the rules holding
/// nothing or blanks (Luden's Tempest on AD champions, Bloodthirster on an AP
/// one, both 2026-10-08). The file is on disk from the start and is the one
/// place those numbers are written down, so it is read instead of a list of
/// them being kept in the code (the user asked for the list; this is it,
/// without a second copy to fall behind). `smart_builds::GAME_ITEMS` is what
/// is left for a file that cannot be read.
pub(crate) fn prime_game_items() {
    for (key, object) in game_item_overrides() {
        let stat = |name: &str| item_stat(object, name) as i32;
        crate::smart_builds::note_engine_item(
            key,
            stat("crit_chance"),
            stat("attack"),
            stat("attack_speed_mult"),
            stat("magic_power"),
        );
    }
}

/// Writes `over` onto `base` the way a `merge` override does: field by field,
/// objects merged in turn, anything else replaced.
fn merge_over(base: &mut serde_json::Map<String, Value>, over: &serde_json::Map<String, Value>) {
    for (name, value) in over {
        if let (Some(Value::Object(below)), Value::Object(above)) = (base.get_mut(name), value) {
            merge_over(below, above);
            continue;
        }
        base.insert(name.clone(), value.clone());
    }
}

/// One item's settings as the game holds them, from its `object` in the
/// client's settings document.
///
/// For the game's own thirty items that document gives the base game's
/// numbers, not what the mod's file makes of them: the Check Tactics tooltip
/// showed Radiant Bloodthirster at 2000 gold and 100 attack, where the game
/// charges 1000 for 50 (the user, 2026-10-08). So the mod's file is merged
/// over the object here, as the game does it. An item the file does not
/// have, which is every mod's own, comes back as it is.
pub(crate) fn merged_item<'a>(
    key: &str,
    object: &'a serde_json::Map<String, Value>,
) -> Cow<'a, serde_json::Map<String, Value>> {
    match game_item_overrides().get(key) {
        Some(over) => {
            let mut merged = object.clone();
            merge_over(&mut merged, over);
            Cow::Owned(merged)
        }
        None => Cow::Borrowed(object),
    }
}

/// Calls `visit` with the key and the settings object of every item under
/// `map`, the root of the settings document at `depth` 0.
pub(crate) fn each_item(
    map: &serde_json::Map<String, Value>,
    depth: usize,
    visit: &mut dyn FnMut(&str, &serde_json::Map<String, Value>),
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
                each_item(object, depth + 1, visit);
            }
            continue;
        }
        // Filed under the item's own `key` field, not the map key it sits
        // under. They are the same for all but one item — `iron_blade` calls
        // itself `ironsword` — and the inner one is the identity that matters:
        // it is what `StablePlayer::item_keys` returns and what the item text
        // document is keyed by. Using the map key left that row with a raw
        // `ironsword` and no icon.
        let key = object
            .get("key")
            .and_then(Value::as_str)
            .filter(|inner| !inner.is_empty())
            .unwrap_or(key);
        visit(key, object);
    }
}

fn collect_items(
    ctx: &StableClient<'_>,
    root: &serde_json::Map<String, Value>,
    out: &mut BTreeMap<String, ItemInfo>,
) {
    each_item(
        root,
        0,
        &mut |key: &str, object: &serde_json::Map<String, Value>| {
            out.insert(
                key.to_string(),
                ItemInfo {
                    name: display_name(ctx, key),
                    frame: icon_frame(object, key),
                    tier: object
                        .get("tier")
                        .and_then(Value::as_u64)
                        .map(|tier| tier as usize),
                },
            );
        },
    );
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

/// Set when the counters change, cleared when they reach disk.
static DIRTY: AtomicBool = AtomicBool::new(false);

/// The totals format this build writes and is willing to read.
///
/// A table that does not match is ignored rather than migrated, and that save's
/// counters start over. These cannot be recomputed from anything — the matches
/// behind them are long gone — so a shape change is the one case where history is
/// lost, and worth weighing before bumping this.
const FORMAT: u32 = 1;

/// The key the whole table lives under, inside this mod's own namespace in the
/// save file. One key: the table is written whole, so splitting it would only add
/// a way for the halves to disagree.
const KEY: &str = "item_stats";

/// Frames to wait for the save's namespace to answer before believing it.
///
/// The namespace reads empty on some frames while `save_can_write` already
/// answers true — mid-load, or across a scene change. Believing the first empty
/// read would start this save's table from nothing and then write that over its
/// real history, so an absent key is only accepted after it has stayed absent
/// this long. A genuinely new save simply waits these frames out once.
const LOAD_GRACE: u32 = 600;

/// Frames spent waiting for that answer.
static WAITED: Mutex<u32> = Mutex::new(0);

/// Loads the save's table, folds anything the simulation has captured, and
/// writes the result back.
///
/// # Why folding happens here rather than on the statistics screen
///
/// It used to run only while that screen was open, so a season could be played
/// with every match still sitting in the capture queue — which is what made that
/// queue a 2MB file. Driven from the management tick instead, the queue drains
/// within a tick or two of a match ending and never needs to persist at all.
///
/// A pass is only queued when it could find something new (see [`due_pass`]),
/// so a frame with captures waiting on records that never come costs an atomic
/// read, and a `record_ids` call every [`SWEEP_CHECK_FRAMES`].
///
/// # What "saved" means now
///
/// `save_set_string` writes the *in-memory* save. It reaches disk when the player
/// saves, and not before — quit without saving and the session's matches are gone
/// along with everything else that session. That is the trade for the table being
/// tied to the save rather than to a folder beside the DLL, and it is what makes
/// loading an older save show that save's numbers instead of a future's.
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    if !ctx.save_can_write() {
        // Back at the menu, or between saves. Drop everything so the next save
        // loads its own table rather than inheriting this one's.
        if with_agg(|agg| agg.loaded).unwrap_or(false) {
            let _ = with_agg(|agg| *agg = Aggregate::default());
            crate::item_stats::sim::forget();
        }
        if let Ok(mut waited) = WAITED.lock() {
            *waited = 0;
        }
        DIRTY.store(false, Ordering::Relaxed);
        return;
    }

    if !with_agg(|agg| agg.loaded).unwrap_or(false) && !load_from_save(ctx) {
        return;
    }

    // Not while a pass is in flight: queuing one resets it to the newest record,
    // so captures arriving faster than a pass finishes would keep it re-reading
    // the same batch. What arrives meanwhile is still new to `due_pass` once the
    // pass ends, and gets a pass of its own then.
    let in_flight = with_agg(|agg| !agg.pending.is_empty()).unwrap_or(false);
    if !in_flight && crate::item_stats::sim::pending() > 0 {
        if let Some((ids, verify)) = due_pass(ctx) {
            queue_pass(&ids, verify);
        }
    }
    // A pass in flight goes on from where the last frame left it, rather than
    // being queued afresh. With none in flight this reads nothing.
    while pump(ctx) {}

    flush(ctx);
}

/// Reads the save's table into the aggregate, or decides it has none.
///
/// Returns whether the table may now be folded into. See [`LOAD_GRACE`] for why
/// an empty answer is not taken at face value.
fn load_from_save(ctx: &StableClient<'_>) -> bool {
    if ctx.save_version() as u32 == FORMAT {
        if let Some(text) = ctx.save_get_string(KEY) {
            let _ = with_agg(|agg| {
                agg.loaded = true;
                load_into(agg, &text);
            });
            return true;
        }
    } else if ctx.save_contains_key(KEY) {
        // A table this build will not read. Left where it is rather than
        // removed: a downgrade should still find its own numbers.
        let _ = with_agg(|agg| agg.loaded = true);
        return true;
    }

    let waited = WAITED
        .lock()
        .map(|mut frames| {
            *frames += 1;
            *frames
        })
        .unwrap_or(LOAD_GRACE);
    if waited < LOAD_GRACE {
        return false;
    }
    // Nothing there after the grace period: a save that has never carried this
    // table. Starting empty is now safe.
    let _ = with_agg(|agg| agg.loaded = true);
    true
}

/// Writes the counters out if they changed since the last call.
///
/// Driven from the management tick beside the queue's own flush. The file is a
/// few thousand rows whatever the save has been through, so unlike the history it
/// replaced this costs the same on the first match and the ten-thousandth.
pub(crate) fn flush(ctx: &mut StableClient<'_>) {
    if !DIRTY.swap(false, Ordering::Relaxed) {
        return;
    }
    // Never before the save's own table has been read, or this writes an empty
    // one over it. `sync` will not fold either until then, so in practice this
    // guard only fires if something else marks the table dirty first.
    if !with_agg(|agg| agg.loaded).unwrap_or(false) {
        return;
    }
    let Some(text) = with_agg(serialise) else {
        return;
    };
    ctx.save_set_version(FORMAT as usize);
    ctx.save_set_string(KEY, &text);
}

/// `{"v": 1, "t": {"<patch>": {"m": matches, "i": [entry, ...]}}}`, where an
/// entry is `{"n": lane, "k": item, "g": games, "w": wins, "f": firsts,
/// "c": {champion: count}}`.
///
/// The champion tally rides inside the entry rather than in a map of its own
/// because both are keyed by the same `(lane, item)` pair — every champion count
/// came from a player whose items were counted in the same pass, so the item map
/// is always a superset and there is no second key space to keep in step.
///
/// `n` is omitted for a player the host gave no lane for, and `c` when no champion
/// on the entry could be named.
fn serialise(agg: &mut Aggregate) -> String {
    let mut out = format!("{{\"v\":{FORMAT},\"t\":{{");
    for (slot, (patch, per_item)) in agg.counts.iter().enumerate() {
        if slot > 0 {
            out.push(',');
        }
        let matches = agg.matches.get(patch).copied().unwrap_or(0);
        out.push_str(&format!("{}:{{\"m\":{matches},\"i\":[", quote(patch)));
        let per_champion = agg.champions.get(patch);
        for (slot, ((lane, item), totals)) in per_item.iter().enumerate() {
            if slot > 0 {
                out.push(',');
            }
            out.push('{');
            if let Some(lane) = lane {
                out.push_str(&format!("\"n\":{lane},"));
            }
            out.push_str(&format!(
                "\"k\":{},\"g\":{},\"w\":{},\"f\":{}",
                quote(item),
                totals.games,
                totals.wins,
                totals.firsts
            ));
            let tally = per_champion.and_then(|per| per.get(&(*lane, item.clone())));
            if let Some(tally) = tally.filter(|tally| !tally.is_empty()) {
                out.push_str(",\"c\":{");
                for (slot, (champion, count)) in tally.iter().enumerate() {
                    if slot > 0 {
                        out.push(',');
                    }
                    out.push_str(&format!("{}:{count}", quote(champion)));
                }
                out.push('}');
            }
            out.push('}');
        }
        out.push_str("]}");
    }
    out.push_str("}}");
    out
}

fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn load_into(agg: &mut Aggregate, text: &str) {
    let Ok(Value::Object(file)) = serde_json::from_str::<Value>(text) else {
        return;
    };
    if file.get("v").and_then(Value::as_u64) != Some(FORMAT as u64) {
        return;
    }
    let Some(Value::Object(patches)) = file.get("t") else {
        return;
    };

    for (patch, body) in patches {
        let Some(body) = body.as_object() else {
            continue;
        };
        let matches = body.get("m").and_then(Value::as_u64).unwrap_or(0) as u32;
        let Some(Value::Array(entries)) = body.get("i") else {
            continue;
        };
        let per_item = agg.counts.entry(patch.clone()).or_default();
        let mut champions: BTreeMap<(Option<usize>, String), BTreeMap<String, u32>> =
            BTreeMap::new();
        for entry in entries {
            let Some(fields) = entry.as_object() else {
                continue;
            };
            let Some(item) = fields.get("k").and_then(Value::as_str) else {
                continue;
            };
            let lane = fields
                .get("n")
                .and_then(Value::as_u64)
                .map(|lane| lane as usize);
            let key = (lane, item.to_string());
            let read = |name: &str| fields.get(name).and_then(Value::as_u64).unwrap_or(0) as u32;
            per_item.insert(
                key.clone(),
                Totals {
                    games: read("g"),
                    wins: read("w"),
                    firsts: read("f"),
                },
            );
            if let Some(Value::Object(tally)) = fields.get("c") {
                let per_champion = champions.entry(key).or_default();
                for (champion, count) in tally {
                    let Some(count) = count.as_u64() else {
                        continue;
                    };
                    per_champion.insert(champion.clone(), count as u32);
                }
            }
        }
        agg.matches.insert(patch.clone(), matches);
        if !champions.is_empty() {
            agg.champions.insert(patch.clone(), champions);
        }
    }
}

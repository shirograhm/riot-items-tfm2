//! Per-item win/loss totals over the loadouts [`sim`] captured. A match record
//! supplies only its patch (`version`) and the `seed` that joins it to a
//! capture; other leagues' captures are placed by the server
//! ([`place_captures`]).
//!
//! Only the running totals are kept, in the save under this mod's namespace
//! (see [`sync`]): a match is folded in once and dropped, so nothing can be
//! recomputed and a new column fills only from new matches. Record ids are
//! pruned and reused, which is harmless because records contribute no numbers.

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

// Records read per [`pump`] call: each costs about half a millisecond on the UI
// thread, and most passes read only new records anyway.
const CHUNK: usize = 8;

#[derive(Clone, Copy, Default)]
pub(crate) struct Totals {
    pub games: u32,
    pub wins: u32,
    // Games where this item was in the player's first slot. Items are appended
    // as they are completed, so slot 0 is the one finished first, the only
    // purchase order the simulation exposes.
    pub firsts: u32,
}

impl Totals {
    pub fn losses(&self) -> u32 {
        self.games.saturating_sub(self.wins)
    }

    // Win rate in percent; `None` for no games, which must not print as 0%.
    pub fn win_rate(&self) -> Option<f64> {
        (self.games > 0).then(|| self.wins as f64 * 100.0 / self.games as f64)
    }

    // Share of this item's buys where it was bought first, in percent; `None`
    // for no games, as for [`Totals::win_rate`].
    pub fn first_rate(&self) -> Option<f64> {
        (self.games > 0).then(|| self.firsts as f64 * 100.0 / self.games as f64)
    }

    // Times built per match, in percent (`games` counts players, so 200% is two
    // a match); `None` with no matches. `matches` ignores the lane filter, so
    // with a lane picked this reads "built in that lane, per match".
    pub fn play_rate(&self, matches: u32) -> Option<f64> {
        (matches > 0).then(|| self.games as f64 * 100.0 / matches as f64)
    }
}

#[derive(Default)]
struct Aggregate {
    // Record ids still to read for their patch, newest first.
    pending: Vec<usize>,
    // Each read record's patch and seed (`None` if it cannot be placed), so a
    // pass reads only new ids. An id that leaves the list is forgotten, since
    // it may come back as another match; the periodic verify pass catches ids
    // reused in between (see [`due_pass`]).
    read: HashMap<usize, Option<(String, u64)>>,
    // The queued pass has not yet matched captures against [`Self::read`].
    rematch: bool,
    // Whether the save's counters have been read in. Nothing may be written
    // before then: a read can come back empty while `save_can_write` is already
    // true, and saving that would wipe the history.
    loaded: bool,
    // Patch -> (lane, item) -> totals. Keyed by patch first so the patch filter
    // is a lookup and "All" a merge; the lane rides in the inner key so both
    // filters are one pass.
    counts: BTreeMap<String, BTreeMap<(Option<usize>, String), Totals>>,
    // Patch -> (lane, item) -> champion -> times that champion was holding it.
    // Feeds the "purchased on" column, which is the top few of these by count.
    champions: BTreeMap<String, BTreeMap<(Option<usize>, String), BTreeMap<String, u32>>>,
    // Patch -> captured matches.
    matches: BTreeMap<String, u32>,
}

static AGG: Mutex<Option<Aggregate>> = Mutex::new(None);

fn with_agg<T>(f: impl FnOnce(&mut Aggregate) -> T) -> Option<T> {
    let mut guard = AGG.lock().ok()?;
    Some(f(guard.get_or_insert_with(Aggregate::default)))
}

// What the panel draws.
pub(crate) struct Snapshot {
    // Items in display order — see [`rows`].
    pub rows: Vec<(String, Totals)>,
    pub matches: u32,
    // Records still to read. Non-zero means a patch pass is in flight.
    pub pending: usize,
    // Per item, the champions that bought it most, best first, at most
    // [`TOP_CHAMPIONS`] of them.
    pub champions: BTreeMap<String, Vec<String>>,
}

// How many champions the "purchased on" column shows: as many as the vanilla
// "Most Used Champ" column, whose 132px cell they fill exactly.
pub(crate) const TOP_CHAMPIONS: usize = 3;

// Queues a pass over the match records to backfill patches. Record ids are
// pruned and reused, so ids that leave the list are forgotten; re-reading is
// harmless since captures are deduplicated by seed.
pub(crate) fn sweep(ctx: &StableClient<'_>) {
    let ids = crate::perf::time(crate::perf::Section::RecordIds, || {
        ctx.record_ids(RecordKindV1::MatchReplay)
    });
    queue_pass(&ids, false);
}

// [`sweep`] over ids already in hand. `verify` re-reads the records already
// in [`Aggregate::read`] instead of trusting them.
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

// Frames between looks at the record list while captures are waiting.
const SWEEP_CHECK_FRAMES: u32 = 30;

// Frames between passes that re-read every record, to catch an id reused
// between two looks at the list. A long save holds thousands of records, so
// this runs about every five minutes.
const SWEEP_VERIFY_FRAMES: u32 = 18_000;

// What the records looked like when the last pass was queued.
#[derive(Default)]
struct SweepMark {
    // [`crate::item_stats::sim::captures`] at the time.
    captures: u64,
    ids: Vec<usize>,
    // Frames since the id list was last looked at.
    since_check: u32,
    // Frames since the last verify pass was queued.
    since_verify: u32,
}

static SWEEP_MARK: Mutex<Option<SweepMark>> = Mutex::new(None);

// The record ids to pass over, and whether it is a verify pass, when a pass
// could find something new; `None` otherwise. A pass every frame while a
// capture waits for its record would read records in full each frame (~10 ms),
// so it needs a new capture or a changed id list (checked every
// [`SWEEP_CHECK_FRAMES`]).
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

// Reads a bounded batch of records for their patches and folds any captures
// they vouch for. Returns whether anything was folded, or the pass just
// finished.
pub(crate) fn pump(ctx: &StableClient<'_>) -> bool {
    // Never fold before the save's table is loaded: the load would overwrite
    // the fold, and the capture would be lost.
    if !with_agg(|agg| agg.loaded).unwrap_or(false) {
        return false;
    }
    // First the captures the server placed ([`place_captures`]), which need no
    // record.
    let mut folded = false;
    for (patch, players) in crate::item_stats::sim::take_placed() {
        fold(&patch, &players);
        folded = true;
    }
    // Every capture is matched, so there is nothing to read records for.
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

    // Then records already read: a capture often arrives after its record was
    // read.
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
        // No capture waiting: simmed before capturing began, or already
        // counted.
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

// Folds one vouched match into the running totals; called once per match, ever.
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
                // Slot 0 is the item finished first (see [`Totals::firsts`]).
                entry.firsts += u32::from(slot == 0);
            }
        }

        let per_champion = agg.champions.entry(patch.to_string()).or_default();
        for player in players {
            // A nameless player still counts toward games and wins, but not as
            // a champion, or the blank would outrank real names in the
            // "purchased on" column.
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

// Matches folded since the game started.
static FOLDS: AtomicU64 = AtomicU64::new(0);

// Matches folded so far; the statistics screen repaints when this moves, since
// most folds happen in [`sync`].
pub(crate) fn folds() -> u64 {
    FOLDS.load(Ordering::Relaxed)
}

// Sets the server looks up per pass: 32 a second with [`PLACE_EVERY`], at two
// small reads each.
const PLACE_BATCH: usize = 8;

// Least time between passes, by the clock, since the server's tick rate cannot
// be counted on.
const PLACE_EVERY: Duration = Duration::from_millis(250);

// Test-log lines per look-up outcome; a set whose record never comes is asked
// about every round.
const PLACINGS_TO_LOG: u32 = 8;

// What the server's records said about one captured set.
struct Placing {
    // The match record's `replays`, as the server gave it.
    listed: Option<String>,
    // The set's replay record.
    replay: Option<u64>,
    // That record's `version`.
    patch: Option<String>,
}

// Looks a captured set up in the server's records: its replay record via the
// match record's `replays` list, unless the simulation already named it. An
// unrecorded set is asked about again later.
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

// Finds the patch of captures no record's seed asks for (other leagues' sets)
// so [`pump`] can fold them. Runs on the server, where `RecordKindV1::Match` is
// the whole table, a few sets up to four times a second; none is given up on,
// since records arrive when a game day is committed.
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

// Patches seen in the records, newest first: exactly the patches there is
// something to filter to.
pub(crate) fn patches() -> Vec<String> {
    with_agg(|agg| {
        let mut out: Vec<String> = agg.counts.keys().cloned().collect();
        out.sort_by(|a, b| b.cmp(a));
        out
    })
    .unwrap_or_default()
}

// The table for one patch and lane, or all, in key order: sorting is the UI's
// job, and key order keeps equal rows from reshuffling between repaints.
pub(crate) fn snapshot(patch: Option<&str>, lane: Option<usize>) -> Snapshot {
    with_agg(|agg| {
        // One patch reads its own submap; "All" merges them, so there is one
        // set of numbers to be wrong.
        let mut counts: BTreeMap<String, Totals> = BTreeMap::new();
        let mut champions: BTreeMap<String, BTreeMap<String, u32>> = BTreeMap::new();
        let mut matches = 0;

        // A player with no lane matches no lane filter, only "All".
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

// The most frequent champions per item, best first; ties break on key so they
// do not swap places between repaints.
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

// One record's patch and seed, or `None` when it cannot be placed. The whole
// record is fetched in one call where the host allows it.
fn read_record(ctx: &StableClient<'_>, id: usize) -> Option<(String, u64)> {
    // A named read costs about half a full read, so the full read stays first.
    let full = crate::perf::time(crate::perf::Section::RecordRead, || {
        ctx.record_get_json(RecordKindV1::MatchReplay, id, "")
            .and_then(|json| serde_json::from_str::<Value>(&json).ok())
    });
    let record = match full {
        Some(record) => record,
        // Fallback for hosts that refuse the empty path: named reads, rebuilt
        // into the same shape.
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
    // A league match always names its version; one that does not cannot be
    // placed.
    let patch = record
        .get("version")
        .and_then(Value::as_str)
        .filter(|version| !version.is_empty())?
        .to_string();

    // `as_u64` keeps the seed exact, where an `f64` would round it.
    let seed = record.get("seed").and_then(Value::as_u64)?;
    Some((patch, seed))
}

// -- item catalog -----------------------------------------------------------

// Display name and sprite frame for one item key.
#[derive(Clone, Default)]
pub(crate) struct ItemInfo {
    pub name: String,
    // `rect_tag` into the item sheet, or `None` for an item with no art.
    pub frame: Option<String>,
    // 0..=4 for starter/basic/epic/legendary/radiant; `None` if nothing
    // describes the item.
    pub tier: Option<usize>,
}

static CATALOG: Mutex<Option<BTreeMap<String, ItemInfo>>> = Mutex::new(None);

// This mod's item keys and tiers in registration order, since the settings
// document describes only the 30 base items.
static REGISTERED: Mutex<Vec<(String, usize)>> = Mutex::new(Vec::new());

// Notes one item key and tier; called from `init` for every item the mod adds.
pub(crate) fn note_registered(key: &str, tier: usize) {
    if let Ok(mut keys) = REGISTERED.lock() {
        keys.push((key.to_string(), tier));
    }
}

// Builds the item table from the settings document, once, keeping every item
// (components too, since a match can end on them). Called from `post_update`,
// because `setting_get_json` does not work inside a UI click handler.
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

    // An empty result is not cached: the document may be readable on a later
    // frame.
    if out.is_empty() {
        return;
    }

    // The mod's items are not in the settings document, so they come from the
    // mod: names from the item text, and the key itself as the sprite frame.
    // Not `icon_frame`'s `base_slug`, which strips `radiant_` and would draw
    // the plain art.
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

// The catalog as [`prime_catalog`] last left it, empty until it succeeds.
// Takes no ctx, so it is safe to call from a click handler.
pub(crate) fn catalog() -> BTreeMap<String, ItemInfo> {
    CATALOG
        .lock()
        .ok()
        .and_then(|cached| cached.clone())
        .unwrap_or_default()
}

// One of the game's own thirty items (drawn as Radiant Luden's Tempest): a
// settings document that describes it has the game's items, not only mods'.
pub(crate) const A_GAME_ITEM: &str = "prophet_of_the_abyss";

// The game's finals that give attack, attack speed or ability power, which
// [`prime_item_traits`] logs since the rules once read blank records of them.
const GAME_DAMAGE_FINALS: [&str; 3] = ["warlords_final_judgement", "storm_sovereign", A_GAME_ITEM];

// Frames between two tries of [`prime_item_traits`] while the settings
// document cannot be read: half a second at 60 frames a second.
const TRAITS_RETRY_FRAMES: u32 = 30;

// Hands the game's own item stats from the settings document to the Smart
// Builds rules, retrying until it settles. Separate from [`prime_catalog`],
// which also caches display names and must wait for the item text.
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
    // For the test log: what was read under the game's three damage finals.
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
            crate::route_choice::note_engine_item(key, |name| item_stat(&object, name));
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
            // Settled only by real numbers in the document's own words, not the
            // merged ones (see below).
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
    // Settle only once the document has real numbers for the game's items:
    // until then it holds all thirty with zeroed stats, so key presence or a
    // time limit would record blanks.
    if games {
        PRIMED.store(true, Ordering::Relaxed);
    }
}

// Passive numbers the engine keeps outside the stat block: config entry,
// config field, the settings field, and what one of the config's is of the
// setting's (a range is written in thousandths of a distance unit).
const GAME_PASSIVE_FIELDS: [(&str, &str, &str, i64); 8] = [
    ("giants_belt", "effect_bonus_hp_regen", "flat_regen", 1),
    ("sunfire_cape", "effect_bonus_hp_regen", "flat_regen", 1),
    ("radiant_sunfire_cape", "effect_bonus_hp_regen", "flat_regen", 1),
    ("radiant_sunfire_cape", "effect_bonus_flat_damage", "flat_aoe_damage", 1),
    ("radiant_sunfire_cape", "effect_caster_hp_percent_damage", "max_hp_aoe_ratio", 1),
    ("radiant_sunfire_cape", "effect_max_distance", "aoe_range", 1000),
    ("radiant_thornmail", "effect_bonus_flat_damage", "flat_damage", 1),
    ("radiant_thornmail", "effect_caster_defence_percent_damage", "defence_ratio", 1),
];

// Config fields of those items that are the DLL's to read, not the settings
// file's: Sunfire Cape's Immolate is mod code, and so is both capes' bonus
// against minions and monsters.
const GAME_DLL_FIELDS: [(&str, &str); 5] = [
    ("sunfire_cape", "effect_bonus_flat_damage"),
    ("sunfire_cape", "effect_caster_hp_percent_damage"),
    ("sunfire_cape", "effect_max_distance"),
    ("sunfire_cape", "effect_minion_bonus_percent"),
    ("radiant_sunfire_cape", "effect_minion_bonus_percent"),
];

// Writes the config's numbers for the game's thirty items (`config.json` over
// `config-default.json`) into the settings file `root`, and returns whether
// anything changed. Prices, [`GAME_PASSIVE_FIELDS`] and stat-block stats,
// rounded half to even.
fn apply_game_item_config(root: &mut serde_json::Map<String, Value>) -> bool {
    let defaults = crate::patches::base::read_object("config-default.json");
    let overrides = crate::patches::base::read_object("config.json");
    let mut changed = false;
    for (name, key) in crate::item_catalog::GAME_ITEM_NAMES {
        let Some(Value::Object(entry)) = root.get_mut(key) else {
            continue;
        };
        for config in [&defaults, &overrides] {
            let Some(Value::Object(numbers)) = config.get(name) else {
                continue;
            };
            for (field, value) in numbers {
                let Some(number) = value.as_f64() else {
                    continue;
                };
                if GAME_DLL_FIELDS.contains(&(name, field.as_str())) {
                    continue;
                }
                let whole = number.round_ties_even() as i64;
                let passive = GAME_PASSIVE_FIELDS
                    .iter()
                    .find(|(item, config_field, _, _)| *item == name && *config_field == field);
                let slot = if field == "price" {
                    entry.get_mut("price").map(|slot| (slot, whole))
                } else if let Some(&(_, _, setting, scale)) = passive {
                    entry.get_mut(setting).map(|slot| (slot, whole * scale))
                } else {
                    entry
                        .get_mut("stat")
                        .and_then(Value::as_object_mut)
                        .and_then(|stat| stat.get_mut(field))
                        .map(|slot| (slot, whole))
                };
                // A field the item has no place for is skipped.
                let Some((slot, wanted)) = slot else {
                    continue;
                };
                if slot.as_f64() != Some(wanted as f64) {
                    *slot = Value::from(wanted);
                    changed = true;
                }
            }
        }
    }
    changed
}

// The mod's copy of the game's thirty items
// (`setting/item_setting.item_setting`, merged by `mod.override_info`) with the
// player's config applied, read once. Written back when the config changed it,
// so the game's copy matches from its next read; until then
// [`sync_server_items`] gives the server these numbers.
pub(crate) fn game_item_file() -> &'static serde_json::Map<String, Value> {
    static FILE: OnceLock<serde_json::Map<String, Value>> = OnceLock::new();
    FILE.get_or_init(|| {
        let path = crate::config::mod_dir()
            .join("setting")
            .join("item_setting.item_setting");
        let read: Option<Value> = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(text.trim_start_matches('\u{feff}')).ok());
        let Some(Value::Object(mut root)) = read else {
            return serde_json::Map::new();
        };
        if apply_game_item_config(&mut root) {
            let written = serde_json::to_string_pretty(&root)
                .ok()
                .is_some_and(|text| std::fs::write(&path, text).is_ok());
            crate::match_builds::log("config.game_items", || {
                format!("the config changes the game's own items; settings file written: {written}")
            });
        }
        root
    })
}

// [`game_item_file`] by item key.
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

// Whether [`sync_server_items`] writes anything; off, it only reads and
// reports.
//
// `setting_set_json` rebuilds the server's whole item settings from JSON, and a
// mod item rebuilt that way loses the item object its hooks run on, leaving
// server-played matches without mod items. So writes happen only with the mod
// items lifted out and put back after ([`lift_mod_items`]), and `next_tier` is
// left as the server has it.
const SYNC_SERVER_ITEMS: bool = true;

// Whether the server of the save now loaded has been through
// [`sync_server_items`].
static SERVER_ITEMS_SYNCED: AtomicBool = AtomicBool::new(false);

// A new server: its item settings have not been looked at.
pub(crate) fn server_started() {
    SERVER_ITEMS_SYNCED.store(false, Ordering::Relaxed);
}

// Whether the save's item totals have been read, which is also the save's
// namespace having answered at all (see [`LOAD_GRACE`]).
pub(crate) fn loaded() -> bool {
    with_agg(|agg| agg.loaded).unwrap_or(false)
}

// Whether two settings values say the same thing, a number being the same
// whether the host wrote it `50` or `50.0`.
pub(crate) fn same_setting(a: &Value, b: &Value) -> bool {
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

// A game item's `next_tier` as the server holds it, less the links to this
// mod's items that no longer name it (`key`) as what they are built from. The
// game writes each mod item into the `next_tier` of its `previous_tier` as a
// save loads but never takes one out, so a save older than a recipe change
// kept building by the old recipe: Long Sword into Madred's Razors, long after
// Madred's came to build from Hunter's Machete alone. The game's own items and
// other mods' are left as they are.
fn current_next_tier(key: &str, next_tier: &Value) -> Value {
    let Some(list) = next_tier.as_array() else {
        return next_tier.clone();
    };
    let mod_items = &crate::patches::base::base().mod_items;
    let kept = list.iter().filter(|next| {
        next.as_str()
            .and_then(|next| mod_items.get(next))
            .is_none_or(|item| item.previous.iter().any(|previous| previous == key))
    });
    Value::Array(kept.cloned().collect())
}

// Makes the server's item settings hold the mod's numbers for the game's thirty
// items, once per server, merging each from [`game_item_file`] and writing only
// what changed. Matches use the server's settings, and the client's view shows
// base-game numbers, so the `mod.override_info` merge cannot be relied on. Each
// item's `next_tier` stays the server's own, which has this mod's items in it,
// but for links an old save kept from a recipe since changed
// ([`current_next_tier`]).
pub(crate) fn sync_server_items(ctx: &mut StableServerCtx<'_>) {
    if SERVER_ITEMS_SYNCED.load(Ordering::Relaxed) {
        return;
    }
    let (mut read, mut written, mut refused) = (0, 0, 0);
    let mut before: Vec<String> = Vec::new();
    // Work out every write first, so the mod items are lifted out only once.
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
        let key = held.get("key").and_then(Value::as_str).unwrap_or(name);
        match held.get("next_tier") {
            Some(next_tier) => {
                merged.insert("next_tier".to_string(), current_next_tier(key, next_tier))
            }
            None => merged.remove("next_tier"),
        };
        let merged = Value::Object(merged);
        if same_setting(&merged, &Value::Object(held)) {
            continue;
        }
        writes.push((name.as_str(), merged.to_string()));
    }
    // Nothing read: the server has no settings yet or the file is unreadable.
    // The next tick asks again.
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

// Takes every mod's items out of the server's item settings for the length of a
// write (see [`SYNC_SERVER_ITEMS`]); they go back when the result is dropped.
// `Ok(None)` when there are none, `Err` when they could not be lifted, and then
// the caller must not write.
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

// One of an item's stats from its settings object, as a whole number: the
// host is free to write `50.0`. Nothing where the object has no such stat.
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

// Hands the Smart Builds rules the game's thirty items at start-up, from the
// mod's settings file with the player's config applied. The client's settings
// document describes them only around a match, and with base-game numbers.
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
        crate::route_choice::note_engine_item(key, |name| item_stat(object, name));
    }
}

// Writes `over` onto `base` the way a `merge` override does: field by field,
// objects merged in turn, anything else replaced.
pub(crate) fn merge_over(
    base: &mut serde_json::Map<String, Value>,
    over: &serde_json::Map<String, Value>,
) {
    for (name, value) in over {
        if let (Some(Value::Object(below)), Value::Object(above)) = (base.get_mut(name), value) {
            merge_over(below, above);
            continue;
        }
        base.insert(name.clone(), value.clone());
    }
}

// One item's settings as the game holds them: the client's document shows
// base-game numbers for the game's own items, so the mod's file is merged over
// them. Mod items come back unchanged.
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

// Calls `visit` with the key and the settings object of every item under
// `map`, the root of the settings document at `depth` 0.
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
        // Filed under the item's own `key` field, which differs from the map
        // key only for `iron_blade` (`ironsword`) and is what `item_keys` and
        // the item text use.
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

// The item's own name, "Radiant" included: unlike the build editor, this table
// lists both versions as separate rows.
fn display_name(ctx: &StableClient<'_>, key: &str) -> String {
    ctx.i18n(&format!("#asset/base/text/item?{key}.name"))
        .filter(|name| !name.is_empty() && !name.starts_with('#'))
        .unwrap_or_else(|| key.to_string())
}

// The item's frame in the (mod-overridden) sheet: the settings document's
// `icon`, else the key itself. Never `base_slug`, which strips `radiant_` and
// picks the plain art.
fn icon_frame(object: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    object
        .get("icon")
        .and_then(Value::as_str)
        .filter(|icon| !icon.is_empty())
        .map(str::to_string)
        .or_else(|| Some(key.to_string()))
}

// Set when the counters change, cleared when they reach disk.
static DIRTY: AtomicBool = AtomicBool::new(false);

// The totals format this build reads and writes. A mismatched table is ignored,
// not migrated, and totals cannot be recomputed, so bumping this loses history.
const FORMAT: u32 = 1;

// Key of the whole table in this mod's save namespace; one key, since the table
// is written whole.
const KEY: &str = "item_stats";

// Frames to wait for the save namespace before believing an absent table: it
// reads empty on some frames while `save_can_write` is already true, and
// starting empty would overwrite the history. A new save waits this out once.
const LOAD_GRACE: u32 = 600;

// Frames spent waiting for that answer.
static WAITED: Mutex<u32> = Mutex::new(0);

// Loads the save's table, folds new captures and writes the result, from the
// management tick so the capture queue drains within a tick or two of a match.
//
// `save_set_string` writes the in-memory save, which reaches disk only when the
// player saves; that ties the table to the save, so loading an older save shows
// its own numbers.
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    if !ctx.save_can_write() {
        // Back at the menu or between saves: drop everything so the next save
        // loads its own table.
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

    // Not while a pass is in flight: re-queuing restarts it at the newest
    // record. New captures get a pass of their own once it ends.
    let in_flight = with_agg(|agg| !agg.pending.is_empty()).unwrap_or(false);
    if !in_flight && crate::item_stats::sim::pending() > 0 {
        if let Some((ids, verify)) = due_pass(ctx) {
            queue_pass(&ids, verify);
        }
    }
    // Continue any pass in flight; with none this reads nothing.
    while pump(ctx) {}

    flush(ctx);
}

// Reads the save's table into the aggregate, or decides it has none; returns
// whether it may be folded into (see [`LOAD_GRACE`]).
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
        // A table this build cannot read: left in place so a downgrade still
        // finds it.
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
    // Still nothing after the grace period: a save that never had the table.
    let _ = with_agg(|agg| agg.loaded = true);
    true
}

// Writes the counters out if they changed. Their size is bounded by items,
// patches and lanes, so this costs the same however long the save.
pub(crate) fn flush(ctx: &mut StableClient<'_>) {
    if !DIRTY.swap(false, Ordering::Relaxed) {
        return;
    }
    // Never before the save's table is read, or this would write an empty one
    // over it.
    if !with_agg(|agg| agg.loaded).unwrap_or(false) {
        return;
    }
    let Some(text) = with_agg(serialise) else {
        return;
    };
    ctx.save_set_version(FORMAT as usize);
    ctx.save_set_string(KEY, &text);
}

// `{"v": 1, "t": {"<patch>": {"m": matches, "i": [entry, ...]}}}`, an entry
// being `{"n": lane, "k": item, "g": games, "w": wins, "f": firsts, "c":
// {champion: count}}`. `n` is omitted when the lane is unknown and `c` when no
// champion was named.
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

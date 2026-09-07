//! End-of-match item loadouts, captured from the simulation itself.
//!
//! # Why this exists at all
//!
//! [`crate::item_stats`] reads saved match records, and a record's per-player
//! `items` is the build the game *assigned*, not what the champion finished the
//! match holding. The evidence is in the shape of it: across 80 logged player
//! entries every one held exactly 3 or 4 items — never fewer — and every entry
//! was a finished item. Not one component. A real end-state is ragged: someone
//! who fell behind holds two, someone mid-upgrade is sitting on a BF Sword.
//!
//! The simulation knows the difference. `StablePlayer::item_keys` is live
//! inventory, so reading it on the last tick answers the question the record
//! cannot.
//!
//! # Why it is joined on the seed
//!
//! A capture has everything except the **patch**, which only the record carries
//! (`version`). Rather than give up the patch filter, each capture is filed
//! under the match's rng seed, which both sides have: `StableSim::seed` here and
//! a `seed` field on the record. Effectively unique per match, and — unlike a
//! record id or `sim_origin().match_id` — it needs no assumption about how the
//! host numbers things.
//!
//! A record with no capture is skipped rather than counted from its planned
//! build. That is what makes the table "actual only": it starts empty and fills
//! as matches are simmed, instead of quietly mixing two different meanings of
//! "the items in this match".
//!
//! # Why nothing here is written to a file
//!
//! A capture is a **queue entry**, not history: it waits for a record to vouch
//! for it, [`crate::item_stats`] folds it into the running totals, and it is
//! dropped. The totals live in the save file itself, so the only thing this
//! module ever needs to hold is what is in flight.
//!
//! That used to be a file — `item_stats/<save>/queue.json`, which reached 2MB —
//! because folding was gated on opening the statistics screen, so a season could
//! be played with every match still waiting. Folding now runs from the
//! management tick whenever [`pending`] is non-zero, which drains the queue
//! within a tick or two of a match ending. What is left is a buffer measured in
//! seconds, and a buffer that small is not worth a file: the cost of losing it to
//! a crash is the handful of matches simmed in that window.
//!
//! The price, unchanged, is that a column the table does not collect yet cannot
//! be answered retroactively — the raw loadouts are gone once counted, so a new
//! statistic only fills in from matches simmed after it is added.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Mutex;

use mod_api_stable::*;

/// Captures held while they wait for a record to vouch for them.
///
/// The queue drains every time the statistics tab sweeps records, so it holds
/// the matches simmed since the last sweep, not a history. The cap is what stops
/// a save played for a season without ever opening the tab from growing the file
/// without bound; reaching it drops the oldest uncounted match.
const MAX_QUEUED: usize = 4_000;

/// Seeds remembered after their match has been counted.
///
/// Dedup used to be free: an already-counted match was still in the file, so
/// `by_seed` answered it. Now that a counted match is dropped, the seed has to be
/// remembered on its own, or watching a presimmed match play out would capture
/// and count it a second time.
///
/// A ring rather than the full set, because the risk it covers is immediate — a
/// replay or a live view of a match simmed moments ago. Nothing re-runs a seed
/// from three seasons back.
const MAX_COUNTED: usize = 4_000;

/// Ticks between roster top-ups.
///
/// This is the only work the module does inside the simulation loop, and it used
/// to run every tick of every sim — a process-wide lock taken 60 times a second
/// per match, with presims arriving in batches. Half a second between passes cuts
/// that by thirty and still catches every champion, because a champion that is
/// alive at all is alive for far longer than that.
const ROSTER_EVERY: usize = 30;

/// One champion's finished loadout.
#[derive(Clone)]
pub(crate) struct CapturedPlayer {
    pub champion: String,
    pub items: Vec<String>,
    pub won: bool,
    /// [`LaneV1`] as its code, 0..=4 for top/jungle/mid/bottom/support.
    ///
    /// `None` only if the host declines to answer, which no normal match does.
    /// Such a player still counts in the unfiltered table rather than being
    /// dropped — the loadout is real either way, it just cannot be placed.
    pub lane: Option<usize>,
}

/// Matches captured but not yet counted, and the seeds of those that have been.
///
/// A queue entry carries no patch: the simulation has no idea what patch it is
/// running under, only the match record knows, and that is read much later. So an
/// entry sits here until [`take`] hands it over with the patch attached.
#[derive(Default)]
struct Queue {
    by_seed: BTreeMap<u64, Vec<CapturedPlayer>>,
    /// Insertion order, so the oldest can be evicted. A `BTreeMap` is ordered by
    /// seed, which says nothing about when a match was played.
    order: VecDeque<u64>,
    /// Seeds already folded into the totals, oldest first, for eviction order.
    counted: VecDeque<u64>,
    /// The same seeds, for lookup.
    ///
    /// Kept beside the queue rather than scanning it: `seen` is asked on every
    /// tick once a match has ended, since `is_end` stays true for the rest of the
    /// sim, and a linear walk of [`MAX_COUNTED`] seeds there would be a tax on the
    /// simulation loop — the one place this module must not cost anything.
    counted_set: BTreeSet<u64>,
}

impl Queue {
    /// Whether this seed has been captured, whether or not it has been counted.
    fn seen(&self, seed: u64) -> bool {
        self.by_seed.contains_key(&seed) || self.counted_set.contains(&seed)
    }

    /// Remembers a seed as counted, dropping the oldest once full.
    fn mark_counted(&mut self, seed: u64) {
        if !self.counted_set.insert(seed) {
            return;
        }
        self.counted.push_back(seed);
        while self.counted.len() > MAX_COUNTED {
            if let Some(oldest) = self.counted.pop_front() {
                self.counted_set.remove(&oldest);
            }
        }
    }
}

static QUEUE: Mutex<Option<Queue>> = Mutex::new(None);

/// Champion names by player index, taken at match start and keyed by seed.
///
/// They have to be read then, not at the end: `StablePlayer::champion` resolves
/// a live entity, and a champion that is dead on the final tick no longer has
/// one. Reading at the end gave a name only for the survivors, which showed up
/// as most rows having no "purchased on" portraits at all.
static ROSTERS: Mutex<Option<BTreeMap<u64, Vec<String>>>> = Mutex::new(None);

/// Fills in any roster entry still unknown, from whoever is resolvable now.
///
/// # Why the start-of-match roster was not enough
///
/// `on_match_start` runs before the champions exist, so it records ten empty
/// strings, and the end-of-match fallback — `player.champion()`, which resolves
/// a *live* entity — is what actually supplied the names. That only works for
/// whoever is still standing on the final tick, which is why 72% of losing
/// players were captured nameless against 14% of winners: the losing side is
/// dead when the match ends. Their items were recorded either way, so the item
/// totals were right, but the "purchased on" column had nothing to draw.
///
/// Topping up as the match runs fixes it at the source, because every champion
/// is alive on *some* tick — including everyone who is dead by the last one.
///
/// The cost decays to nothing. Only entries that are still blank are read, so
/// once a roster is complete this is one pass over ten strings per tick.
fn top_up_roster(sim: &mut StableSim<'_>) {
    // Twice a second is enough: see [`ROSTER_EVERY`]. Taken off the sim's own
    // tick rather than a counter of our own, so batched presims each throttle
    // independently instead of sharing one global phase — and so it costs no
    // state, which a counter would have had to lock to reach.
    //
    // This assumes `on_match_tick` arrives once per tick, so every value comes
    // round. It does today: the proc queue's stagger arithmetic depends on the
    // same thing. A host that called it every other tick could land only on odd
    // values and skip the pass entirely, which would show up as the "purchased
    // on" column going empty again.
    if sim.tick() % ROSTER_EVERY != 0 {
        return;
    }
    let seed = sim.seed();
    let count = sim.player_count();

    // Which seats still need a name. The lock is taken to answer that and then
    // released before any of them are resolved: `player_at` crosses the ABI,
    // and presims arrive in batches, so holding a process-wide lock across those
    // calls would serialise sims that have nothing to do with each other.
    let missing: Vec<usize> = {
        let Ok(mut guard) = ROSTERS.lock() else {
            return;
        };
        let rosters = guard.get_or_insert_with(BTreeMap::new);
        // Created here when `on_match_start` never ran for this sim, under the
        // same bound that function applies — this is a second way into the map.
        if !rosters.contains_key(&seed) {
            if rosters.len() >= MAX_ROSTERS {
                if let Some(&oldest) = rosters.keys().next() {
                    rosters.remove(&oldest);
                }
            }
            rosters.insert(seed, vec![String::new(); count]);
        }
        let Some(roster) = rosters.get_mut(&seed) else {
            return;
        };
        roster.resize(count, String::new());
        roster
            .iter()
            .enumerate()
            .filter(|(_, name)| name.is_empty())
            .map(|(index, _)| index)
            .collect()
    };
    if missing.is_empty() {
        return;
    }

    let found: Vec<(usize, String)> = missing
        .into_iter()
        .filter_map(|index| {
            let name = sim
                .player_at(index)
                .and_then(|player| player.champion())
                .and_then(|champion| champion.name())
                .filter(|name| !name.is_empty())?;
            Some((index, name))
        })
        .collect();
    if found.is_empty() {
        return;
    }

    if let Ok(mut guard) = ROSTERS.lock() {
        if let Some(roster) = guard.as_mut().and_then(|rosters| rosters.get_mut(&seed)) {
            for (index, name) in found {
                // Only ever fills a gap. The entry may have been written while
                // the lock was down, and the earlier answer is no worse.
                if let Some(slot) = roster.get_mut(index).filter(|slot| slot.is_empty()) {
                    *slot = name;
                }
            }
        }
    }
}

/// Matches whose roster is remembered while they play. A sim that somehow never
/// reaches its end tick would otherwise leak an entry forever.
const MAX_ROSTERS: usize = 512;

fn with_queue<T>(f: impl FnOnce(&mut Queue) -> T) -> Option<T> {
    let mut guard = QUEUE.lock().ok()?;
    Some(f(guard.get_or_insert_with(Queue::default)))
}

/// How many captures are waiting to be counted.
///
/// The client folds whenever this is non-zero, which is what keeps the queue a
/// buffer of seconds rather than the season-long backlog it used to be. See
/// [`crate::item_stats::sync`].
pub(crate) fn pending() -> usize {
    with_queue(|queue| queue.by_seed.len()).unwrap_or(0)
}

/// Hands over a captured match to be counted, and remembers that it was.
///
/// The entry is removed as it is returned: once the caller has folded it into the
/// totals the loadouts have served their purpose, and keeping them is the cost
/// this module exists to avoid. The seed stays behind so the same match cannot be
/// captured again — see [`MAX_COUNTED`].
pub(crate) fn take(seed: u64) -> Option<Vec<CapturedPlayer>> {
    let taken = with_queue(|queue| {
        let players = queue.by_seed.remove(&seed)?;
        queue.order.retain(|queued| *queued != seed);
        queue.mark_counted(seed);
        Some(players)
    })
    .flatten();

    taken
}

/// The match hook. Registered for every match the game simulates.
pub(crate) struct EndOfMatchItems;

impl StableMatchHook for EndOfMatchItems {
    fn on_match_start(&self, sim: &mut StableSim<'_>) {
        let seed = sim.seed();
        let roster: Vec<String> = (0..sim.player_count())
            .map(|index| {
                sim.player_at(index)
                    .and_then(|player| player.champion())
                    .and_then(|champion| champion.name())
                    .unwrap_or_default()
            })
            .collect();

        if let Ok(mut guard) = ROSTERS.lock() {
            let rosters = guard.get_or_insert_with(BTreeMap::new);
            if rosters.len() >= MAX_ROSTERS {
                // Oldest by seed is arbitrary, but so is any other order here —
                // the point is only that the map cannot grow without bound.
                if let Some(&oldest) = rosters.keys().next() {
                    rosters.remove(&oldest);
                }
            }
            rosters.insert(seed, roster);
        }
    }

    fn on_match_tick(&self, sim: &mut StableSim<'_>, _rng_seed: u64) {
        if !sim.is_end() {
            top_up_roster(sim);
            return;
        }

        // `is_end` stays true for the rest of the sim, and a replay or a
        // client-side view of an already-simmed match arrives with the same
        // seed. First capture wins, so both are no-ops rather than double
        // counts — which is also why this does not need to filter on
        // `sim_origin`, and can therefore keep matches the player watches live.
        let seed = sim.seed();
        if with_queue(|queue| queue.seen(seed)).unwrap_or(true) {
            return;
        }

        let roster = ROSTERS
            .lock()
            .ok()
            .and_then(|mut guard| guard.as_mut().and_then(|rosters| rosters.remove(&seed)))
            .unwrap_or_default();

        let mut players = Vec::new();
        for index in 0..sim.player_count() {
            let Some(player) = sim.player_at(index) else {
                continue;
            };
            let items = player.item_keys();
            if items.is_empty() {
                continue;
            }
            let team = player.team();
            let lane = player.lane().map(|lane| lane.code() as usize);
            players.push(CapturedPlayer {
                // The start-of-match roster first; the live entity only as a
                // fallback, for a host that never called `on_match_start`.
                champion: roster
                    .get(index)
                    .filter(|name| !name.is_empty())
                    .cloned()
                    .or_else(|| player.champion().and_then(|champion| champion.name()))
                    .unwrap_or_default(),
                items,
                // At the final tick the side that is ahead is the side that won.
                won: sim.score_diff(team) > 0,
                lane,
            });
        }

        if players.is_empty() {
            return;
        }

        let _ = with_queue(|queue| {
            queue.by_seed.insert(seed, players);
            queue.order.push_back(seed);
            while queue.order.len() > MAX_QUEUED {
                if let Some(oldest) = queue.order.pop_front() {
                    queue.by_seed.remove(&oldest);
                }
            }
        });
    }
}

pub(crate) fn forget() {
    if let Ok(mut guard) = QUEUE.lock() {
        *guard = None;
    }
}

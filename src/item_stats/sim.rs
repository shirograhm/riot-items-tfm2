//! End-of-match item loadouts captured from the simulation, because a match
//! record's `items` is the planned build, not what the champion finished with.
//!
//! Captures are joined to records by rng seed to get the patch; server pre-sims
//! the seed cannot place carry a [`Fixture`] so the server can place them
//! instead. A capture is only an in-memory queue entry until it is folded into
//! the save's totals, so a new statistic fills in only from matches simmed
//! after it is added.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ops::Bound;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use mod_api_stable::*;

// Captures waiting to be counted. The cap stops a save that never opens the tab
// from growing without bound; the oldest is dropped.
const MAX_QUEUED: usize = 4_000;

// Seeds already counted, so watching a presimmed match play out does not count
// it twice. A ring, since the risk is only a replay of a recent match.
const MAX_COUNTED: usize = 4_000;

// Sets the server placed lately (see [`Queue::vouched`]). Short, because match
// and replay ids are re-used over a long save.
const MAX_VOUCHED: usize = 512;

// Ticks between roster top-ups: twice a second still catches every champion and
// keeps the process-wide lock out of most sim ticks.
const ROSTER_EVERY: usize = 30;

#[derive(Clone)]
pub(crate) struct CapturedPlayer {
    pub champion: String,
    pub items: Vec<String>,
    pub won: bool,
    // [`LaneV1`] code, 0..=4. `None` only if the host declines to answer; such
    // a player still counts in the unfiltered table.
    pub lane: Option<usize>,
}

// Where a server pre-sim sits in the server's match table.
#[derive(Clone, Copy)]
pub(crate) struct Fixture {
    // The match's id, and the index of this set in it.
    pub set: Option<(u64, u64)>,
    // The set's replay record, where the simulation already names one.
    pub replay: Option<u64>,
}

impl Fixture {
    // `None` unless this is a server pre-sim naming its match or replay:
    // replays, spectated matches and tool runs must never be counted.
    fn of(origin: SimOriginV1) -> Option<Self> {
        if !matches!(
            SimOriginKindV1::from_code(origin.kind),
            Some(SimOriginKindV1::ServerPresim)
        ) {
            return None;
        }
        let known = |id: u64| (id != SimOriginV1::NONE).then_some(id);
        let fixture = Self {
            set: known(origin.match_id).zip(known(origin.set_index)),
            replay: known(origin.replay_id),
        };
        (fixture.set.is_some() || fixture.replay.is_some()).then_some(fixture)
    }
}

// Captures not yet counted, and the seeds of those that were. Entries wait here
// because only the match record, read later, knows the patch.
#[derive(Default)]
struct Queue {
    by_seed: BTreeMap<u64, Vec<CapturedPlayer>>,
    // Insertion order, for eviction. Only [`MAX_QUEUED`] evicts: records are
    // written when a game day is committed, often minutes later, so an age
    // limit would lose captures.
    order: VecDeque<u64>,
    // Seeds already folded into the totals, oldest first, for eviction order.
    counted: VecDeque<u64>,
    // The same seeds for lookup: `seen` runs every tick after a match ends, so
    // it must not walk the ring.
    counted_set: BTreeSet<u64>,
    // Seed -> where the set sits in its match table, for the server; removed
    // when the capture is placed, counted or evicted.
    fixtures: BTreeMap<u64, Fixture>,
    // Where [`unplaced`] left off, so a set whose record never comes cannot
    // hold up the rest.
    cursor: u64,
    // Seed -> patch for captures the server has placed, waiting for the client
    // to fold them.
    placed: BTreeMap<u64, String>,
    // Recently placed (set, replay) pairs: a second simulation of a set runs on
    // another seed, so only this spots the repeat.
    vouched: VecDeque<(Option<(u64, u64)>, u64)>,
}

impl Queue {
    // Whether this seed has been captured, whether or not it has been counted.
    fn seen(&self, seed: u64) -> bool {
        self.by_seed.contains_key(&seed) || self.counted_set.contains(&seed)
    }

    // Remembers a seed as counted, dropping the oldest once full.
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

// Champion names by player index, keyed by seed. They must be read while the
// champions live: `StablePlayer::champion` resolves a live entity, so the dead
// have no name at the end.
static ROSTERS: Mutex<Option<BTreeMap<u64, Vec<String>>>> = Mutex::new(None);

// Fills in roster names still unknown from whoever is alive now.
// `on_match_start` runs before champions exist, but every champion is alive on
// some tick; once complete this is one pass over ten strings.
fn top_up_roster(sim: &mut StableSim<'_>) {
    // Throttled on the sim's own tick (see [`ROSTER_EVERY`]), so batched
    // presims throttle independently. Assumes `on_match_tick` arrives every
    // tick.
    if sim.tick() % ROSTER_EVERY != 0 {
        return;
    }
    let seed = sim.seed();
    let count = sim.player_count();

    // Find the missing seats under the lock, then resolve them without it:
    // `player_at` crosses the ABI, and holding a process-wide lock would
    // serialise batched presims.
    let missing: Vec<usize> = {
        let Ok(mut guard) = ROSTERS.lock() else {
            return;
        };
        let rosters = guard.get_or_insert_with(BTreeMap::new);
        // Created here when `on_match_start` never ran for this sim, under the
        // same bound.
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
                // Only fills gaps; an entry written while the lock was down is
                // no worse.
                if let Some(slot) = roster.get_mut(index).filter(|slot| slot.is_empty()) {
                    *slot = name;
                }
            }
        }
    }
}

// Bound on rosters held for running matches, so a sim that never ends cannot
// leak one.
const MAX_ROSTERS: usize = 512;

fn with_queue<T>(f: impl FnOnce(&mut Queue) -> T) -> Option<T> {
    let mut guard = QUEUE.lock().ok()?;
    Some(f(guard.get_or_insert_with(Queue::default)))
}

// How many captures are waiting; the client folds whenever this is non-zero
// (see [`crate::item_stats::sync`]).
pub(crate) fn pending() -> usize {
    with_queue(|queue| queue.by_seed.len()).unwrap_or(0)
}

// Bumped on every capture, so `item_stats::sync` re-reads records only when a
// capture has arrived since its last sweep.
static CAPTURES: AtomicU64 = AtomicU64::new(0);

pub(crate) fn captures() -> u64 {
    CAPTURES.load(Ordering::Relaxed)
}

// Hands over a captured match to be counted and drops it, keeping only the seed
// so it cannot be captured again (see [`MAX_COUNTED`]).
pub(crate) fn take(seed: u64) -> Option<Vec<CapturedPlayer>> {
    let taken = with_queue(|queue| {
        let players = queue.by_seed.remove(&seed)?;
        queue.order.retain(|queued| *queued != seed);
        queue.fixtures.remove(&seed);
        queue.placed.remove(&seed);
        queue.mark_counted(seed);
        Some(players)
    })
    .flatten();

    taken
}

// The next `limit` waiting captures the server has yet to find the patch of.
pub(crate) fn unplaced(limit: usize) -> Vec<(u64, Fixture)> {
    with_queue(|queue| {
        let after = (Bound::Excluded(queue.cursor), Bound::Unbounded);
        let next: Vec<(u64, Fixture)> = queue
            .fixtures
            .range(after)
            .chain(queue.fixtures.range(..=queue.cursor))
            .take(limit)
            .map(|(seed, fixture)| (*seed, *fixture))
            .collect();
        if let Some((seed, _)) = next.last() {
            queue.cursor = *seed;
        }
        next
    })
    .unwrap_or_default()
}

// Files a waiting capture under the patch the server found for it, for the
// client to fold. `replay` is the record that named the patch.
pub(crate) fn place(seed: u64, replay: u64, patch: String) {
    let _ = with_queue(|queue| {
        // Gone if it was counted or evicted while the server looked. Removed
        // either way: it is placed now, or it repeats a set that was.
        let Some(fixture) = queue.fixtures.remove(&seed) else {
            return;
        };
        let vouch = (fixture.set, replay);
        if queue.vouched.contains(&vouch) {
            return;
        }
        queue.vouched.push_back(vouch);
        while queue.vouched.len() > MAX_VOUCHED {
            queue.vouched.pop_front();
        }
        queue.placed.insert(seed, patch);
    });
}

// Hands over every capture the server has placed, each with its patch, and
// remembers that it did: [`take`], for the captures no record's seed asks for.
pub(crate) fn take_placed() -> Vec<(String, Vec<CapturedPlayer>)> {
    with_queue(|queue| {
        if queue.placed.is_empty() {
            return Vec::new();
        }
        let mut taken = Vec::new();
        for (seed, patch) in std::mem::take(&mut queue.placed) {
            if let Some(players) = queue.by_seed.remove(&seed) {
                queue.mark_counted(seed);
                taken.push((patch, players));
            }
        }
        let Queue { order, by_seed, .. } = queue;
        order.retain(|queued| by_seed.contains_key(queued));
        taken
    })
    .unwrap_or_default()
}

// Match hook, registered for every match the game simulates.
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
                // Any eviction order will do; this only bounds the map.
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

        // `is_end` stays true for the rest of the sim, and replays of a simmed
        // match reuse its seed. First capture wins, so matches watched live
        // need no `sim_origin` filter.
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
                // fallback.
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

        // What lets the server place this set when no record carries its seed.
        let fixture = sim.sim_origin().and_then(Fixture::of);

        let _ = with_queue(|queue| {
            queue.by_seed.insert(seed, players);
            if let Some(fixture) = fixture {
                queue.fixtures.insert(seed, fixture);
            }
            queue.order.push_back(seed);
            while queue.order.len() > MAX_QUEUED {
                if let Some(oldest) = queue.order.pop_front() {
                    queue.by_seed.remove(&oldest);
                    queue.fixtures.remove(&oldest);
                    queue.placed.remove(&oldest);
                }
            }
        });
        CAPTURES.fetch_add(1, Ordering::Relaxed);
        crate::perf::count(crate::perf::Section::CaptureQueued);
    }
}

pub(crate) fn forget() {
    if let Ok(mut guard) = QUEUE.lock() {
        *guard = None;
    }
}

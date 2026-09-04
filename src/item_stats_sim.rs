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
//! # Why it is written to a file
//!
//! Captures happen during presims; the table is read much later, often in
//! another session. Held only in memory they would be lost on every restart and
//! the table would reset to empty each launch — which looks exactly like the
//! feature not working. The file lives next to the DLL, the way
//! `item-builds.json` does, and is keyed by seed so two saves cannot corrupt
//! each other's numbers: a seed from one simply never matches the other's
//! records.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use mod_api_stable::*;

/// Matches kept before the oldest are dropped.
///
/// A season is on the order of a hundred matches, so this is years of history at
/// a few hundred KB. The cap exists so a save played indefinitely cannot grow
/// the file without bound.
const MAX_MATCHES: usize = 20_000;

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

/// One match: the loadouts, and the patch it was played on once a record has
/// been seen for it.
///
/// The patch is backfilled rather than captured because the simulation has no
/// idea what patch it is running under — only the match record knows, and it is
/// read much later. Storing it here means a match keeps its patch even after the
/// game prunes the record it came from.
#[derive(Default)]
pub(crate) struct Match {
    pub patch: Option<String>,
    pub players: Vec<CapturedPlayer>,
}

#[derive(Default)]
struct Captures {
    by_seed: BTreeMap<u64, Match>,
    /// Insertion order, so the oldest can be evicted. A `BTreeMap` is ordered by
    /// seed, which says nothing about when a match was played.
    order: VecDeque<u64>,
    loaded: bool,
}

static CAPTURES: Mutex<Option<Captures>> = Mutex::new(None);

/// Bumped whenever the captures change in a way that alters the totals — a new
/// match, or a patch backfilled onto an old one. The table folds itself again
/// when this moves, which is what keeps it a pure function of what is on file
/// rather than a running tally that has to be kept in step by hand.
static REVISION: AtomicU64 = AtomicU64::new(0);

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
static DIRTY: AtomicBool = AtomicBool::new(false);

fn with_captures<T>(f: impl FnOnce(&mut Captures) -> T) -> Option<T> {
    let mut guard = CAPTURES.lock().ok()?;
    let captures = guard.get_or_insert_with(Captures::default);
    if !captures.loaded {
        captures.loaded = true;
        load_into(captures);
        if !captures.by_seed.is_empty() {
            // The totals are rebuilt when this moves, and reading a file full of
            // matches changes them as surely as playing one does. Without this
            // the revision sits at its initial value, matches the freshly
            // defaulted aggregate, and a session that loads history folds none
            // of it — an empty table on top of a full file.
            REVISION.fetch_add(1, Ordering::Relaxed);
        }
    }
    Some(f(captures))
}

/// Whether a match was simmed while this was running.
pub(crate) fn has(seed: u64) -> bool {
    with_captures(|captures| captures.by_seed.contains_key(&seed)).unwrap_or(false)
}

/// Attaches the patch a match was played on, learned from its record.
pub(crate) fn set_patch(seed: u64, patch: &str) {
    let changed = with_captures(|captures| {
        let Some(entry) = captures.by_seed.get_mut(&seed) else {
            return false;
        };
        if entry.patch.as_deref() == Some(patch) {
            return false;
        }
        entry.patch = Some(patch.to_string());
        true
    })
    .unwrap_or(false);

    if changed {
        REVISION.fetch_add(1, Ordering::Relaxed);
        DIRTY.store(true, Ordering::Relaxed);
    }
}

/// The revision the totals were last folded at.
pub(crate) fn revision() -> u64 {
    REVISION.load(Ordering::Relaxed)
}

/// Every captured match, for the fold. Takes a callback rather than returning a
/// collection because this runs over every match on file.
pub(crate) fn for_each(mut f: impl FnMut(Option<&str>, &[CapturedPlayer])) {
    let _ = with_captures(|captures| {
        for entry in captures.by_seed.values() {
            f(entry.patch.as_deref(), &entry.players);
        }
    });
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
        if with_captures(|captures| captures.by_seed.contains_key(&seed)).unwrap_or(true) {
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

        let _ = with_captures(|captures| {
            captures.by_seed.insert(
                seed,
                Match {
                    patch: None,
                    players,
                },
            );
            captures.order.push_back(seed);
            while captures.order.len() > MAX_MATCHES {
                if let Some(oldest) = captures.order.pop_front() {
                    captures.by_seed.remove(&oldest);
                }
            }
        });
        REVISION.fetch_add(1, Ordering::Relaxed);
        DIRTY.store(true, Ordering::Relaxed);
    }
}

/// Writes the captures out if any have been added since the last call.
///
/// Called from the management tick rather than from the capture itself: presims
/// arrive in batches of dozens as a season advances, and a file write per match
/// would put disk IO inside the simulation loop.
pub(crate) fn flush() {
    if !DIRTY.swap(false, Ordering::Relaxed) {
        return;
    }
    let Some(text) = with_captures(serialise) else {
        return;
    };
    let Some(path) = crate::config::dll_dir().map(|dir| dir.join(FILE)) else {
        return;
    };
    // Written whole rather than appended: the map is the truth and a partial
    // append after a crash would be a file that no longer parses.
    let _ = std::fs::write(path, text);
}

const FILE: &str = "item-stats-builds.json";

/// The capture format this build writes and is willing to read.
///
/// Bumped to 2 when the lane was added to each player. A file written before
/// that is **discarded rather than migrated**: its matches have no lane, so
/// every one of them would sit outside whatever the lane filter is set to and
/// silently under-report — the table would disagree with itself depending on
/// which dropdown you touched. Starting the history over is the honest option,
/// and it is what "no stats should be retroactive" asks for.
const FORMAT: u32 = 2;

/// `{"v": 2, "m": {"<seed>": {"p": patch, "l": [{"c", "w", "i", "n"}, ...]}}}`
///
/// Hand-rolled rather than derived: the mod's `serde` is not wired up for these
/// types, the shape is five fields, and keeping it terse matters when the file
/// holds thousands of matches — `n` rather than `lane` is worth roughly half a
/// megabyte across a full history.
///
/// `v` is what makes a format change safe: see [`FORMAT`].
fn serialise(captures: &mut Captures) -> String {
    let mut out = format!("{{\"v\":{FORMAT},\"m\":{{");
    let mut first = true;
    for seed in captures.order.iter() {
        let Some(entry) = captures.by_seed.get(seed) else {
            continue;
        };
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(&format!("\"{seed}\":{{"));
        if let Some(patch) = &entry.patch {
            out.push_str(&format!("\"p\":{},", quote(patch)));
        }
        out.push_str("\"l\":[");
        for (slot, player) in entry.players.iter().enumerate() {
            if slot > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"c\":{},\"w\":{},",
                quote(&player.champion),
                player.won
            ));
            if let Some(lane) = player.lane {
                out.push_str(&format!("\"n\":{lane},"));
            }
            out.push_str("\"i\":[");
            for (position, item) in player.items.iter().enumerate() {
                if position > 0 {
                    out.push(',');
                }
                out.push_str(&quote(item));
            }
            out.push_str("]}");
        }
        out.push_str("]}");
    }
    out.push_str("}}");
    out
}

fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn load_into(captures: &mut Captures) {
    let Some(path) = crate::config::dll_dir().map(|dir| dir.join(FILE)) else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(serde_json::Value::Object(file)) = serde_json::from_str::<serde_json::Value>(&text)
    else {
        return;
    };

    // Anything this build did not write is not read. The older shapes — a bare
    // array per seed, and the unversioned object that followed it — carry no
    // lane, and there is nothing to infer one from.
    if file.get("v").and_then(serde_json::Value::as_u64) != Some(FORMAT as u64) {
        return;
    }
    let Some(serde_json::Value::Object(root)) = file.get("m").cloned() else {
        return;
    };

    for (seed, value) in root {
        let Ok(seed) = seed.parse::<u64>() else {
            continue;
        };
        let serde_json::Value::Object(fields) = &value else {
            continue;
        };
        let patch = fields
            .get("p")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let serde_json::Value::Array(players) = fields.get("l").cloned().unwrap_or_default() else {
            continue;
        };

        let loadouts: Vec<CapturedPlayer> = players
            .iter()
            .filter_map(|player| {
                let fields = player.as_object()?;
                let items = fields
                    .get("i")?
                    .as_array()?
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_string))
                    .collect::<Vec<_>>();
                (!items.is_empty()).then(|| CapturedPlayer {
                    champion: fields
                        .get("c")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    items,
                    won: fields
                        .get("w")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                    lane: fields
                        .get("n")
                        .and_then(serde_json::Value::as_u64)
                        .map(|lane| lane as usize),
                })
            })
            .collect();
        if loadouts.is_empty() {
            continue;
        }
        captures.by_seed.insert(
            seed,
            Match {
                patch,
                players: loadouts,
            },
        );
        captures.order.push_back(seed);
    }
}

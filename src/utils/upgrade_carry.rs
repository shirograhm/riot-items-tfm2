//! Carries an item's state into the instance that replaces it on an upgrade.
//!
//! `on_upgrade` / `on_upgraded_from` are meant for this, and the host does not
//! call them: logged in game on 2026-10-02 (0.6.2), neither fired across three
//! sessions of upgrades, and every Radiant item arrived holding nothing.
//!
//! So an item notes its state here whenever it changes, under its upgrade line
//! (the base item's key), the match seed and the player. The instance that
//! replaces it reads the latest note back, once.
//!
//! A seed is not one run of a match. The same logs show a seed played on four
//! threads at once: sometimes tick for tick the same, sometimes the same up to
//! a point and then apart. So a note also carries the match's history when it
//! was made (its kills so far, as a hash), and an instance takes only notes
//! from before its own tick whose history is its own. Those are notes it made
//! itself, or ones a run identical up to there made first. Notes of one tick
//! are a set and the largest value wins, so the answer never depends on which
//! run wrote last.

use std::collections::HashMap;
use std::sync::Mutex;

use mod_api_stable::{KillLogV1, StableSim};

/// (upgrade line, match seed, player).
type Key = (&'static str, u64, usize);
/// (tick, value, history before that tick), in no order.
type Notes = Vec<(usize, u64, u64)>;

/// Two generations, so a long session forgets finished matches without ever
/// dropping one that is still being played: a carrier noted again moves to
/// `current`, and `previous` is only let go a full generation later.
#[derive(Default)]
struct Table {
    current: HashMap<Key, Notes>,
    previous: HashMap<Key, Notes>,
}

static TABLE: Mutex<Option<Table>> = Mutex::new(None);
/// Carriers in a generation.
const GENERATION_CAP: usize = 2048;

/// The match's kills, oldest first.
fn kills(ctx: &StableSim<'_>) -> Vec<KillLogV1> {
    (0..ctx.kill_log_count())
        .filter_map(|index| ctx.kill_log_at(index))
        .collect()
}

/// The kills before `tick` as one hash (FNV-1a over each kill's fields). Kills
/// of `tick` itself are left out: whether one is logged yet depends on where
/// in the tick the caller runs.
fn history(kills: &[KillLogV1], tick: usize) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut fold = |value: u64| {
        for byte in value.to_le_bytes() {
            hash = (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for kill in kills.iter().filter(|kill| kill.tick < tick) {
        fold(kill.tick as u64);
        fold(kill.killer_team as u64);
        fold(kill.killer_position as u64);
        fold(kill.killed_position as u64);
        fold(kill.assist_count as u64);
        for &position in kill.assist_positions.iter().take(kill.assist_count as usize) {
            fold(position as u64);
        }
    }
    hash
}

/// Notes `value` as the state of `player`'s item on `line` at this tick.
pub(crate) fn note(line: &'static str, ctx: &StableSim<'_>, player: usize, value: u64) {
    let key = (line, ctx.seed(), player);
    let tick = ctx.tick();
    let note = (tick, value, history(&kills(ctx), tick));
    let Ok(mut table) = TABLE.lock() else {
        return;
    };
    let table = table.get_or_insert_with(Table::default);
    if !table.current.contains_key(&key) {
        if table.current.len() >= GENERATION_CAP {
            table.previous = std::mem::take(&mut table.current);
        }
        let notes = table.previous.remove(&key).unwrap_or_default();
        table.current.insert(key, notes);
    }
    let notes = table.current.entry(key).or_default();
    if !notes.contains(&note) {
        notes.push(note);
    }
}

/// The latest note for `player`'s item on `line` from before this tick and
/// from this run's own history, as (tick, value): what the instance that
/// replaces the item takes over.
pub(crate) fn latest(
    line: &'static str,
    ctx: &StableSim<'_>,
    player: usize,
) -> Option<(usize, u64)> {
    let key = (line, ctx.seed(), player);
    let tick = ctx.tick();
    let mut notes: Notes = {
        let table = TABLE.lock().ok()?;
        let table = table.as_ref()?;
        let found = table.current.get(&key).or_else(|| table.previous.get(&key))?;
        found.iter().copied().filter(|note| note.0 < tick).collect()
    };
    // Latest first; tuples order by tick, then value.
    notes.sort_unstable_by(|a, b| b.cmp(a));
    let kills = kills(ctx);
    notes
        .into_iter()
        .find(|&(noted_at, _, noted_history)| history(&kills, noted_at) == noted_history)
        .map(|(noted_at, value, _)| (noted_at, value))
}

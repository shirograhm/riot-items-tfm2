//! What an item has built up over a match, as a number on its icon in the
//! in-match player rows.
//!
//! Counting items report from their `update` ([`note`], [`note_with`]);
//! fight-scoped counts ([`note_in_combat`]) show only during a fight and are
//! zeroed on death by [`on_match_tick`]. Only the simulation the client shows
//! is recorded.
//!
//! That simulation runs ahead and may replay the match from the start, so every
//! count and holding is kept by tick and drawn as of the tick the match clock
//! reads ([`shown_tick`]); a rewind drops what was kept from that tick on.
//! Nothing is drawn where the clock cannot be read.
//!
//! [`sync`] spawns a `count` label into each game-spawned item slot
//! (`item_spend::slot_nodes`), again whenever game code respawns the slot; zero
//! counts, spectated matches and the result screen show nothing. With
//! `match_builds::LOG` on, each step is logged to `match-builds.log` under
//! `counts.`.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use mod_api_stable::{BuffV1, StableClient, StableSim};

use super::item_spend::{self, HALVES, ROW_HOSTS};
use super::match_builds::{self, LANES};
use crate::TICKS_PER_SECOND;

// -- the match as it went -----------------------------------------------------

// Sim ticks between two looks at the counts and at what is held: four a
// second.
const NOTE_EVERY: usize = 15;

// One athlete of the match on screen as the simulation has played it: each
// a list of changes, oldest first, by the tick they were seen on.
#[derive(Default)]
struct Seat {
    // The items held, in slot order.
    held: Vec<(usize, Vec<String>)>,
    // Each counting item's count.
    counts: Vec<Count>,
}

struct Count {
    key: &'static str,
    // A count of the fight the carrier is in, which a death ends.
    in_combat: bool,
    changes: Vec<(usize, u64)>,
}

impl Count {
    // Notes `value` at `tick`, where it is not what the count already says.
    fn change(&mut self, tick: usize, value: u64) {
        if self.changes.last().map(|&(_, last)| last) != Some(value) {
            self.changes.push((tick, value));
        }
    }
}

struct Counts {
    seed: u64,
    // The tick the simulation was last seen on.
    tick: usize,
    // By side (0 blue, 1 red) and by lane.
    seats: [[Seat; LANES]; 2],
}

impl Counts {
    // Lets go of everything seen from `tick` on.
    fn forget_from(&mut self, tick: usize) {
        for seat in self.seats.iter_mut().flatten() {
            seat.held.retain(|(at, _)| *at < tick);
            for count in &mut seat.counts {
                count.changes.retain(|(at, _)| *at < tick);
            }
        }
    }
}

static COUNTS: Mutex<Option<Counts>> = Mutex::new(None);

// Writes into the match of `seed` as the simulation has it at `tick`: one
// first seen starts empty, and one that has gone back to `tick` from later
// is played again from there.
fn record(seed: u64, tick: usize, write: impl FnOnce(&mut Counts)) {
    let Ok(mut counts) = COUNTS.lock() else {
        return;
    };
    if counts.as_ref().is_none_or(|counts| counts.seed != seed) {
        *counts = Some(Counts {
            seed,
            tick,
            seats: Default::default(),
        });
    }
    let Some(counts) = counts.as_mut() else {
        return;
    };
    if tick < counts.tick {
        counts.forget_from(tick);
    }
    counts.tick = tick;
    write(counts);
}

// Notes what the on-screen match's athletes hold and ends the dead's in-combat
// counts. From the match hook on every tick of every simulation; it works only
// every [`NOTE_EVERY`]th tick of the client's own.
pub(crate) fn on_match_tick(sim: &StableSim<'_>) {
    let tick = sim.tick();
    if tick % NOTE_EVERY != 0 || !item_spend::on_screen(sim) {
        return;
    }
    // (side, lane, what the athlete holds, whether their champion is alive)
    let mut seen = Vec::new();
    for index in 0..sim.player_count() {
        let Some(player) = sim.player_at(index) else {
            continue;
        };
        let side = player.team();
        let Some(lane) = player
            .lane()
            .map(|lane| lane.code() as usize)
            .filter(|lane| *lane < LANES)
        else {
            continue;
        };
        if side < 2 {
            let alive = player.champion().is_some_and(|champion| champion.is_alive());
            seen.push((side, lane, player.item_keys(), alive));
        }
    }
    record(sim.seed(), tick, |counts| {
        for (side, lane, keys, alive) in seen {
            let seat = &mut counts.seats[side][lane];
            if seat.held.last().map(|(_, last)| last) != Some(&keys) {
                seat.held.push((tick, keys));
            }
            if !alive {
                for count in seat.counts.iter_mut().filter(|count| count.in_combat) {
                    count.change(tick, 0);
                }
            }
        }
    });
}

// Notes what `player`'s item `key` stands at, from its `update`; it works only
// every [`NOTE_EVERY`]th tick of the client's own simulation.
pub(crate) fn note(ctx: &StableSim<'_>, player: usize, key: &'static str, count: u64) {
    note_as(ctx, player, key, false, || count);
}

// Notes a count of the fight the carrier is in, which a death ends. `count` is
// only asked for on the ticks a note is taken.
pub(crate) fn note_in_combat(
    ctx: &StableSim<'_>,
    player: usize,
    key: &'static str,
    count: impl FnOnce() -> u64,
) {
    note_as(ctx, player, key, true, count);
}

// [`note`] for a count that has to be worked out: `count` is only asked for
// on the ticks a note is taken.
pub(crate) fn note_with(
    ctx: &StableSim<'_>,
    player: usize,
    key: &'static str,
    count: impl FnOnce() -> u64,
) {
    note_as(ctx, player, key, false, count);
}

// What the copies of the buff `name` the champion of `player` wears add up
// to in one stat, `value` picking it out of each: Hubris's Eminence, a copy
// a takedown, each its own Attack Damage.
pub(crate) fn buff_sum(
    ctx: &StableSim<'_>,
    player: usize,
    name: &str,
    value: impl Fn(&BuffV1) -> i32,
) -> u64 {
    ctx.get_player(player)
        .and_then(|player| player.champion())
        .map_or(0, |champion| {
            (0..champion.buff_count())
                .filter_map(|index| champion.buff_at(index))
                .filter(|buff| buff.name() == name)
                .map(|buff| value(&buff))
                .sum::<i32>()
                .max(0) as u64
        })
}

// How many copies of the buff `name` the champion of `player` wears: the stacks
// of an item that keeps them on the champion (`crate::add_stack`).
pub(crate) fn buff_copies(ctx: &StableSim<'_>, player: usize, name: &str) -> u64 {
    ctx.get_player(player)
        .and_then(|player| player.champion())
        .map_or(0, |champion| {
            (0..champion.buff_count())
                .filter(|&index| champion.buff_at(index).is_some_and(|buff| buff.name() == name))
                .count() as u64
        })
}

fn note_as(
    ctx: &StableSim<'_>,
    player: usize,
    key: &'static str,
    in_combat: bool,
    count: impl FnOnce() -> u64,
) {
    let tick = ctx.tick();
    if tick % NOTE_EVERY != 0 {
        return;
    }
    note_origin(ctx);
    if !item_spend::on_screen(ctx) {
        return;
    }
    let seat = ctx.get_player(player).and_then(|player| {
        let lane = player.lane().map(|lane| lane.code() as usize)?;
        Some((player.team(), lane))
    });
    match_builds::log(&format!("counts.note.{key}.{player}"), || match seat {
        Some((side, lane)) => format!("side {side}, lane {lane}"),
        None => "no side and lane".to_string(),
    });
    let Some((side, lane)) = seat.filter(|&(side, lane)| side < 2 && lane < LANES) else {
        return;
    };
    let count = count();
    record(ctx.seed(), tick, |counts| {
        let items = &mut counts.seats[side][lane].counts;
        let at = items
            .iter()
            .position(|known| known.key == key)
            .unwrap_or_else(|| {
                items.push(Count {
                    key,
                    in_combat,
                    changes: Vec::new(),
                });
                items.len() - 1
            });
        items[at].change(tick, count);
    });
}

// The kinds of simulation counting items have noted from, for the test log:
// a bit a `SimOriginKindV1` code, and the top bit for one with no origin.
fn note_origin(ctx: &StableSim<'_>) {
    static KINDS: AtomicU32 = AtomicU32::new(0);
    let kind = ctx
        .sim_origin()
        .map_or(1 << 31, |origin| 1u32 << origin.kind.min(30));
    let before = KINDS.fetch_or(kind, Ordering::Relaxed);
    if before & kind == 0 {
        let kinds = before | kind;
        match_builds::log("counts.origins", || {
            let codes: Vec<String> = (0..32u32)
                .filter(|&bit| kinds & (1u32 << bit) != 0)
                .map(|bit| if bit == 31 { "none".to_string() } else { bit.to_string() })
                .collect();
            codes.join(", ")
        });
    }
}

// -- the moment on screen -----------------------------------------------------

// The match clock over the map, "12:34".
const CLOCK: &str = "ingame.header.game_time.value";

// The tick the screen is at, by the match clock: the first of the second it
// reads, so a count that changes within that second shows when the clock
// moves on, and never early.
fn shown_tick(ctx: &StableClient<'_>) -> Option<usize> {
    let text = ctx.ui_text(CLOCK)?;
    let mut seconds = 0usize;
    for part in text.trim().split(':') {
        let part = part.trim().parse::<usize>().ok()?;
        seconds = seconds.checked_mul(60)?.checked_add(part)?;
    }
    Some((seconds as f64 * TICKS_PER_SECOND) as usize)
}

// The counts as the screen shows the match, where the clock says: (side,
// lane, slot, count), every item that holds a count above zero in the slot
// it was in then.
fn at_clock(ctx: &StableClient<'_>) -> Vec<(usize, usize, usize, u64)> {
    let shown = shown_tick(ctx);
    let Ok(counts) = COUNTS.lock() else {
        return Vec::new();
    };
    let Some(counts) = counts.as_ref() else {
        return Vec::new();
    };
    let Some(shown) = shown else {
        match_builds::log("counts.clock", || {
            format!("{CLOCK} cannot be read: {:?}", ctx.ui_text(CLOCK))
        });
        return Vec::new();
    };
    let seconds = (shown as f64 / TICKS_PER_SECOND) as usize;
    if seconds % 30 == 0 {
        match_builds::log("counts.clock", || {
            format!(
                "{seconds} s on the clock = tick {shown}; the simulation is at tick {}",
                counts.tick
            )
        });
    }
    let mut placed = Vec::new();
    for (side, lanes) in counts.seats.iter().enumerate() {
        for (lane, seat) in lanes.iter().enumerate() {
            let Some((_, held)) = seat.held.iter().rev().find(|(at, _)| *at <= shown) else {
                continue;
            };
            for count in &seat.counts {
                let Some(&(_, value)) = count.changes.iter().rev().find(|(at, _)| *at <= shown)
                else {
                    continue;
                };
                if value == 0 {
                    continue;
                }
                if let Some(slot) = held.iter().position(|held| held.as_str() == count.key) {
                    placed.push((side, lane, slot, value));
                }
            }
        }
    }
    placed
}

// -- the labels ---------------------------------------------------------------

// The label's name in its slot, and the label, with `%TEXT%` for its number:
// outlined like the game's skill cooldown number, and taking no events so the
// slot still answers the cursor.
const LABEL: &str = "count";
const LABEL_SOURCE: &str = r##"count:label {
  palette: false;
  @"asset/base/style/main#bold_label";
  anchor_x: 1;
  pivot_x: 1;
  anchor_y: 1;
  pivot_y: 1;

  width: 80px;
  height: 18px;
  size: 13;

  color: #ffffffff;
  outline: 1;
  outline_color: #000000ff;

  align_x: Right;
  align_y: Bottom;

  text: "%TEXT%";
  ignore_event: true;
}
"##;

// Client frames between two looks at the labels: ten a second at sixty
// frames.
const SYNC_EVERY: u32 = 6;

// The labels now showing a number, by path.
static SHOWN: Mutex<Vec<String>> = Mutex::new(Vec::new());

// What there is on the way to `path`, for the test log: the deepest node of
// it there is, and what that node holds.
fn tree(ctx: &StableClient<'_>, path: &str) -> String {
    let mut at = path;
    loop {
        if ctx.ui_exists(at) {
            return format!("{at} holds {:?}", ctx.ui_child_names(at));
        }
        match at.rsplit_once('.') {
            Some((up, _)) => at = up,
            None => return "none of it is there".to_string(),
        }
    }
}

// The slots to label, by path, and the number each label is to say.
fn wanted(ctx: &StableClient<'_>) -> Vec<(String, String)> {
    let placed = at_clock(ctx);
    if placed.is_empty() {
        return Vec::new();
    }
    let mut wanted = Vec::new();
    for (view, host) in ROW_HOSTS {
        if ctx.ui_visible(view) != Some(true) {
            continue;
        }
        let rows = ctx.ui_child_names(host);
        let first = wanted.len();
        for &(side, lane, slot, count) in &placed {
            let row = (0..rows.len())
                .find(|place| item_spend::row_lane(&rows, *place) == Some(lane))
                .and_then(|place| rows.get(place));
            let Some(row) = row else {
                continue;
            };
            let items = format!("{host}.{row}.{}.items", HALVES[side]);
            let Some(node) = item_spend::slot_nodes(ctx, &items).into_iter().nth(slot) else {
                match_builds::log(&format!("counts.tree.{items}"), || {
                    format!("no slot {slot}: {}", tree(ctx, &items))
                });
                continue;
            };
            wanted.push((node, count.to_string()));
        }
        match_builds::log(&format!("counts.{host}"), || {
            let slots: Vec<&String> = wanted[first..].iter().map(|(path, _)| path).collect();
            format!("rows {rows:?}; slots {slots:?}")
        });
    }
    wanted
}

// Draws every count of the match on screen on its item's icon, and takes off
// the ones whose item is gone. Every [`SYNC_EVERY`]th frame in a match; labels
// are re-read each time, since game code respawns slots without them.
pub(crate) fn sync(ctx: &mut StableClient<'_>) {
    static FRAME: AtomicU32 = AtomicU32::new(0);
    static SPAWNED: AtomicU32 = AtomicU32::new(0);
    if FRAME.fetch_add(1, Ordering::Relaxed) % SYNC_EVERY != 0 {
        return;
    }
    let wanted = wanted(ctx);
    let Ok(mut shown) = SHOWN.lock() else {
        return;
    };
    shown.retain(|path| {
        let slot = path.strip_suffix(LABEL).and_then(|up| up.strip_suffix('.'));
        if wanted.iter().any(|(known, _)| Some(known.as_str()) == slot) {
            return true;
        }
        ctx.ui_remove_node(path);
        false
    });
    for (slot, text) in wanted {
        let label = format!("{slot}.{LABEL}");
        if ctx.ui_exists(&label) {
            if ctx.ui_text(&label).as_deref() == Some(text.as_str())
                && ctx.ui_visible(&label) == Some(true)
            {
                continue;
            }
            let written = ctx.ui_set_text(&label, &text);
            let showing = ctx.ui_set_visible(&label, true);
            match_builds::log(&format!("counts.label.{label}"), || {
                format!("written: {written}, shown: {showing}")
            });
        } else {
            let source = LABEL_SOURCE.replace("%TEXT%", &text);
            let spawned = ctx.ui_spawn_source(&slot, &source);
            let times = SPAWNED.fetch_add(1, Ordering::Relaxed) + 1;
            // Every spawn would fill the log if game code spawned its slots
            // again and again; each power of two says whether it does.
            if !spawned || times.is_power_of_two() {
                match_builds::log(&format!("counts.label.{label}"), || {
                    format!("spawned: {spawned} (labels spawned so far: {times})")
                });
            }
            if !spawned {
                continue;
            }
        }
        if !shown.contains(&label) {
            shown.push(label);
        }
    }
}

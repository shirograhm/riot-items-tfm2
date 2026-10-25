//! Heal and Shield Power: every heal and shield a carrier casts, on allies and
//! on itself, raised by a percentage its items add up, as Lethality adds up
//! across items. As in League, regeneration and lifesteal are not raised: the
//! engine's HP regen and omnivamp, and the mod's heals of that kind
//! ([`unraised`]).
//!
//! The stable API cannot change a heal or a shield as it lands, so the raise
//! is made inside the engine, by detours on its one heal function and its one
//! add-shield function (`crate::tactics::heal_shield`). They ask [`power`]
//! what the healer or the shield's caster has. The mod's own shields bypass
//! the engine's add-shield, so an item that shields raises its shields itself
//! ([`raised`], [`of`]).
//!
//! Each holding item reports its share from its `update` ([`note`]), every
//! tick: a carrier's total is the sum of one tick's reports (two items of 12%
//! make 24%), so an item upgraded into one without the stat stops counting a
//! tick later. Totals are kept per thread and per match (the engine's game
//! object): matches run on several threads, and their entity ids overlap.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::mem::size_of;

use mod_api_stable::{SimCtxV1, StableSim};

// Ticks a report holds after the tick it was made on: an item's `update` and a
// heal come in either order within a tick.
const FRESH_TICKS: usize = 1;

// Matches a thread keeps totals for at most; a thread that has run more has
// finished with the oldest.
const MAX_GAMES: usize = 8;

#[derive(Default)]
struct Game {
    // The tick the match was last seen on.
    tick: usize,
    carriers: HashMap<usize, Total>,
}

// A champion's total: what its items have reported so far on `tick`, and all
// they reported on the tick before. A heal can land between two items'
// reports, so it reads the larger of the two rather than half a sum.
#[derive(Default)]
struct Total {
    tick: usize,
    running: f64,
    last: f64,
}

#[derive(Default)]
struct Carriers {
    games: HashMap<usize, Game>,
    // The match this thread last ran a tick of: the add-shield detour is handed
    // no game object, only the unit.
    current: usize,
}

thread_local! {
    static CARRIERS: RefCell<Carriers> = RefCell::new(Carriers::default());
}

// The engine's game object behind `sim`, which is the first word of the host
// state every sim slot is handed (the host's `heal` slot passes it on as the
// heal function's first argument). `StableSim` keeps the context private; it
// is one pointer wide, which the assertion holds it to.
pub(crate) fn game_of(sim: &StableSim<'_>) -> usize {
    const _: () = assert!(size_of::<StableSim<'static>>() == size_of::<usize>());
    unsafe {
        let raw = *(sim as *const StableSim<'_> as *const *const SimCtxV1);
        if raw.is_null() {
            return 0;
        }
        let state = (*raw).state as *const usize;
        if state.is_null() {
            0
        } else {
            *state
        }
    }
}

// Reports `percent` Heal and Shield Power from one of `player`'s items, from the
// item's `update`.
pub(crate) fn note(ctx: &StableSim<'_>, player: usize, percent: f64) {
    if percent <= 0.0 {
        return;
    }
    let Some(champion) = ctx
        .get_player(player)
        .and_then(|player| player.champion())
        .map(|champion| champion.id())
    else {
        return;
    };
    let game = game_of(ctx);
    if game == 0 {
        return;
    }
    let tick = ctx.tick();
    let _ = CARRIERS.try_with(|cell| {
        let Ok(mut all) = cell.try_borrow_mut() else {
            return;
        };
        all.current = game;
        let state = all.games.entry(game).or_default();
        state.tick = tick;
        let total = state.carriers.entry(champion).or_default();
        if total.tick != tick {
            // The tick before's sum is whole only where it was the tick before.
            total.last = if total.tick + 1 == tick {
                total.running
            } else {
                0.0
            };
            total.running = 0.0;
            total.tick = tick;
        }
        total.running += percent;
    });
}

// A match's first tick: what this thread kept for an earlier match in the same
// game object goes.
pub(crate) fn on_match_start(sim: &StableSim<'_>) {
    let game = game_of(sim);
    let _ = CARRIERS.try_with(|cell| {
        let Ok(mut all) = cell.try_borrow_mut() else {
            return;
        };
        all.games.remove(&game);
        if all.games.len() >= MAX_GAMES {
            all.games.clear();
        }
        all.current = game;
    });
}

// Every tick of every match, from the match hook: which match this thread is
// running, and how far it has got.
pub(crate) fn on_match_tick(sim: &StableSim<'_>) {
    let game = game_of(sim);
    if game == 0 {
        return;
    }
    let tick = sim.tick();
    let _ = CARRIERS.try_with(|cell| {
        let Ok(mut all) = cell.try_borrow_mut() else {
            return;
        };
        all.current = game;
        if let Some(state) = all.games.get_mut(&game) {
            state.tick = tick;
        }
    });
}

// The Heal and Shield Power `champion` has in `game`: 0 for one with none, or
// none reported lately. Never panics, since the detours call it.
pub(crate) fn power(game: usize, champion: usize) -> f64 {
    CARRIERS
        .try_with(|cell| {
            let all = cell.try_borrow().ok()?;
            let state = all.games.get(&game)?;
            let total = state.carriers.get(&champion)?;
            (total.tick <= state.tick && state.tick - total.tick <= FRESH_TICKS)
                .then_some(total.running.max(total.last))
        })
        .ok()
        .flatten()
        .unwrap_or(0.0)
}

// The match this thread is running, where it has a carrier in it.
pub(crate) fn current_game() -> Option<usize> {
    CARRIERS
        .try_with(|cell| {
            let all = cell.try_borrow().ok()?;
            let state = all.games.get(&all.current)?;
            (!state.carriers.is_empty()).then_some(all.current)
        })
        .ok()
        .flatten()
}

// What `champion` has in the match `ctx` is running.
pub(crate) fn of(ctx: &StableSim<'_>, champion: usize) -> f64 {
    power(game_of(ctx), champion)
}

thread_local! {
    // A heal of the mod's is under way that is not raised ([`unraised`]).
    static UNRAISED: Cell<bool> = const { Cell::new(false) };
}

// Runs `heal`, a heal of the mod's that Heal and Shield Power leaves alone, as
// it does the engine's regen and lifesteal: regeneration (Warmog's Heart), a
// share of damage dealt or a restore on hit (the jungle items' sustain), or a
// heal received made bigger (Spirit Visage). The heal detour reads the mark
// ([`is_unraised`]); a heal's call runs on the thread that makes it.
pub(crate) fn unraised<R>(heal: impl FnOnce() -> R) -> R {
    let was = UNRAISED.try_with(|flag| flag.replace(true)).unwrap_or(false);
    let out = heal();
    let _ = UNRAISED.try_with(|flag| flag.set(was));
    out
}

// Whether the heal under way is one of [`unraised`]'s.
pub(crate) fn is_unraised() -> bool {
    UNRAISED.try_with(Cell::get).unwrap_or(false)
}

// `amount` raised by `percent`.
pub(crate) fn raised(amount: usize, percent: f64) -> usize {
    if percent <= 0.0 {
        return amount;
    }
    (amount as f64 * (100.0 + percent) / 100.0).round() as usize
}

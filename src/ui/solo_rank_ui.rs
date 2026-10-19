//! Solo-rank match history's item row.
//!
//! The 5th and 6th slots put six-item records into solo rank, and the game
//! looks up `item.item_slot{i}` for each owned item, so a row short of slots
//! takes the whole tab down. `mod.override_info` remaps
//! `solo_rank_component/view_slot` to a copy of 0.6.0's with
//! `item_slot4`/`item_slot5` added, compacted to fit the row; rebase it onto
//! vanilla after any game update that touches the row.
//!
//! [`sync`] restores a three-slot look and only runs while
//! `build_config::picker_slots` is 3, which it never is now. It is kept in case
//! the count becomes configurable again; the `VANILLA_*` geometry is 0.5.x's.

use std::sync::Mutex;

use mod_api_stable::*;

use crate::build_config::picker_slots;

// The slot the three-slot look puts away. A node with this child is an item
// block, since the row has no better name to match on.
const SLOT3: &str = "item_slot3";

// Vanilla geometry, restored onto slots 0/1/2 when the config says three.
// These are the values in the base copy of `view_slot.ui`, not invented ones.
const VANILLA_SLOT: u32 = 40;
const VANILLA_ICON: u32 = 32;
const VANILLA_ROUNDING: u32 = 8;

// Gap between slots while three are shown (vanilla's is 4px): 2px keeps the row
// inside the 128px block whether or not the engine's `LeftToRight` skips the
// hidden fourth slot.
const THREE_SPACING: u32 = 2;

// How often the tree is walked, in frames; between walks the cached blocks are
// re-asserted. A newly expanded row shows the wide look for up to this many
// frames.
const WALK_EVERY: u32 = 15;

// How deep below the solo-rank screen an item block may be: room to spare
// without letting a wrong root loose on the whole tree.
const MAX_DEPTH: u32 = 8;

#[derive(Default)]
struct State {
    // Full path of the solo-rank screen, once found. Cleared whenever the
    // screen goes away so a re-entered screen is searched for again rather than
    // written to at a stale path.
    screen: Option<String>,
    // Item block paths found by the last walk.
    blocks: Vec<String>,
    tick: u32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> Option<T> {
    let mut guard = STATE.lock().ok()?;
    Some(f(guard.get_or_insert_with(State::default)))
}

// Finds the solo-rank screen among `main.contents`' children, by name: game
// code builds it with no layout of its own to take a path from. Names with and
// without the underscore are accepted, since the engine uses both.
fn find_screen(ctx: &StableClient<'_>) -> Option<String> {
    ctx.ui_child_names("main.contents")
        .into_iter()
        .find(|name| {
            let name = name.to_ascii_lowercase();
            name == "solo_rank" || name == "solorank"
        })
        .map(|name| format!("main.contents.{name}"))
}

// Collects every item block under `root`, breadth-first: any node with an
// `item_slot3` child, however the athlete list nests.
fn find_blocks(ctx: &StableClient<'_>, root: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut level = vec![root.to_string()];
    for _ in 0..MAX_DEPTH {
        if level.is_empty() {
            break;
        }
        let mut next = Vec::new();
        for parent in level {
            for name in ctx.ui_child_names(&parent) {
                let path = format!("{parent}.{name}");
                if ctx.ui_exists(&format!("{path}.{SLOT3}")) {
                    // The block itself. Its children are the slots, so there is
                    // nothing below it worth descending into.
                    found.push(path);
                } else {
                    next.push(path);
                }
            }
        }
        level = next;
    }
    found
}

// Writes the three-slot look onto one item block.
fn apply_three(ctx: &mut StableClient<'_>, block: &str) {
    ctx.ui_set_properties(
        block,
        &format!("child_type: LeftToRight {{ spacing: {THREE_SPACING}px; }}"),
    );
    for slot in 0..3 {
        let path = format!("{block}.item_slot{slot}");
        ctx.ui_set_properties(
            &path,
            &format!(
                "width: {VANILLA_SLOT}px; height: {VANILLA_SLOT}px; \
                 rounding: Uniform {{ rounding: {VANILLA_ROUNDING}; }}"
            ),
        );
        ctx.ui_set_properties(
            &format!("{path}.icon"),
            &format!(
                "width: {VANILLA_ICON}px; height: {VANILLA_ICON}px; \
                 rounding: Uniform {{ rounding: {VANILLA_ROUNDING}; }}"
            ),
        );
    }
    // Zero-width as well as hidden: hiding alone still reserves the slot's width
    // if the engine lays out invisible children, and the block has no room to
    // give. See `THREE_SPACING`.
    let slot3 = format!("{block}.{SLOT3}");
    ctx.ui_set_properties(&slot3, "width: 0px; height: 0px;");
    ctx.ui_set_visible(&slot3, false);
}

// Per-frame entry point, called before the client hook's strategy-screen early
// return.
pub fn sync(ctx: &mut StableClient<'_>) {
    if picker_slots() != 3 {
        // The shipped layout already holds every slot; only a three-slot count
        // needs this.
        return;
    }

    let Some(screen) = with_state(|state| state.screen.clone()).flatten() else {
        // No screen cached: look for one, but only on the walk cadence so that
        // every other screen in the game does not pay for a child enumeration
        // every frame.
        let due = with_state(|state| {
            let due = state.tick % WALK_EVERY == 0;
            state.tick = state.tick.wrapping_add(1);
            due
        })
        .unwrap_or(false);
        if due {
            if let Some(found) = find_screen(ctx) {
                let _ = with_state(|state| state.screen = Some(found));
            }
        }
        return;
    };

    if !ctx.ui_exists(&screen) {
        // Left the screen. Drop everything: the rows are gone, and the next
        // visit builds new ones at paths this cache cannot predict.
        let _ = with_state(|state| {
            state.screen = None;
            state.blocks.clear();
        });
        return;
    }

    let (blocks, due) = with_state(|state| {
        let due = state.tick % WALK_EVERY == 0;
        state.tick = state.tick.wrapping_add(1);
        (state.blocks.clone(), due)
    })
    .unwrap_or((Vec::new(), false));

    if due {
        let found = find_blocks(ctx, &screen);
        for block in &found {
            apply_three(ctx, block);
        }
        let _ = with_state(|state| state.blocks = found);
        return;
    }

    // Between walks, re-assert what is known: game code drives these nodes, so
    // a one-shot write may not survive.
    for block in &blocks {
        if ctx.ui_exists(block) {
            apply_three(ctx, block);
        }
    }
}

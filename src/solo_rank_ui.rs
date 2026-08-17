//! Solo-rank match history: makes the item row match the configured slot count.
//!
//! # Why the layout cannot just be the right one
//!
//! `ui/layout/solo_rank_component/view_slot.ui` is an asset override, and
//! `mod.override_info` is static JSON the loader reads before this code runs.
//! There is no way to pick a different layout per `4items.cfg` value, so the
//! shipped layout has to be one of the two — and which one is not a free choice.
//!
//! `solo_rank_ui.rs` does not look up `item_slot0`/`1`/`2` as literals. The exe
//! holds only the prefix `item.item_slot` and the suffix `.icon` (`0x3447ed1`
//! and `0x3447ee1` in the 0.5.5 build) and formats them with the item's index,
//! so it asks for exactly as many slot nodes as the athlete has items. A
//! four-item record against a three-slot layout fails the lookup and takes the
//! whole tab with it:
//!
//! ```text
//! [main_ui] failed to create main tab: Solorank
//! ```
//!
//! Four items are reachable here because `slots = 4` byte-patches the game's own
//! buy resolver and `run_tick_ext` process-wide, and `FIXB` deliberately injects
//! into background sims scoped by `is_my_athlete` — which is precisely who plays
//! solo-rank matches.
//!
//! So the shipped layout declares **four** slots always. That direction cannot
//! break: a fourth node nobody asks for is an empty box, whereas a missing
//! fourth node nobody shipped is a dead tab. This module pays for that choice by
//! restoring the vanilla three-slot look at runtime when the config says three.
//!
//! The consequence worth stating plainly: **if this module never runs, the tab
//! still works.** It is cosmetics on top of a layout that is already safe in
//! both modes, which is the whole reason the split is this way round and not the
//! other.
//!
//! # Inert in 4-slot mode
//!
//! [`sync`] returns on its first line when the count is four, because the
//! shipped layout already *is* the four-slot geometry. Nothing walks, nothing is
//! written, and the per-frame cost is one atomic load. Only 3-slot users pay for
//! any of this.

use std::sync::Mutex;

use mod_api_stable::*;

use crate::build_config::picker_slots;

/// The node every item block carries in the shipped layout, and the one the
/// vanilla three-slot look has to put away. Its presence is also what marks a
/// node as an item block during the walk — the row has no other name worth
/// matching on, and `item` alone is far too common a name to trust.
const SLOT3: &str = "item_slot3";

/// Vanilla geometry, restored onto slots 0/1/2 when the config says three.
/// These are the values in the base copy of `view_slot.ui`, not invented ones.
const VANILLA_SLOT: u32 = 40;
const VANILLA_ICON: u32 = 32;
const VANILLA_ROUNDING: u32 = 8;

/// Gap between slots while three are shown, and the one number here that is not
/// vanilla's (which is 4px).
///
/// It has to fit whether or not the engine's `LeftToRight` skips an invisible
/// child, because that is not something this mod gets to find out from a layout
/// file. The block is 128px wide and slot 3 is set to zero width, so:
///
/// * child skipped:     3*40 + 2*2 = 124  <= 128
/// * child not skipped: 3*40 + 3*2 +0 = 126  <= 128
///
/// At vanilla's 4px the second case is 132px and overflows the block, which
/// would push `replay_wrap` off the end of a row that has no spare pixels
/// anywhere. 2px fits both, and the difference is not visible.
const THREE_SPACING: u32 = 2;

/// How often the tree is walked, in frames. Between walks the cached blocks are
/// re-asserted every frame, which is a handful of writes.
///
/// The walk is the expensive half and the rows it looks for only appear when the
/// player expands an athlete, so a walk per frame would spend its whole budget
/// finding nothing. The cost of the throttle is that a freshly expanded row
/// shows the four-slot geometry for up to this many frames (about a quarter of a
/// second at 60fps) before it snaps to three.
const WALK_EVERY: u32 = 15;

/// How deep below the solo-rank screen root an item block is allowed to be.
///
/// The chain is roughly screen -> list -> scroll contents -> athlete -> view ->
/// row -> item, and this leaves room to spare without letting a wrong root turn
/// the walk loose over the whole tree.
const MAX_DEPTH: u32 = 8;

#[derive(Default)]
struct State {
    /// Full path of the solo-rank screen, once found. Cleared whenever the
    /// screen goes away so a re-entered screen is searched for again rather than
    /// written to at a stale path.
    screen: Option<String>,
    /// Item block paths found by the last walk.
    blocks: Vec<String>,
    tick: u32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> Option<T> {
    let mut guard = STATE.lock().ok()?;
    Some(f(guard.get_or_insert_with(State::default)))
}

/// Finds the solo-rank screen among the children of `main.contents`.
///
/// Matched by name rather than written down as a constant: every screen the mod
/// already addresses is one it also ships a layout override for, so its path is
/// known from the override. This screen is not — the tab is built by game code
/// from `solo_rank_component/*` with no top-level layout of its own, and a
/// guessed constant that is wrong fails silently and leaves the rows untouched
/// with nothing to show for it.
///
/// The tab is `Solorank` in the log, the runner is `solo_rank_ui` and the assets
/// are `solo_rank_component`, so the engine is inconsistent about the
/// underscore. Both spellings are accepted for that reason.
fn find_screen(ctx: &StableClient<'_>) -> Option<String> {
    ctx.ui_child_names("main.contents")
        .into_iter()
        .find(|name| {
            let name = name.to_ascii_lowercase();
            name == "solo_rank" || name == "solorank"
        })
        .map(|name| format!("main.contents.{name}"))
}

/// Collects every item block under `root`, breadth-first.
///
/// A block is any node with an `item_slot3` child. That test is what keeps this
/// honest about a tree whose shape is game code's business: it does not care how
/// the athlete list nests, only that it recognises the thing it came to change.
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

/// Writes the three-slot look onto one item block.
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

/// Per-frame entry point, called from the mod's one client hook.
///
/// Ordered before that hook's own strategy-screen early return, since this
/// screen is not that one.
pub fn sync(ctx: &mut StableClient<'_>) {
    if picker_slots() != 3 {
        // The shipped layout is already correct for four slots. This is the
        // author's own configuration, so the common path costs one atomic load.
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

    // Between walks, re-assert what is already known. Game code owns these nodes
    // and drives them from its own idea of the row, so a one-shot write is not
    // guaranteed to survive — the same reason the strategy screen's properties
    // are re-asserted rather than set once on entry.
    for block in &blocks {
        if ctx.ui_exists(block) {
            apply_three(ctx, block);
        }
    }
}

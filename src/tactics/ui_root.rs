//! Locating the live UI root node (`GameUI.root`) without the classic API.
//!
//! The classic `ModExtension::post_update` was handed `&mut GameUI`, and every
//! UI handler in `super` starts from its `root` field. A stable-ABI mod is never
//! handed that object, and the merge's first attempt — reusing `TIP_ROOT`, the
//! tooltip function's node search root — crashed the game on the first UI frame,
//! because that pointer is not a `Node`.
//!
//! So the address is *found* rather than assumed, and only a candidate that
//! survives a structural walk is ever handed to `find_node`. This is the same
//! self-validating approach `probe_db` and `dump_mod_items` already use for the
//! `Database` and the mod item array: guess an address, then prove it.
//!
//! # Why the validation has to be raw
//!
//! The VEH only rewrites faults whose RIP lands inside the inline-asm block in
//! `safe_copy`; a fault anywhere else is not caught. Handing a bad pointer to
//! `find_node` therefore kills the process rather than returning `None` — which
//! is exactly how the first attempt failed. Every read below goes through
//! `safe_read_*`, and `&Node` is constructed only after a candidate has proven
//! it has a readable id and a coherent child vector several levels deep.
//!
//! # Anchors
//!
//! 1. `TIP_ROOT` — tested first because it costs nothing. It is probably not the
//!    root, but the validator settles that instead of a comment guessing.
//! 2. A window scan from the `App` object (`GAME_VIEW - 0x4a50`, per
//!    `cap_game_view`), testing each slot both as a pointer to a `Node` and as
//!    an inline `Node`. `GameUI` may be embedded in `App` rather than boxed, so
//!    both shapes have to be considered.
//!
//! # Field offsets
//!
//! Taken with `offset_of!` from `mod_api`'s own `Node`, so they follow the type
//! rather than a hardcoded number that rots on the next SDK bump. Only the
//! engine's string layout is hardcoded — `{len@0, ptr@8, cap@16}`, documented in
//! `super::set_img_src` and reused here.

use std::mem::{offset_of, size_of};
use std::sync::atomic::{AtomicUsize, Ordering};

use mod_api::Node;

use super::{safe_read_bytes, safe_read_u64};

/// `GameView = App + 0x4a50` — see `cap_game_view`, which captures the former.
const GAME_VIEW_IN_APP: usize = 0x4a50;

/// How far past `App` to look for the UI root, in bytes.
const SCAN_WINDOW: usize = 0x10000;

/// An id every screen has under the root: UI paths in this game are rooted at
/// `main.` (`main.contents.strategy...`), so a node tree containing `main` is
/// the tree the handlers expect. Chosen over `player_info` because that one only
/// exists during a match, and the root has to resolve before then.
const ROOT_MARKER_ID: &str = "main";

/// Depth the marker search descends. The marker sits within a couple of levels
/// of the root; going deeper mostly buys time spent walking garbage.
const MARKER_DEPTH: usize = 3;

/// Nodes visited per candidate before giving up. Garbage that happens to look
/// like a `Vec` can otherwise fan out indefinitely.
const VISIT_BUDGET: usize = 512;

/// Resolved root, or 0. Not a `OnceLock`: the answer is only cached once a
/// candidate has been *validated*, so a scan that runs before the UI exists
/// leaves the question open for the next frame rather than poisoning it.
static UI_ROOT: AtomicUsize = AtomicUsize::new(0);
/// Bounded retries, so a game that never produces a resolvable root does not
/// pay for a full window scan on every frame forever.
static ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
const MAX_ATTEMPTS: usize = 600;
/// How the resolved root was found, for the diagnostic report.
static SOURCE: AtomicUsize = AtomicUsize::new(0);

/// Reads the engine string at `addr` — `{len@0, ptr@8, cap@16}`, the layout
/// `set_img_src` documents. Rejects anything that is not a short ASCII
/// identifier, which is what every node id in this game is.
unsafe fn read_engine_string(addr: usize) -> Option<String> {
    let len = safe_read_u64(addr)? as usize;
    let ptr = safe_read_u64(addr + 8)? as usize;
    if len == 0 || len > 64 || ptr <= 0x10000 {
        return None;
    }
    let mut bytes = Vec::new();
    if !safe_read_bytes(ptr, len, &mut bytes) {
        return None;
    }
    // Node ids are ASCII identifiers. Requiring that rejects almost all
    // coincidental (len, ptr) pairs that happen to point at readable memory.
    if !bytes
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
    {
        return None;
    }
    String::from_utf8(bytes).ok()
}

/// The id of the `Node` at `addr`, if it reads as one.
unsafe fn node_id(addr: usize) -> Option<String> {
    read_engine_string(addr + offset_of!(Node, id))
}

/// The `(ptr, len)` of the `Node` at `addr`'s child vector.
///
/// `Vec<Node>` is `RawVec { ptr, cap }` plus `len`, but that field order is not
/// a guarantee, so both plausible arrangements are tried — the same tactic
/// `dump_mod_items` uses on the mod item array. An empty child vector is a valid
/// answer (leaf node), reported as `(0, 0)`.
unsafe fn node_children(addr: usize) -> Option<(usize, usize)> {
    let base = addr + offset_of!(Node, child);
    let w0 = safe_read_u64(base)? as usize;
    let w1 = safe_read_u64(base + 8)? as usize;
    let w2 = safe_read_u64(base + 16)? as usize;

    // (ptr, cap, len) and (cap, ptr, len). `len` is last in both, which is what
    // rustc has always emitted for `Vec`.
    for (ptr, cap) in [(w0, w1), (w1, w0)] {
        let len = w2;
        if len == 0 {
            // A leaf: the pointer is dangling-but-aligned, so it is not checked.
            return Some((0, 0));
        }
        if len <= cap && len <= 4096 && ptr > 0x10000 {
            return Some((ptr, len));
        }
    }
    None
}

/// Whether the subtree at `addr` contains a node whose id is `target`.
///
/// Walks with `safe_read_*` only. `budget` bounds total visits so a candidate
/// that merely *resembles* a node tree cannot spin.
unsafe fn subtree_has_id(addr: usize, target: &str, depth: usize, budget: &mut usize) -> bool {
    if *budget == 0 || depth == 0 {
        return false;
    }
    *budget -= 1;

    if node_id(addr).as_deref() == Some(target) {
        return true;
    }
    let Some((ptr, len)) = node_children(addr) else {
        return false;
    };
    let stride = size_of::<Node>();
    for i in 0..len.min(*budget) {
        if subtree_has_id(ptr + i * stride, target, depth - 1, budget) {
            return true;
        }
    }
    false
}

/// Whether `addr` is a `Node` that roots the tree the handlers expect.
///
/// The id check runs first because it is one read and rejects nearly everything;
/// only survivors pay for the subtree walk.
unsafe fn is_ui_root(addr: usize) -> bool {
    if addr <= 0x10000 || addr % 8 != 0 {
        return false;
    }
    if node_id(addr).is_none() {
        return false;
    }
    let mut budget = VISIT_BUDGET;
    subtree_has_id(addr, ROOT_MARKER_ID, MARKER_DEPTH, &mut budget)
}

/// The live UI root, scanning for it on first call and caching a validated hit.
///
/// Returns `None` until the UI exists — callers must treat that as "not this
/// frame", exactly as they already treat a missing `GAME_VIEW`.
pub fn resolve() -> Option<usize> {
    let cached = UI_ROOT.load(Ordering::Relaxed);
    if cached != 0 {
        return Some(cached);
    }
    if ATTEMPTS.fetch_add(1, Ordering::Relaxed) >= MAX_ATTEMPTS {
        return None;
    }

    // 1. TIP_ROOT. Free to test, and the validator — not an assumption — decides.
    let tip = super::TIP_ROOT.load(Ordering::Relaxed);
    if tip > 0x10000 && unsafe { is_ui_root(tip) } {
        UI_ROOT.store(tip, Ordering::Relaxed);
        SOURCE.store(1, Ordering::Relaxed);
        return Some(tip);
    }

    // 2. Window scan from App.
    let game_view = super::GAME_VIEW.load(Ordering::Relaxed);
    if game_view <= GAME_VIEW_IN_APP {
        return None;
    }
    let app = game_view - GAME_VIEW_IN_APP;

    let mut offset = 0usize;
    while offset < SCAN_WINDOW {
        let slot = app + offset;
        // The UI may be boxed (a pointer in the slot) or embedded (the slot is
        // the node). Both are cheap to test, so neither is assumed.
        if let Some(pointee) = unsafe { safe_read_u64(slot) } {
            let pointee = pointee as usize;
            if unsafe { is_ui_root(pointee) } {
                UI_ROOT.store(pointee, Ordering::Relaxed);
                SOURCE.store(2, Ordering::Relaxed);
                return Some(pointee);
            }
        }
        if unsafe { is_ui_root(slot) } {
            UI_ROOT.store(slot, Ordering::Relaxed);
            SOURCE.store(3, Ordering::Relaxed);
            return Some(slot);
        }
        offset += 8;
    }
    None
}

/// One line on how the root was resolved, for `build_ext_diag.txt`.
pub fn report() -> String {
    let root = UI_ROOT.load(Ordering::Relaxed);
    if root == 0 {
        return format!(
            "UI root: NOT RESOLVED after {} attempts (in-match 4th slot icon is off)",
            ATTEMPTS.load(Ordering::Relaxed)
        );
    }
    let how = match SOURCE.load(Ordering::Relaxed) {
        1 => "TIP_ROOT",
        2 => "pointer in App window",
        _ => "inline in App window",
    };
    let id = unsafe { node_id(root) }.unwrap_or_else(|| "<unreadable>".into());
    format!("UI root: {root:#x} via {how}, id={id:?}")
}

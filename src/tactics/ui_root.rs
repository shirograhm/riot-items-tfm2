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
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

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
///
/// This counts *scans*, and only scans that had something to scan. Counting
/// calls instead made it a timer rather than a budget: `post_update` ticks from
/// the title screen, where neither anchor exists yet, so the whole allowance was
/// spent on frames that returned `None` without looking at anything — and by the
/// time a match produced a `GAME_VIEW`, every later call returned at the budget
/// check instead of scanning. Measured on 2026-08-04: `GAME_VIEW` live for
/// 12,515 frames, root `NOT RESOLVED after 16501 attempts`, window scan run zero
/// times. Whether the in-match icon worked came down to whether the player
/// reached a match within ~600 frames of launching, which is why it looked like
/// "the first restart after editing `4items.cfg` is broken, the second is fine".
static ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
const MAX_ATTEMPTS: usize = 600;
/// The anchor the counted attempts were spent against. A different anchor is a
/// different object graph, so failures against the old one say nothing about
/// this one and the budget starts over.
static LAST_ANCHOR: AtomicUsize = AtomicUsize::new(0);
/// How the resolved root was found, for the diagnostic report.
static SOURCE: AtomicUsize = AtomicUsize::new(0);

/// FNV-1a of the root node's id at the moment it was accepted, so a cached root
/// can be re-checked in one string read. See [`resolve`] for why it has to be.
static ROOT_ID_HASH: AtomicU64 = AtomicU64::new(0);
/// Times a cached root failed re-validation and was dropped, for the report.
static INVALIDATIONS: AtomicUsize = AtomicUsize::new(0);

fn id_hash(id: &str) -> u64 {
    id.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Caches a root that [`is_ui_root`] has just accepted, with the fingerprint
/// [`resolve`] re-checks it against.
///
/// # Safety
/// `addr` must have passed `is_ui_root`, which is what makes reading its id here
/// sound.
unsafe fn accept(addr: usize, source: usize) -> Option<usize> {
    ROOT_ID_HASH.store(node_id(addr).map(|id| id_hash(&id)).unwrap_or(0), Ordering::Relaxed);
    SOURCE.store(source, Ordering::Relaxed);
    UI_ROOT.store(addr, Ordering::Relaxed);
    Some(addr)
}

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
///
/// # Why the cache is re-checked rather than trusted
///
/// The cached address does not outlive the UI tree it was validated against.
/// Returning to the main menu tears that tree down, and loading a save builds a
/// new one — so a root resolved during the first save is a dangling pointer for
/// the second. Handing it back was a crash, not a wrong answer: `find_node`
/// walks `Node.child` with ordinary Rust reads, and per the module docs above,
/// the VEH only covers faults inside `safe_copy`. Load a save, go back to the
/// menu, load one again, and the first UI frame of the second save died in
/// `handle_tactics_screen`.
///
/// So each call re-reads the root's own id and compares it against the
/// fingerprint taken when it was accepted. That is one string read through the
/// protected path — freed or reused memory answers `None` or a different id
/// instead of faulting — and unlike re-running [`is_ui_root`] it does not depend
/// on the shape of the tree below, which legitimately changes with every screen.
/// A mismatch drops the cache and re-scans, with the attempt budget reset: a new
/// tree is a new object graph, and the old failures said nothing about it.
pub fn resolve() -> Option<usize> {
    let cached = UI_ROOT.load(Ordering::Relaxed);
    if cached != 0 {
        let live = unsafe { node_id(cached) }.map(|id| id_hash(&id));
        if live == Some(ROOT_ID_HASH.load(Ordering::Relaxed)) {
            return Some(cached);
        }
        UI_ROOT.store(0, Ordering::Relaxed);
        ROOT_ID_HASH.store(0, Ordering::Relaxed);
        SOURCE.store(0, Ordering::Relaxed);
        // The anchor may be unchanged across a save reload (`App` outlives the
        // scene), so the budget cannot be left to `LAST_ANCHOR` to reset here —
        // without this the rescan would run into an allowance already spent on
        // the previous save and never resolve again.
        ATTEMPTS.store(0, Ordering::Relaxed);
        INVALIDATIONS.fetch_add(1, Ordering::Relaxed);
    }
    // Both anchors are published by `cap_game_view`, so before the first UI
    // frame there is nothing to look at. Returning here — ahead of the budget —
    // is what keeps menu frames from spending an allowance meant for scans.
    let tip = super::TIP_ROOT.load(Ordering::Relaxed);
    let game_view = super::GAME_VIEW.load(Ordering::Relaxed);
    let has_app = game_view > GAME_VIEW_IN_APP;
    if tip <= 0x10000 && !has_app {
        return None;
    }

    // Spend against the anchor, and reset when it changes: a new `App` means the
    // previous failures were about a different object graph.
    let anchor = if has_app { game_view } else { tip };
    if LAST_ANCHOR.swap(anchor, Ordering::Relaxed) != anchor {
        ATTEMPTS.store(0, Ordering::Relaxed);
    }
    if ATTEMPTS.fetch_add(1, Ordering::Relaxed) >= MAX_ATTEMPTS {
        return None;
    }

    // 1. TIP_ROOT. Free to test, and the validator — not an assumption — decides.
    if tip > 0x10000 && unsafe { is_ui_root(tip) } {
        return unsafe { accept(tip, 1) };
    }

    // 2. Window scan from App.
    if !has_app {
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
                return unsafe { accept(pointee, 2) };
            }
        }
        if unsafe { is_ui_root(slot) } {
            return unsafe { accept(slot, 3) };
        }
        offset += 8;
    }
    None
}

/// One line on how the root was resolved, for `build_ext_diag.txt`.
pub fn report() -> String {
    let root = UI_ROOT.load(Ordering::Relaxed);
    if root == 0 {
        // `scans` counts real scans now, so 0 with a live anchor means the
        // budget was exhausted against an *earlier* anchor, and anything else
        // is a scan that ran and found nothing — two different bugs that the
        // old single "attempts" number could not tell apart.
        let scans = ATTEMPTS.load(Ordering::Relaxed);
        let anchor = LAST_ANCHOR.load(Ordering::Relaxed);
        let state = if anchor == 0 {
            "no anchor yet (TIP_ROOT and GAME_VIEW both unpublished)".to_string()
        } else if scans > MAX_ATTEMPTS {
            format!("budget exhausted against anchor {anchor:#x}")
        } else {
            format!("scanned from anchor {anchor:#x} and found nothing")
        };
        return format!(
            "UI root: NOT RESOLVED, {scans} scans, {state} (in-match 4th slot icon is off)"
        );
    }
    let how = match SOURCE.load(Ordering::Relaxed) {
        1 => "TIP_ROOT",
        2 => "pointer in App window",
        _ => "inline in App window",
    };
    let id = unsafe { node_id(root) }.unwrap_or_else(|| "<unreadable>".into());
    // A non-zero invalidation count is the expected trace of a save reload, not
    // a fault: it says the stale-root re-check fired and the root was found
    // again. It climbing every frame would mean the fingerprint is unstable.
    let dropped = INVALIDATIONS.load(Ordering::Relaxed);
    format!("UI root: {root:#x} via {how}, id={id:?}, stale-root drops={dropped}")
}

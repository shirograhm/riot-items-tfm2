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

/// How far past the nominal `App` base to look for the UI root, in bytes.
const SCAN_WINDOW: usize = 0x10000;

/// How far *below* the nominal `App` base to look as well.
///
/// `GAME_VIEW_IN_APP` is a hint, not a fact — it has moved on game updates and
/// nothing revalidates it. Scanning behind the computed base makes the search
/// tolerant of it being wrong in the direction a forward-only scan cannot
/// survive. Sized to comfortably exceed the drift seen across 0.5.2..0.5.5.
const SCAN_BACK: usize = 0x18000;

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

/// What the last scan actually saw, for `build_ext_diag.txt`.
///
/// "budget exhausted, found nothing" is the same report whether the window is
/// wrong, `Node.id` no longer reads as a string, or the marker moved deeper than
/// [`MARKER_DEPTH`] — three different fixes. This records the evidence that
/// separates them: how many slots read as a node at all, a sample of the ids
/// seen, and whether a deeper/larger search *would* have found the marker.
static SCAN_DIAG: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Depth and budget for the diagnostic-only retry. Deliberately far past what
/// the real check uses, to answer "is the marker simply deeper now?".
const PROBE_DEPTH: usize = 12;
const PROBE_BUDGET: usize = 8192;
/// Cap on diagnostic deep-searches per scan, so the probe cannot dominate a
/// frame when thousands of slots read as plausible nodes.
const PROBE_LIMIT: usize = 96;
/// Failed scans instrumented so far; see the `diag` binding in [`resolve`].
static DIAG_RUNS: AtomicUsize = AtomicUsize::new(0);
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

/// Whether the *cached* root still reads as a node.
///
/// Deliberately not [`is_ui_root`]: that costs up to [`VISIT_BUDGET`] visits and
/// allocates a `String` per node, which is fine once per scan and far too much
/// once per frame. This is five `safe_read_*` and one short allocation — enough
/// to notice that the tree the pointer named has been freed, which is the whole
/// job.
///
/// It is a filter, not a proof. A freed block reused by something that happens
/// to read as an id plus a coherent child vector would still pass, which is why
/// [`invalidate`] is also called at the session boundary rather than relying on
/// this alone.
unsafe fn still_a_node(addr: usize) -> bool {
    node_id(addr).is_some() && node_children(addr).is_some()
}

/// Drops the resolved root, so the next [`resolve`] has to prove a fresh one.
///
/// Called at the session boundary (`tactics_on_server_start`). The UI tree does
/// not survive a return to the main menu, and the address it lived at says
/// nothing about the tree the next save builds.
pub fn invalidate() {
    UI_ROOT.store(0, Ordering::Relaxed);
    ATTEMPTS.store(0, Ordering::Relaxed);
    LAST_ANCHOR.store(0, Ordering::Relaxed);
    SOURCE.store(0, Ordering::Relaxed);
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
        // Re-prove the cached address before handing it out. It was validated
        // when it was found, and nothing has revalidated it since — so after the
        // game freed that tree (returning to the main menu does), this returned
        // a dangling pointer and `tactics_post_update` gave it straight to
        // `find_node`, which walks with raw reads and cannot fault gracefully.
        //
        // That is not hypothetical: it is the load-save / main-menu / load-again
        // crash. The minidump caught `find_node` recursing into a freed node
        // whose child `Vec` read as `ptr = 8` (Rust's dangling pointer for an
        // empty vector) with a non-zero length, then faulting on `[8 + 0x10]`.
        if unsafe { still_a_node(cached) } {
            return Some(cached);
        }
        invalidate();
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
        // Re-arm the scan probe too. It is capped at a few runs to keep its cost
        // off the frame path, and without this reset those runs are spent on
        // whichever anchor appeared first — which is not the anchor the failing
        // scans end up using. `GAME_VIEW` is republished on every `gv_update`
        // call and does change during a session (menu vs match), so the first
        // window scanned and the last can be in unrelated regions entirely: the
        // 0.5.5 report showed a probe window around 0xa3c75e6a10 while the
        // anchor being scanned was 0x280524ccab8. Evidence has to come from the
        // anchor that is actually failing.
        DIAG_RUNS.store(0, Ordering::Relaxed);
    }
    if ATTEMPTS.fetch_add(1, Ordering::Relaxed) >= MAX_ATTEMPTS {
        return None;
    }

    // 1. TIP_ROOT. Free to test, and the validator — not an assumption — decides.
    if tip > 0x10000 && unsafe { is_ui_root(tip) } {
        UI_ROOT.store(tip, Ordering::Relaxed);
        SOURCE.store(1, Ordering::Relaxed);
        return Some(tip);
    }

    // 2. Window scan around the GameView pointer.
    //
    // This used to start at `game_view - GAME_VIEW_IN_APP` and only walk
    // *forward*, which made a stale `GAME_VIEW_IN_APP` fatal in one direction:
    // if the real App offset grows, the computed base lands past the true one
    // and every slot below it — the whole App, in the worst case — is outside
    // the window. Game 0.5.5 did exactly that (`[reg+0x4a50]` went from 17 sites
    // in 0.5.4 to 1), and `build_ext_diag.txt` reported
    // `NOT RESOLVED, 15107 scans, budget exhausted` with a live anchor: the scan
    // was running and looking in the wrong place.
    //
    // The pointer we actually *know* is `game_view`, so the window is centred on
    // it and the constant is demoted to a hint about where the App starts. What
    // decides is still `is_ui_root`, never the arithmetic.
    if !has_app {
        return None;
    }
    let nominal_app = game_view.saturating_sub(GAME_VIEW_IN_APP);
    let start = nominal_app.saturating_sub(SCAN_BACK);
    let span = SCAN_BACK + SCAN_WINDOW;

    // Diagnostic tallies for this scan. All work is gated on BUILD_EXT_DIAG so
    // the production path is unchanged.
    // Only the first few failed scans are instrumented. The probe allocates a
    // `String` per plausible node across a 160KB window, which is fine once but
    // would be a per-frame cost repeated for the whole 600-scan budget.
    let diag = super::BUILD_EXT_DIAG && DIAG_RUNS.load(Ordering::Relaxed) < 3;
    let mut n_nodes = 0usize; // slots that read as a Node (id + children)
    let mut ids: Vec<String> = Vec::new(); // distinct ids seen, sampled
    let mut probes = 0usize; // deep searches spent
    let mut deep_hit: usize = 0; // candidate where a deeper search found the marker

    let mut offset = 0usize;
    while offset < span {
        let slot = start + offset;
        if diag {
            for cand in [
                unsafe { safe_read_u64(slot) }.map(|p| p as usize).unwrap_or(0),
                slot,
            ] {
                if cand <= 0x10000 || cand % 8 != 0 {
                    continue;
                }
                let Some(id) = (unsafe { node_id(cand) }) else {
                    continue;
                };
                if unsafe { node_children(cand) }.is_none() {
                    continue;
                }
                n_nodes += 1;
                if ids.len() < 16 && !ids.contains(&id) {
                    ids.push(id);
                }
                if deep_hit == 0 && probes < PROBE_LIMIT {
                    probes += 1;
                    let mut b = PROBE_BUDGET;
                    if unsafe { subtree_has_id(cand, ROOT_MARKER_ID, PROBE_DEPTH, &mut b) } {
                        deep_hit = cand;
                    }
                }
            }
        }
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
    if diag {
        DIAG_RUNS.fetch_add(1, Ordering::Relaxed);
        // Reached only when the whole window failed, which is exactly when the
        // evidence is wanted. How to read it:
        //   nodes=0            -> nothing in the window reads as a Node: either
        //                         the window is wrong, or `Node.id` / the engine
        //                         string layout no longer match the 0.5.2 rlib.
        //   nodes>0, ids listed, marker_at_depth<=12 = 0
        //                      -> the tree is there but contains no `main` at
        //                         all: ROOT_MARKER_ID is stale.
        //   marker_at_depth<=12 = <addr>
        //                      -> the marker exists but sits deeper than
        //                         MARKER_DEPTH (3) or past VISIT_BUDGET (512);
        //                         raise those rather than touching the window.
        // The anchor is recorded here, not just in `report()`: the two can
        // disagree, and when they do the whole line is about a window nobody is
        // scanning any more. Compare it against the anchor on the line above.
        *SCAN_DIAG.lock().unwrap_or_else(|e| e.into_inner()) = format!(
            "anchor={game_view:#x} window [{start:#x}..{:#x}) nodes={n_nodes} probes={probes} \
             marker_at_depth<={PROBE_DEPTH} = {deep_hit:#x}; ids seen: {}",
            start + span,
            if ids.is_empty() {
                "(none)".to_string()
            } else {
                ids.join(", ")
            }
        );
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
            "UI root: NOT RESOLVED, {scans} scans, {state} (in-match 4th slot icon is off)\n  \
             last scan: {}",
            SCAN_DIAG
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
                .as_str()
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

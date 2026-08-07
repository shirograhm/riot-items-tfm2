//! ui_inject - UI modification via chained loader hooks (tfm2_4items).
//!   (1) Chained install: save the current 12 bytes of the entry point and prepend ourselves -> chain "behind"
//!      another mod's single-owner hook (ai_adjust/scrim). Installed late, from post_update (after those mods' init), to guarantee ordering.
//!      Works standalone when ai_adjust is off too (it saves the original prologue). Idempotent across re-init (INSTALLED).
//!   (2) When the player_info/wide_player_info templates are loaded -> replace the root's children with our edited version (.ui, compressed 4 slots).
//!      (Unlike an asset override this is a loader hook, so it layers on top of other overrides and chains. "Edits" still do not compose between mods,
//!       but nobody else touches player_info, so it is safe.)
//!   RVAs (0.4.14 hotfix): LOADER 0x540ad0 / PARSER 0x220e100 / ALLOC 0x231fb70.
#![allow(dead_code)]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// The same engine functions as scrim/draft_overlay. (0.5.0_3 was: LOADER 0x51cd40 / PARSER 0x2499f30 / ALLOC 0x25ab3d0 / DEALLOC 0x25ab430)
// ** 0.5.2 re-confirmation (2026-07-22, statically re-derived via string-xref - the methodology was validated by reproducing the known 0.5.1 answer):
//   the monomorphic copy split of asset-get (0.5.1 = copy#1 0x40f3d0 for the main family / copy#2 0xeb17d0 for strategy, training and draft)
//   **converged in 0.5.2 so that all 4 of our target paths use the same copy, 0x5ac950**.
//   Evidence: player_info lea@0x50a671 -> call 0x5ac950 / wide lea@0x50a6b5 -> call 0x5ac950 /
//         strategy lea@0xcdee2b, 0xce3a19 -> call 0x5ac950 / training's many leas -> all call 0x5ac950.
//   (The same script reproduced 0x40f3d0 and 0xeb17d0 exactly on 0.5.1 = the method is trustworthy.)
//   => STRAT_LOADER = the same address as LOADER -> install() **skips** the second hook (preventing a double chain on one function).
// ** 0.5.3 re-pin (2026-07-29) - LOADER was derived directly through the **canonical string-xref path** (the auto-matcher's 0x91ab0 is wrong:
//   it is a sibling in the same monomorphic clone family). The tool was first run on 0.5.2 to reproduce the documented answer (0x5ac950) as validation before applying it here.
//   Measured on 0.5.3: player_info lea@0xa93f3a / wide lea@0xa93f7e / strategy lea@0x200fb1b, 0x201446f /
//               training leas in 12 places - **all converge on call 0x2e1550** => STRAT_LOADER stays equal to LOADER (second hook skipped).
// ** 0.5.4 re-derivation (2026-08-04) - `tools/rederive.py loader`, the same canonical string-xref path as 0.5.3,
//   but run **without the old executable**: Steam overwrites it in place, so the exe2exe masked-signature method
//   every earlier migration used was unavailable. (Keep a copy of the exe before updating and it comes back.)
//   Evidence: all 4 hooked asset paths were located in .rdata, every `lea r64,[rip+disp]` pointing at them found,
//   and the first `call rel32` after each taken -> **16 of 16 sites converge on 0x2e35d0** (a .pdata function start,
//   size 425). player_info lea@0xb0a86a and wide lea@0xb0a8ae both sit in fn 0xb05640; strategy lea@0x218471b,
//   0x2189013 in fn 0x21846c0; training's leas span several callers. Zero sites reach any other target.
//   => the monomorphic copies are still merged, exactly as in 0.5.2/0.5.3, so STRAT_LOADER stays equal to LOADER
//   and install() keeps skipping the second hook.
//   Entry is `8 push + sub rsp,0x98` — the same idiom as 0.5.3, and the first 12B are precisely the eight pushes,
//   so the 12B chained install is unchanged.
const LOADER_RVA: usize = 0x2e35d0; // 0.5.4 (0.5.3 was 0x2e1550, 0.5.2 0x5ac950).
// ** 0.5.4 (2026-08-04): exe2exe `match` against the kept 0.5.3 binary - **1 hit at 320 and at 640 bytes**
//   of masked signature, size 2192. Three first-principles attempts had failed on this one (error-marker
//   store, 0x90 stride as imul, node-type string xref); with the old exe it took a single command.
const PARSER_RVA: usize = 0x1a3ce0; // 0.5.4 (0.5.3 was 0x1a6530, 0.5.2 0x24b5a00). The 3-argument contract (out, ptr, len), the `:`/`{`/`}` parsing, out[2]=-1 on error and the 0x90 node stride are all confirmed identical => NT_SIZE unchanged.
// WARNING in 0.5.3 the 2-argument `__rust_alloc(size, align)` shim **disappeared** (inlined into every call site) => we call the internal heap helper directly.
//   ~~candidate 0xbb2bd0 (align fixed at 8, aborts on OOM)~~ -> **0x28f7df0 adopted** (instruction-identical to 0.5.2's 0x25d9640, preserves returning 0 on OOM,
//   and matches the value used by the parallel ai_adjust session = unified across mods). For the contract see the `AllocFn` comment above.
// ** 0.5.4 (2026-08-04): reached through `__rust_realloc` (0x29a7640) rather than guessed — its over-aligned path
//   calls this with `rdx = 0` and `r8 = align + size` and never sets `rcx`, which *is* the documented contract.
//   The body confirms the rest: GetProcessHeap -> `test rax,rax` -> **`xor eax,eax; ret` on failure** (returns 0,
//   does not abort, so the null check in `append_child` still means something) -> else HeapAlloc by tail-jmp with
//   rcx = heap, edx = flags, r8 = size. Still no align argument (see the AllocFn warning below).
const ALLOC_RVA: usize  = 0x29bb920; // 0.5.4 heap alloc helper (0.5.3 was 0x28f7df0). (corresponds to 0.5.2's 0x25d9640. The old __rust_alloc shim 0x25c4d30 does not exist in 0.5.3).
const DEALLOC_RVA: usize = 0x1000; // 0.5.3 (0.5.2 was 0x25c4d90). The only `__rust_dealloc(ptr,size,align)`-shaped function. Currently unused.
const NT_SIZE: usize = 0x90;

// Target paths + our edited .ui text (embedded at compile time).
const PATH_PI: &[u8]   = b"asset/base/ui/layout/ingame_component/player_info";
const PATH_WIDE: &[u8] = b"asset/base/ui/layout/ingame_component/wide_player_info";
// `../../` rather than `../`: this file moved down one level, from `src/` into
// `src/tactics/`, when the mod was merged into riot_items_tfm2.
//
// These are embedded and delivered through the loader hook below, and
// deliberately NOT listed in `mod.override_info`. An override is applied by the
// loader before any of this code runs, so it cannot be conditional — listing
// them there pinned the 4-slot layout on even with `slots = 3`, where the whole
// point of the `mode4` gate in `loader_body` is to leave the vanilla template
// alone. (They were listed during the merge; removed 2026-08-04.)
const UI_PI: &str   = include_str!("../../ui/layout/ingame_component/player_info.ui");
const UI_WIDE: &str = include_str!("../../ui/layout/ingame_component/wide_player_info.ui");


type LoaderFn = extern "win64" fn(usize, *const u8, usize) -> usize;
type ParserFn = extern "win64" fn(*mut u8, *const u8, usize);
// * 0.5.3: the 2-argument `__rust_alloc(size, align)` shim is gone (inlined into every call site), so we call **the internal heap helper directly**.
//   Contract = (rcx = ignored, rdx = flags(0), r8 = size) -> rax = ptr / on failure it **returns 0** (not abort = the null check below still works).
//   It is instruction-for-instruction identical to the corresponding function in the old exe (0.5.2's 0x25d9640), and 0.5.2's __rust_alloc took a tail-jmp
//   into exactly this helper for align <= 0x10, so the resulting block is identical too (-> consistent with __rust_dealloc(align 8) and the game parser's free path).
//   WARNING it takes no align argument - if align > 0x10 is ever needed, this path must not be used (the current use is align 8).
type AllocFn  = extern "win64" fn(usize, usize, usize) -> usize;

static BASE: AtomicUsize = AtomicUsize::new(0);
static TRAMP: AtomicUsize = AtomicUsize::new(0);
static INSTALLED: AtomicBool = AtomicBool::new(false);
// * 4-item mode gate: when false no UI modification happens (3-slot mode = vanilla). Set by lib.rs from the cfg.
pub static MODE4: AtomicBool = AtomicBool::new(true);
// * Gate for replacing the in-match player_info slot UI: turned on once the 4th purchase was wired up (prevents empty slots while unwired). Off by default.
pub static IN_MATCH_UI: AtomicBool = AtomicBool::new(true); // * ON: inject the #slot3 node (the target the overlay fills). The bounds patch stays off, so there is no OOB crash.
// Replacement idempotence (avoids re-replacing the same template ptr; a reload = a new ptr = replace again).
static LAST_PI: AtomicUsize = AtomicUsize::new(0);
static LAST_WIDE: AtomicUsize = AtomicUsize::new(0);

/// Diagnostic: `(hook installed, player_info replaced, wide replaced)` so far
/// this session.
///
/// Each template is replaced from [`loader_body`], which only sees a template
/// the game loads *after* [`install`] has run. Before the merge these two
/// `player_info` templates were also listed in `mod.override_info`, which is
/// applied at load time and cannot miss — so a miss here was invisible. Now
/// that the loader hook is the only delivery route, "did it ever fire" is a
/// question worth being able to answer.
pub fn inject_state() -> (bool, bool, bool) {
    (
        INSTALLED.load(Ordering::Relaxed),
        LAST_PI.load(Ordering::Relaxed) != 0,
        LAST_WIDE.load(Ordering::Relaxed) != 0,
    )
}

pub static DBG: AtomicBool = AtomicBool::new(false);
static LOGP: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
pub fn set_log(p: String) { *LOGP.lock().unwrap_or_else(|e| e.into_inner()) = p; }
fn logln(s: &str) {
    if !DBG.load(Ordering::Relaxed) { return; }
    let p = LOGP.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if p.is_empty() { return; }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = writeln!(f, "{}", s);
    }
}

type BOOL = i32;
#[link(name = "kernel32")]
extern "system" {
    fn VirtualAlloc(addr: usize, size: usize, typ: u32, protect: u32) -> usize;
    fn VirtualProtect(addr: usize, size: usize, new: u32, old: *mut u32) -> BOOL;
    fn FlushInstructionCache(proc: usize, addr: usize, size: usize) -> BOOL;
    fn GetCurrentProcess() -> usize;
    fn GetModuleHandleW(name: *const u16) -> usize;
}
const MEM_CR: u32 = 0x1000 | 0x2000;
const RWX: u32 = 0x40;

// Shared loader detour body - used by both monomorphic copies of asset-get (0x40f3d0 = main family, 0xeb17d0 = strategy family).
//   r = the lookup result (UI template ptr). Fan-in is 100+, so it stays cheap by checking len first.
fn loader_body(path: *const u8, len: usize, r: usize) {
    if path.is_null() || r <= 0x10000 || len >= 200 { return; }
    let s = unsafe { core::slice::from_raw_parts(path, len) };
    let mode4 = MODE4.load(Ordering::Relaxed);
    // In-match player_info (the 4-slot replacement) = MODE4 only, behind the IN_MATCH_UI gate (turned on once the 4th purchase was wired up - prevents empty slots).
    //   mode=3 keeps the vanilla 3 slots (no replacement). The strategy-screen overlay (STRAT) is common to modes 3 and 4.
    if mode4 && IN_MATCH_UI.load(Ordering::Relaxed) && s == PATH_PI && r != LAST_PI.load(Ordering::Relaxed) {
        let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { replace_children(r, UI_PI) })).unwrap_or(false);
        if ok { LAST_PI.store(r, Ordering::Relaxed); logln(&format!("replaced player_info r={:#x}", r)); }
    } else if mode4 && IN_MATCH_UI.load(Ordering::Relaxed) && s == PATH_WIDE && r != LAST_WIDE.load(Ordering::Relaxed) {
        let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { replace_children(r, UI_WIDE) })).unwrap_or(false);
        if ok { LAST_WIDE.store(r, Ordering::Relaxed); logln(&format!("replaced wide r={:#x}", r)); }
    }
}
// copy #1: 0x40f3d0 (main / player_info / wide / title etc.)
extern "win64" fn detour(am: usize, path: *const u8, len: usize) -> usize {
    let t = TRAMP.load(Ordering::Relaxed);
    if t == 0 { return 0; }
    let r = unsafe { core::mem::transmute::<usize, LoaderFn>(t)(am, path, len) };
    loader_body(path, len, r);
    r
}

// Replace the child array of the loaded template r (its root) with the children parsed from our .ui.
unsafe fn replace_children(r: usize, text: &str) -> bool {
    if r <= 0x10000 { return false; }
    let base = BASE.load(Ordering::Relaxed);
    if base == 0 { return false; }
    let parser: ParserFn = core::mem::transmute(base + PARSER_RVA);
    let mut out = [0u8; 0x400];
    parser(out.as_mut_ptr(), text.as_ptr(), text.len());
    let my = out.as_ptr().add(0x10) as usize; // our root NodeTemplate
    if *(my as *const usize) == usize::MAX { logln("parse ERR"); return false; }
    // Our root's children {cap@+0x48, ptr@+0x50, len@+0x58} -> on the heap (allocated by the parser). Copy them in as r's children.
    let mcap = *((my + 0x48) as *const usize);
    let mptr = *((my + 0x50) as *const usize);
    let mlen = *((my + 0x58) as *const usize);
    if mptr <= 0x10000 || mlen == 0 || mlen > 2000 { logln("bad my child"); return false; }
    // Replace r's children (leaking the old array is harmless). Order ptr -> cap -> len (safe against an interleaved read).
    *((r + 0x50) as *mut usize) = mptr;
    *((r + 0x48) as *mut usize) = mcap;
    *((r + 0x58) as *mut usize) = mlen;
    true
}

// Find a container by id in the template tree.
unsafe fn find_tmpl(node: usize, target: &[u8], depth: usize) -> usize {
    if node <= 0x10000 || depth > 12 { return 0; }
    let idptr = *((node + 0x08) as *const usize);
    let idlen = *((node + 0x10) as *const usize);
    if idlen == target.len() && idptr > 0x10000 {
        if core::slice::from_raw_parts(idptr as *const u8, idlen) == target { return node; }
    }
    let cptr = *((node + 0x50) as *const usize);
    let clen = *((node + 0x58) as *const usize);
    if cptr > 0x10000 && clen < 1000 {
        for i in 0..clen {
            let f = find_tmpl(cptr + i * NT_SIZE, target, depth + 1);
            if f != 0 { return f; }
        }
    }
    0
}
// Append a fragment node at the end of the container container_id.
unsafe fn install_one(base: usize, rva: usize, tramp_slot: &AtomicUsize, detour_addr: usize) -> bool {
    let fn_addr = base + rva;
    // Skip if it is already our hook (a reinstall) - i.e. if the entry point is a movabs to our detour (0x48 0xb8 + the detour address).
    let mut cur = [0u8; 12];
    core::ptr::copy_nonoverlapping(fn_addr as *const u8, cur.as_mut_ptr(), 12);
    if cur[0] == 0x48 && cur[1] == 0xb8 {
        let tgt = usize::from_le_bytes(cur[2..10].try_into().unwrap());
        if tgt == detour_addr { logln(&format!("already my hook fn={:#x}", fn_addr)); return true; }
    }
    let stub = VirtualAlloc(0, 64, MEM_CR, RWX);
    if stub == 0 { return false; }
    // tramp = [the current 12 entry bytes (the original prologue or another mod's jmp)] + [movabs rax, fn+0xc; jmp rax]
    let mut s: Vec<u8> = Vec::new();
    s.extend_from_slice(&cur);
    s.extend_from_slice(&[0x48, 0xb8]); s.extend_from_slice(&(fn_addr + 0xc).to_le_bytes());
    s.extend_from_slice(&[0xff, 0xe0]);
    core::ptr::copy_nonoverlapping(s.as_ptr(), stub as *mut u8, s.len());
    tramp_slot.store(stub, Ordering::Relaxed);
    // patch entry = movabs rax, detour; jmp rax
    let mut patch = [0u8; 12];
    patch[0] = 0x48; patch[1] = 0xb8; patch[2..10].copy_from_slice(&detour_addr.to_le_bytes()); patch[10] = 0xff; patch[11] = 0xe0;
    let mut old: u32 = 0;
    if VirtualProtect(fn_addr, 12, RWX, &mut old) == 0 { return false; }
    core::ptr::copy_nonoverlapping(patch.as_ptr(), fn_addr as *mut u8, 12);
    VirtualProtect(fn_addr, 12, old, &mut old);
    FlushInstructionCache(GetCurrentProcess(), fn_addr, 12);
    logln(&format!("chained install OK fn={:#x} tramp={:#x}", fn_addr, stub));
    true
}

pub unsafe fn install() -> bool {
    if INSTALLED.swap(true, Ordering::Relaxed) { return true; }
    let base = GetModuleHandleW(core::ptr::null());
    if base == 0 { INSTALLED.store(false, Ordering::Relaxed); return false; }
    BASE.store(base, Ordering::Relaxed);
    // A second hook on `STRAT_LOADER` used to live here, for the monomorphic
    // copy of asset-get that served the strategy screen (0.5.1 split it out as
    // 0xeb17d0). It has been the same function as `LOADER` since 0.5.2, so the
    // second install was already being skipped — and the only path that needed
    // it, the strategy template, is no longer injected at all.
    let a = install_one(base, LOADER_RVA, &TRAMP, detour as usize);
    if !a { INSTALLED.store(false, Ordering::Relaxed); return false; }
    true
}
